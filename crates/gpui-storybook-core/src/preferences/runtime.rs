use super::*;

pub(super) struct Runtime<L>
where
    L: Language,
{
    state: PreferenceState,
    repository_options: RepositoryOptions,
    repository: Option<PreferenceRepository>,
    languages: Vec<(L, LanguageTag)>,
    supported_languages: SupportedLanguages,
    locale_detector: Arc<dyn LocaleDetector>,
    detected_scheme: SystemColorScheme,
    detected_locales: DetectedLocales,
    overrides: ResolutionOverrides,
    apply_consumer_locale: Rc<dyn Fn(L, &mut App) -> Result<(), String>>,
    localize_consumer_language: Rc<dyn Fn(L, &App) -> Option<String>>,
    applied_theme: Option<AppliedTheme>,
    applied_language: Option<LanguageTag>,
    save_in_flight: bool,
    in_flight_edits: PreferenceEdits,
    pending_edits: PreferenceEdits,
    #[cfg(test)]
    next_save_completion: Option<tokio::sync::oneshot::Sender<Result<(), String>>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct AppliedTheme {
    scheme: SystemColorScheme,
    theme: ThemeId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum PreferenceEdit {
    WindowMode(StorybookWindowMode),
    ColorScheme(PreferredColorScheme),
    Theme {
        scheme: SystemColorScheme,
        theme: Option<ThemeId>,
    },
    Language(PreferredLanguage),
    Scrollbar(PreferredScrollbar),
}

impl PreferenceEdit {
    fn apply_to(&self, preferences: &mut StorybookPreferences) {
        match self {
            Self::WindowMode(value) => preferences.window_mode = *value,
            Self::ColorScheme(value) => preferences.color_scheme = *value,
            Self::Theme { scheme, theme } => match scheme {
                SystemColorScheme::Light => preferences.light_theme = theme.clone(),
                SystemColorScheme::Dark => preferences.dark_theme = theme.clone(),
            },
            Self::Language(value) => preferences.language = value.clone(),
            Self::Scrollbar(value) => preferences.scrollbar = *value,
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct PreferenceEdits {
    window_mode: Option<StorybookWindowMode>,
    color_scheme: Option<PreferredColorScheme>,
    light_theme: Option<Option<ThemeId>>,
    dark_theme: Option<Option<ThemeId>>,
    language: Option<PreferredLanguage>,
    scrollbar: Option<PreferredScrollbar>,
}

impl PreferenceEdits {
    fn is_empty(&self) -> bool {
        self.window_mode.is_none()
            && self.color_scheme.is_none()
            && self.light_theme.is_none()
            && self.dark_theme.is_none()
            && self.language.is_none()
            && self.scrollbar.is_none()
    }

    fn record(&mut self, edit: PreferenceEdit) {
        match edit {
            PreferenceEdit::WindowMode(value) => self.window_mode = Some(value),
            PreferenceEdit::ColorScheme(value) => self.color_scheme = Some(value),
            PreferenceEdit::Theme { scheme, theme } => match scheme {
                SystemColorScheme::Light => self.light_theme = Some(theme),
                SystemColorScheme::Dark => self.dark_theme = Some(theme),
            },
            PreferenceEdit::Language(value) => self.language = Some(value),
            PreferenceEdit::Scrollbar(value) => self.scrollbar = Some(value),
        }
    }

    fn apply_to(&self, preferences: &mut StorybookPreferences) {
        if let Some(value) = self.window_mode {
            preferences.window_mode = value;
        }
        if let Some(value) = self.color_scheme {
            preferences.color_scheme = value;
        }
        if let Some(value) = &self.light_theme {
            preferences.light_theme = value.clone();
        }
        if let Some(value) = &self.dark_theme {
            preferences.dark_theme = value.clone();
        }
        if let Some(value) = &self.language {
            preferences.language = value.clone();
        }
        if let Some(value) = self.scrollbar {
            preferences.scrollbar = value;
        }
    }

    fn coalesce(&mut self, newer: Self) {
        if newer.window_mode.is_some() {
            self.window_mode = newer.window_mode;
        }
        if newer.color_scheme.is_some() {
            self.color_scheme = newer.color_scheme;
        }
        if newer.light_theme.is_some() {
            self.light_theme = newer.light_theme;
        }
        if newer.dark_theme.is_some() {
            self.dark_theme = newer.dark_theme;
        }
        if newer.language.is_some() {
            self.language = newer.language;
        }
        if newer.scrollbar.is_some() {
            self.scrollbar = newer.scrollbar;
        }
    }
}

pub(crate) enum StartupLoad {
    Loaded {
        repository: PreferenceRepository,
        saved: StorybookPreferences,
        recovery: Option<RecoveryDiagnostic>,
    },
    Failed {
        repository: Option<PreferenceRepository>,
        path: Option<PathBuf>,
        category: String,
    },
}

pub(crate) enum RetryOpen {
    Opened {
        repository: PreferenceRepository,
        saved: StorybookPreferences,
        recovery: Option<RecoveryDiagnostic>,
    },
    Failed {
        category: String,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RetryOperation {
    Reload,
    Save,
}

pub(super) async fn load_preferences(
    repository_options: RepositoryOptions,
    repository: Option<PreferenceRepository>,
) -> StartupLoad {
    let fallback_path = repository_options.json_path.clone();
    let (repository, recovery) = match repository {
        Some(repository) => (repository, None),
        None => match PreferenceRepository::open(repository_options).await {
            Ok(open) => (open.repository, open.recovery),
            Err(error) => {
                let path = repository_open_path(&error).or(fallback_path);
                tracing::error!(
                    path = ?path,
                    error = %error,
                    error_debug = ?error,
                    "failed to open Storybook preference repository"
                );
                return StartupLoad::Failed {
                    repository: None,
                    path,
                    category: repository_open_category(&error).to_owned(),
                };
            },
        },
    };

    match repository.load().await {
        Ok(record) => StartupLoad::Loaded {
            repository,
            saved: record.map_or_else(StorybookPreferences::default, |row| row.preferences),
            recovery,
        },
        Err(error) => {
            let category = store_error_category(&error);
            let path = repository.path().map(PathBuf::from);
            tracing::error!(
                path = ?path,
                category,
                "failed to load Storybook preferences"
            );
            StartupLoad::Failed {
                repository: Some(repository),
                path,
                category: category.to_owned(),
            }
        },
    }
}

fn save_failure_notification(message: SharedString, retry_label: SharedString) -> Notification {
    Notification::error(message).action(move |_, _, cx| {
        Button::new("retry-preference-save")
            .debug_selector(|| "retry-preference-save".to_owned())
            .primary()
            .label(retry_label.clone())
            .on_click(cx.listener(|notification, _, window, cx| {
                window.dispatch_action(Box::new(crate::actions::RetryPreferences), cx);
                notification.dismiss(window, cx);
            }))
    })
}
impl<L> Runtime<L>
where
    L: Language,
{
    pub(super) fn new(
        options: RuntimeOptions<L>,
        cx: &App,
    ) -> Result<Self, ResolvePreferencesError> {
        let detected_locales = options.locale_detector.detect();
        let saved = StorybookPreferences::default();
        let resolved = resolve(
            &saved,
            options.initial_scheme,
            &detected_locales,
            &options.supported_languages,
            &options.overrides,
            cx,
        )?;
        let resolution_diagnostics = resolved.diagnostics.clone();

        Ok(Self {
            state: PreferenceState {
                saved,
                resolved,
                persistence_status: PersistenceStatus::Loading,
                diagnostics: Vec::new(),
                resolution_diagnostics,
            },
            repository_options: options.repository,
            repository: None,
            languages: options.languages,
            supported_languages: options.supported_languages,
            locale_detector: options.locale_detector,
            detected_scheme: options.initial_scheme,
            detected_locales,
            overrides: options.overrides,
            apply_consumer_locale: options.apply_consumer_locale,
            localize_consumer_language: options.localize_consumer_language,
            applied_theme: None,
            applied_language: None,
            save_in_flight: false,
            in_flight_edits: PreferenceEdits::default(),
            pending_edits: PreferenceEdits::default(),
            #[cfg(test)]
            next_save_completion: None,
        })
    }

    fn resolve_current(&mut self, cx: &App) -> Result<(), ResolvePreferencesError> {
        let resolved = resolve(
            &self.state.saved,
            self.detected_scheme,
            &self.detected_locales,
            &self.supported_languages,
            &self.overrides,
            cx,
        )?;
        self.state.resolution_diagnostics = resolved.diagnostics.clone();
        self.state.resolved = resolved;
        Ok(())
    }

    fn apply_resolved(&mut self, cx: &mut App) {
        let scheme = self.state.resolved.color_scheme.scheme;
        let effective_theme = AppliedTheme {
            scheme,
            theme: self.state.resolved.theme.theme.clone(),
        };
        if self.applied_theme.as_ref() != Some(&effective_theme)
            && let Some(config) = ThemeRegistry::global(cx)
                .themes()
                .get(effective_theme.theme.as_str())
                .cloned()
        {
            Theme::change(theme_mode(scheme), None, cx);
            Theme::global_mut(cx).apply_config(&config);
            self.applied_theme = Some(effective_theme);
        }
        Theme::global_mut(cx).scrollbar_mode = scrollbar_mode(self.state.resolved.scrollbar);

        let language = self.state.resolved.language.language.clone();
        if self.applied_language.as_ref() != Some(&language) {
            match self.apply_language(&language, cx) {
                Ok(()) => {
                    self.applied_language = Some(language);
                },
                Err(category) => {
                    self.applied_language = None;
                    let diagnostic = PreferenceDiagnostic::LocaleApplicationFailed {
                        language: language.to_string(),
                        category,
                    };
                    if !self.state.diagnostics.contains(&diagnostic) {
                        self.state.diagnostics.push(diagnostic);
                    }
                },
            }
        }
        cx.refresh_windows();
    }

    fn apply_language(&self, tag: &LanguageTag, cx: &mut App) -> Result<(), String> {
        let identifier = tag.as_identifier().clone();
        let typed = self
            .languages
            .iter()
            .find_map(|(language, candidate)| (candidate == tag).then_some(*language))
            .ok_or_else(|| "typed_language_mapping".to_owned())?;
        cx.set_global(crate::language::CurrentLanguage(typed));
        gpui_component::set_locale(&identifier.to_string());
        i18n::change_locale(cx, identifier).map_err(|_| "storybook_locale".to_owned())?;
        (self.apply_consumer_locale)(typed, cx)?;
        Ok(())
    }

    fn optimistic_change(&mut self, edit: PreferenceEdit, cx: &mut App) {
        self.optimistic_changes([edit], cx);
    }

    fn optimistic_changes(
        &mut self,
        edits: impl IntoIterator<Item = PreferenceEdit>,
        cx: &mut App,
    ) {
        for edit in edits {
            edit.apply_to(&mut self.state.saved);
            self.pending_edits.record(edit);
        }
        if let Err(error) = self.resolve_current(cx) {
            tracing::error!(error = %error, "failed to resolve changed Storybook preferences");
            return;
        }
        self.apply_resolved(cx);
        self.queue_save(cx);
    }

    fn queue_save(&mut self, cx: &mut App) {
        if self.save_in_flight {
            self.state.persistence_status = PersistenceStatus::Saving;
            return;
        }
        let Some(repository) = self.repository.clone() else {
            self.start_reopen(cx);
            return;
        };

        let saved = self.state.saved.clone();
        self.in_flight_edits = mem::take(&mut self.pending_edits);
        self.save_in_flight = true;
        self.state.persistence_status = PersistenceStatus::Saving;
        let path = repository.path().map(PathBuf::from);
        let storage_task = spawn_storage(cx, async move {
            repository.upsert(saved).await.map(|_| ()).map_err(|error| {
                let category = store_error_category(&error);
                tracing::error!(
                    path = ?path,
                    category,
                    "failed to save Storybook preferences"
                );
                category.to_owned()
            })
        });
        cx.spawn(async move |cx| {
            let result = storage_task
                .await
                .map_err(|_| "tokio_join".to_owned())
                .and_then(|result| result);
            cx.update(|cx| {
                cx.update_global::<StorybookPreferencesGlobal, _>(|global, cx| {
                    global.0.finish_save(result, cx);
                });
            });
        })
        .detach();
    }

    fn start_reopen(&mut self, cx: &mut App) {
        if self.save_in_flight {
            return;
        }
        self.save_in_flight = true;
        self.state.persistence_status = PersistenceStatus::Saving;
        let repository_options = self.repository_options.clone();
        let storage_task = spawn_storage(cx, async move {
            match PreferenceRepository::open(repository_options).await {
                Ok(open) => {
                    let repository = open.repository;
                    let path = repository.path().map(PathBuf::from);
                    match repository.load().await {
                        Ok(record) => RetryOpen::Opened {
                            repository,
                            saved: record
                                .map_or_else(StorybookPreferences::default, |row| row.preferences),
                            recovery: open.recovery,
                        },
                        Err(error) => {
                            let category = store_error_category(&error);
                            tracing::error!(
                                path = ?path,
                                category,
                                "failed to load reopened Storybook preferences"
                            );
                            RetryOpen::Failed {
                                category: category.to_owned(),
                            }
                        },
                    }
                },
                Err(error) => {
                    tracing::error!(
                        path = ?repository_open_path(&error),
                        error = %error,
                        error_debug = ?error,
                        "failed to reopen Storybook preference repository"
                    );
                    RetryOpen::Failed {
                        category: repository_open_category(&error).to_owned(),
                    }
                },
            }
        });
        cx.spawn(async move |cx| {
            let result = storage_task.await.unwrap_or_else(|_| RetryOpen::Failed {
                category: "tokio_join".to_owned(),
            });
            cx.update(|cx| {
                cx.update_global::<StorybookPreferencesGlobal, _>(|global, cx| {
                    global.0.finish_reopen(result, cx);
                });
            });
        })
        .detach();
    }

    fn start_reload(&mut self, cx: &mut App) {
        if self.save_in_flight {
            return;
        }
        self.save_in_flight = true;
        self.state.persistence_status = PersistenceStatus::Loading;
        let repository_options = self.repository_options.clone();
        let repository = self.repository.clone();
        let storage_task = spawn_storage(cx, async move {
            load_preferences(repository_options, repository).await
        });
        cx.spawn(async move |cx| {
            let loaded = storage_task.await.unwrap_or_else(|_| StartupLoad::Failed {
                repository: None,
                path: None,
                category: "tokio_join".to_owned(),
            });
            cx.update(|cx| {
                cx.update_global::<StorybookPreferencesGlobal, _>(|global, cx| {
                    global.0.finish_reload(loaded, cx);
                });
            });
        })
        .detach();
    }

    fn retry_operation(&self) -> RetryOperation {
        let failed_save = self
            .state
            .diagnostics
            .iter()
            .any(|diagnostic| matches!(diagnostic, PreferenceDiagnostic::SaveFailed { .. }));
        if !self.pending_edits.is_empty() || failed_save {
            RetryOperation::Save
        } else {
            RetryOperation::Reload
        }
    }

    fn notify_save_failure(&self, cx: &mut App) {
        let message: SharedString =
            crate::messages::text(cx, crate::messages::StorybookMessage::PersistenceSaveFailed)
                .into();
        let retry_label: SharedString =
            crate::messages::text(cx, crate::messages::StorybookMessage::RetrySave).into();
        for handle in cx.windows() {
            let message = message.clone();
            let retry_label = retry_label.clone();
            let _ = handle.update(cx, |_, window, cx| {
                window.push_notification(save_failure_notification(message, retry_label), cx);
            });
        }
    }
}

impl<L> PreferenceRuntime for Runtime<L>
where
    L: Language,
{
    fn state(&self) -> &PreferenceState {
        &self.state
    }

    fn available_locales(&self, cx: &App) -> Vec<(String, LanguageTag)> {
        self.languages
            .iter()
            .map(|(language, tag)| {
                let label = (self.localize_consumer_language)(*language, cx)
                    .unwrap_or_else(|| tag.to_string());
                (label, tag.clone())
            })
            .collect()
    }

    fn select_color_scheme(&mut self, value: PreferredColorScheme, cx: &mut App) {
        self.optimistic_change(PreferenceEdit::ColorScheme(value), cx);
    }

    fn select_theme(&mut self, scheme: SystemColorScheme, theme: ThemeId, cx: &mut App) {
        let color_scheme = match scheme {
            SystemColorScheme::Light => PreferredColorScheme::Light,
            SystemColorScheme::Dark => PreferredColorScheme::Dark,
        };
        self.optimistic_changes(
            [
                PreferenceEdit::ColorScheme(color_scheme),
                PreferenceEdit::Theme {
                    scheme,
                    theme: Some(theme),
                },
            ],
            cx,
        );
    }

    fn select_language(&mut self, value: PreferredLanguage, cx: &mut App) {
        if matches!(value, PreferredLanguage::System) {
            self.detected_locales = self.locale_detector.detect();
        }
        self.optimistic_change(PreferenceEdit::Language(value), cx);
    }

    fn select_scrollbar(&mut self, value: PreferredScrollbar, cx: &mut App) {
        self.optimistic_change(PreferenceEdit::Scrollbar(value), cx);
    }

    fn select_window_mode(&mut self, value: StorybookWindowMode, cx: &mut App) {
        self.optimistic_change(PreferenceEdit::WindowMode(value), cx);
    }

    fn window_appearance_changed(&mut self, window: &mut Window, cx: &mut App) {
        self.detected_scheme = color_scheme(window.appearance());
        if self.state.saved.color_scheme == PreferredColorScheme::System
            && self.overrides.color_scheme.is_none()
            && self.resolve_current(cx).is_ok()
        {
            self.apply_resolved(cx);
        }
    }

    fn window_activated(&mut self, window: &mut Window, cx: &mut App) {
        if !window.is_window_active() {
            return;
        }
        self.detected_scheme = color_scheme(window.appearance());
        if matches!(self.state.saved.language, PreferredLanguage::System)
            && self.overrides.language.is_none()
        {
            self.detected_locales = self.locale_detector.detect();
        }
        if self.resolve_current(cx).is_ok() {
            self.apply_resolved(cx);
        }
    }

    #[cfg(not(target_family = "wasm"))]
    fn theme_registry_changed(&mut self, cx: &mut App) {
        self.applied_theme = None;
        if self.resolve_current(cx).is_ok() {
            self.apply_resolved(cx);
        }
    }

    fn retry_preferences(&mut self, cx: &mut App) {
        if self.save_in_flight {
            return;
        }

        if self.retry_operation() == RetryOperation::Save {
            if self.repository.is_some() {
                self.queue_save(cx);
            } else {
                self.start_reopen(cx);
            }
        } else {
            self.start_reload(cx);
        }
    }

    fn finish_loading(&mut self, loaded: StartupLoad, cx: &mut App) -> StorybookReady {
        match loaded {
            StartupLoad::Loaded {
                repository,
                saved,
                recovery,
            } => {
                self.repository = Some(repository);
                self.state.saved = saved;
                self.state.persistence_status = PersistenceStatus::Ready;
                if let Some(recovery) = recovery {
                    self.state
                        .diagnostics
                        .push(PreferenceDiagnostic::Recovered(recovery));
                }
            },
            StartupLoad::Failed {
                repository,
                path,
                category,
            } => {
                self.repository = repository;
                self.state.persistence_status = PersistenceStatus::Error;
                self.state
                    .diagnostics
                    .push(PreferenceDiagnostic::LoadFailed { path, category });
            },
        }

        if let Err(error) = self.resolve_current(cx) {
            tracing::error!(error = %error, "failed to resolve loaded Storybook preferences");
        } else {
            self.apply_resolved(cx);
        }

        StorybookReady {
            persistence_status: self.state.persistence_status,
            diagnostics: self.state.diagnostics.clone(),
        }
    }

    fn finish_save(&mut self, result: Result<(), String>, cx: &mut App) {
        #[cfg(test)]
        let completion_result = result.clone();
        self.save_in_flight = false;
        let completed_edits = mem::take(&mut self.in_flight_edits);
        match result {
            Ok(()) => {
                self.state.persistence_status = PersistenceStatus::Ready;
                if !self.pending_edits.is_empty() {
                    self.queue_save(cx);
                }
            },
            Err(category) => {
                let newer_edits = mem::take(&mut self.pending_edits);
                self.pending_edits = completed_edits;
                self.pending_edits.coalesce(newer_edits);
                self.state.persistence_status = PersistenceStatus::Error;
                self.state
                    .diagnostics
                    .push(PreferenceDiagnostic::SaveFailed { category });
                self.notify_save_failure(cx);
            },
        }
        #[cfg(test)]
        if let Some(completion) = self.next_save_completion.take() {
            let _ = completion.send(completion_result);
        }
        cx.refresh_windows();
    }

    fn finish_reload(&mut self, mut loaded: StartupLoad, cx: &mut App) {
        self.save_in_flight = false;
        let has_pending_edits = !self.pending_edits.is_empty();
        let loaded_successfully = match &mut loaded {
            StartupLoad::Loaded { saved, .. } => {
                self.pending_edits.apply_to(saved);
                true
            },
            StartupLoad::Failed { repository, .. } => {
                if has_pending_edits {
                    *repository = None;
                }
                false
            },
        };

        let _ = self.finish_loading(loaded, cx);
        if loaded_successfully && has_pending_edits {
            self.queue_save(cx);
        }
    }

    fn finish_reopen(&mut self, result: RetryOpen, cx: &mut App) {
        self.save_in_flight = false;
        match result {
            RetryOpen::Opened {
                repository,
                mut saved,
                recovery,
            } => {
                self.repository = Some(repository);
                if let Some(recovery) = recovery {
                    self.state
                        .diagnostics
                        .push(PreferenceDiagnostic::Recovered(recovery));
                }
                let has_pending_edits = !self.pending_edits.is_empty();
                self.pending_edits.apply_to(&mut saved);
                self.state.saved = saved;
                self.state.persistence_status = PersistenceStatus::Ready;
                if let Err(error) = self.resolve_current(cx) {
                    tracing::error!(error = %error, "failed to resolve reopened Storybook preferences");
                } else {
                    self.apply_resolved(cx);
                }
                if has_pending_edits {
                    self.queue_save(cx);
                }
            },
            RetryOpen::Failed { category } => {
                self.state.persistence_status = PersistenceStatus::Error;
                self.state
                    .diagnostics
                    .push(PreferenceDiagnostic::SaveFailed { category });
                self.notify_save_failure(cx);
                cx.refresh_windows();
            },
        }
    }
}

#[cfg(test)]
mod tests;
