//! GPUI-owned resolved preference state and runtime orchestration.

use std::{mem, path::PathBuf, rc::Rc, sync::Arc};

use gpui::{
    App, AsyncApp, BorrowAppContext as _, Global, InteractiveElement as _, SharedString, Task,
    Window, WindowAppearance,
};
use gpui_component::{
    Theme, ThemeMode, ThemeRegistry, WindowExt as _,
    button::{Button, ButtonVariants as _},
    notification::Notification,
    scroll::ScrollbarShow,
};
use gpui_storybook_preferences::{
    AvailableThemeResolver, DetectedLocales, LanguageTag, LocaleDetector, PersistenceMode,
    PreferenceRepository, PreferredColorScheme, PreferredLanguage, PreferredScrollbar,
    RecoveryDiagnostic, RepositoryOpenError, RepositoryOptions, ResolutionDiagnostic,
    ResolutionOverrides, ResolvePreferencesError, ResolvedPreferences, StorybookPreferences,
    SupportedLanguages, SystemColorScheme, ThemeId, resolve_preferences,
};
use unic_langid::LanguageIdentifier;

use crate::{i18n, language::Language};

/// Current state of local preference storage.
///
/// Locale application and resolution diagnostics do not change this status.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum PersistenceStatus {
    /// The local repository is opening and the saved document is loading.
    #[default]
    Loading,
    /// Saved intent is loaded and no write is outstanding.
    Ready,
    /// An optimistic session change is being written.
    Saving,
    /// Loading or saving failed; resolved session state remains usable.
    Error,
}

/// Structured, observable startup or preference-application diagnostic.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PreferenceDiagnostic {
    /// Invalid local JSON was archived before defaults were applied.
    Recovered(RecoveryDiagnostic),
    /// Repository startup failed and defaults remain active for this session.
    LoadFailed {
        /// Optional path involved in the failure.
        path: Option<PathBuf>,
        /// Stable error category.
        category: String,
    },
    /// A save failed; the optimistic in-memory selection remains active.
    SaveFailed {
        /// Stable error category.
        category: String,
    },
    /// Applying the resolved locale failed.
    LocaleApplicationFailed {
        /// Resolved BCP 47 language tag.
        language: String,
        /// Stable error category.
        category: String,
    },
}

/// Readiness result returned after initial load and foreground resolution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StorybookReady {
    /// Storage state after startup completes.
    pub persistence_status: PersistenceStatus,
    /// Startup diagnostics, including storage recovery/failure and locale
    /// application failure.
    pub diagnostics: Vec<PreferenceDiagnostic>,
}

/// Saved intent and effective values used by Storybook menus and windows.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreferenceState {
    /// User intent retained independently of system resolution.
    pub saved: StorybookPreferences,
    /// Effective theme, appearance, language, source, and fallback diagnostics.
    pub resolved: ResolvedPreferences,
    /// Current local storage activity. Locale failures do not change it.
    pub persistence_status: PersistenceStatus,
    /// Storage and locale-application diagnostics.
    pub diagnostics: Vec<PreferenceDiagnostic>,
    /// Resolution fallback diagnostics for direct UI inspection.
    pub resolution_diagnostics: Vec<ResolutionDiagnostic>,
}

/// Prevalidated runtime configuration assembled by the public facade.
#[doc(hidden)]
pub struct RuntimeOptions<L>
where
    L: Language,
{
    pub repository: RepositoryOptions,
    pub languages: Vec<(L, LanguageTag)>,
    pub supported_languages: SupportedLanguages,
    pub locale_detector: Arc<dyn LocaleDetector>,
    pub initial_scheme: SystemColorScheme,
    pub overrides: ResolutionOverrides,
    pub apply_consumer_locale: Rc<dyn Fn(L, &mut App) -> Result<(), String>>,
    pub localize_consumer_language: Rc<dyn Fn(L, &App) -> Option<String>>,
}

pub(crate) trait PreferenceRuntime: 'static {
    fn state(&self) -> &PreferenceState;
    fn available_locales(&self, cx: &App) -> Vec<(String, LanguageTag)>;
    fn select_color_scheme(&mut self, value: PreferredColorScheme, cx: &mut App);
    fn select_theme(&mut self, scheme: SystemColorScheme, theme: ThemeId, cx: &mut App);
    fn select_language(&mut self, value: PreferredLanguage, cx: &mut App);
    fn select_scrollbar(&mut self, value: PreferredScrollbar, cx: &mut App);
    fn window_appearance_changed(&mut self, window: &mut Window, cx: &mut App);
    fn window_activated(&mut self, window: &mut Window, cx: &mut App);
    fn theme_registry_changed(&mut self, cx: &mut App);
    fn retry_preferences(&mut self, cx: &mut App);
    fn finish_loading(&mut self, loaded: StartupLoad, cx: &mut App) -> StorybookReady;
    fn finish_save(&mut self, result: Result<(), String>, cx: &mut App);
    fn finish_reload(&mut self, loaded: StartupLoad, cx: &mut App);
    fn finish_reopen(&mut self, result: RetryOpen, cx: &mut App);
}

pub(crate) struct StorybookPreferencesGlobal(pub(crate) Box<dyn PreferenceRuntime>);

impl Global for StorybookPreferencesGlobal {}

struct Runtime<L>
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
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct AppliedTheme {
    scheme: SystemColorScheme,
    theme: ThemeId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum PreferenceEdit {
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
    color_scheme: Option<PreferredColorScheme>,
    light_theme: Option<Option<ThemeId>>,
    dark_theme: Option<Option<ThemeId>>,
    language: Option<PreferredLanguage>,
    scrollbar: Option<PreferredScrollbar>,
}

impl PreferenceEdits {
    fn is_empty(&self) -> bool {
        self.color_scheme.is_none()
            && self.light_theme.is_none()
            && self.dark_theme.is_none()
            && self.language.is_none()
            && self.scrollbar.is_none()
    }

    fn record(&mut self, edit: PreferenceEdit) {
        match edit {
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

async fn load_preferences(
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

struct RegistryThemes<'a> {
    registry: &'a ThemeRegistry,
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

impl AvailableThemeResolver for RegistryThemes<'_> {
    fn is_available(&self, scheme: SystemColorScheme, theme: &ThemeId) -> bool {
        self.registry
            .themes()
            .get(theme.as_str())
            .is_some_and(|config| config.mode == theme_mode(scheme))
    }

    fn fallback(&self, scheme: SystemColorScheme) -> Option<ThemeId> {
        let config = match scheme {
            SystemColorScheme::Light => self.registry.default_light_theme(),
            SystemColorScheme::Dark => self.registry.default_dark_theme(),
        };
        ThemeId::new(config.name.as_ref()).ok()
    }
}

impl<L> Runtime<L>
where
    L: Language,
{
    fn new(options: RuntimeOptions<L>, cx: &App) -> Result<Self, ResolvePreferencesError> {
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
        Theme::global_mut(cx).scrollbar_show = scrollbar_show(self.state.resolved.scrollbar);

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
        edit.apply_to(&mut self.state.saved);
        self.pending_edits.record(edit);
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
        let storage_task = gpui_tokio::Tokio::spawn(cx, async move {
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
        let storage_task = gpui_tokio::Tokio::spawn(cx, async move {
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
        let storage_task = gpui_tokio::Tokio::spawn(cx, async move {
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
        self.optimistic_change(
            PreferenceEdit::Theme {
                scheme,
                theme: Some(theme),
            },
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

/// Installs a loading runtime and returns a task that completes after local
/// state has been resolved and applied on the GPUI foreground.
#[doc(hidden)]
pub fn initialize<L>(
    options: RuntimeOptions<L>,
    cx: &mut App,
) -> Result<Task<StorybookReady>, ResolvePreferencesError>
where
    L: Language,
{
    let repository_options = options.repository.clone();
    let runtime = Runtime::new(options, cx)?;
    cx.set_global(StorybookPreferencesGlobal(Box::new(runtime)));

    let storage_task = gpui_tokio::Tokio::spawn(cx, load_preferences(repository_options, None));

    Ok(cx.spawn(async move |cx: &mut AsyncApp| {
        let loaded = storage_task.await.unwrap_or_else(|_| StartupLoad::Failed {
            repository: None,
            path: None,
            category: "tokio_join".to_owned(),
        });
        cx.update(|cx| {
            cx.update_global::<StorybookPreferencesGlobal, _>(|global, cx| {
                global.0.finish_loading(loaded, cx)
            })
        })
    }))
}

/// Returns the current Storybook preference snapshot when initialized.
pub fn try_state(cx: &App) -> Option<&PreferenceState> {
    cx.try_global::<StorybookPreferencesGlobal>()
        .map(|runtime| runtime.0.state())
}

/// Returns localized available languages in typed application order.
pub(crate) fn available_locales(cx: &App) -> Vec<(String, LanguageTag)> {
    cx.try_global::<StorybookPreferencesGlobal>()
        .map_or_else(Vec::new, |runtime| runtime.0.available_locales(cx))
}

/// Applies a user-selected appearance intent and queues persistence.
pub fn select_color_scheme(value: PreferredColorScheme, cx: &mut App) {
    if cx.try_global::<StorybookPreferencesGlobal>().is_some() {
        cx.update_global::<StorybookPreferencesGlobal, _>(|runtime, cx| {
            runtime.0.select_color_scheme(value, cx);
        });
    }
}

/// Applies a named theme to its independent light or dark slot.
pub fn select_theme(scheme: SystemColorScheme, theme: ThemeId, cx: &mut App) {
    if cx.try_global::<StorybookPreferencesGlobal>().is_some() {
        cx.update_global::<StorybookPreferencesGlobal, _>(|runtime, cx| {
            runtime.0.select_theme(scheme, theme, cx);
        });
    }
}

/// Applies a system or explicit language intent and queues persistence.
pub fn select_language(value: PreferredLanguage, cx: &mut App) {
    if cx.try_global::<StorybookPreferencesGlobal>().is_some() {
        cx.update_global::<StorybookPreferencesGlobal, _>(|runtime, cx| {
            runtime.0.select_language(value, cx);
        });
    }
}

/// Applies a scrollbar policy and queues persistence.
///
/// This is a no-op when the facade preference runtime is not installed.
pub fn select_scrollbar(value: PreferredScrollbar, cx: &mut App) {
    if cx.try_global::<StorybookPreferencesGlobal>().is_some() {
        cx.update_global::<StorybookPreferencesGlobal, _>(|runtime, cx| {
            runtime.0.select_scrollbar(value, cx);
        });
    }
}

/// Retries loading preferences after startup failure or saving dirty intent.
///
/// This is a no-op when the facade preference runtime is not installed.
pub fn retry_preferences(cx: &mut App) {
    if cx.try_global::<StorybookPreferencesGlobal>().is_some() {
        cx.update_global::<StorybookPreferencesGlobal, _>(|runtime, cx| {
            runtime.0.retry_preferences(cx);
        });
    }
}

/// Feeds a live window appearance event into preference resolution.
///
/// This is a no-op when the facade preference runtime is not installed.
pub fn window_appearance_changed(window: &mut Window, cx: &mut App) {
    if cx.try_global::<StorybookPreferencesGlobal>().is_some() {
        cx.update_global::<StorybookPreferencesGlobal, _>(|runtime, cx| {
            runtime.0.window_appearance_changed(window, cx);
        });
    }
}

/// Re-detects system locale and appearance when a window becomes active.
///
/// This is a no-op when the facade preference runtime is not installed.
pub fn window_activated(window: &mut Window, cx: &mut App) {
    if cx.try_global::<StorybookPreferencesGlobal>().is_some() {
        cx.update_global::<StorybookPreferencesGlobal, _>(|runtime, cx| {
            runtime.0.window_activated(window, cx);
        });
    }
}

/// Re-resolves the effective slot after the development theme registry reloads.
pub(crate) fn theme_registry_changed(cx: &mut App) {
    if cx.try_global::<StorybookPreferencesGlobal>().is_some() {
        cx.update_global::<StorybookPreferencesGlobal, _>(|runtime, cx| {
            runtime.0.theme_registry_changed(cx);
        });
    }
}

/// Converts a GPUI window appearance into the platform-independent scheme.
pub fn color_scheme(appearance: WindowAppearance) -> SystemColorScheme {
    match appearance {
        WindowAppearance::Light | WindowAppearance::VibrantLight => SystemColorScheme::Light,
        WindowAppearance::Dark | WindowAppearance::VibrantDark => SystemColorScheme::Dark,
    }
}

fn resolve(
    saved: &StorybookPreferences,
    detected_scheme: SystemColorScheme,
    detected_locales: &DetectedLocales,
    supported_languages: &SupportedLanguages,
    overrides: &ResolutionOverrides,
    cx: &App,
) -> Result<ResolvedPreferences, ResolvePreferencesError> {
    resolve_preferences(
        saved,
        detected_scheme,
        detected_locales,
        supported_languages,
        &RegistryThemes {
            registry: ThemeRegistry::global(cx),
        },
        overrides,
    )
}

fn theme_mode(scheme: SystemColorScheme) -> ThemeMode {
    match scheme {
        SystemColorScheme::Light => ThemeMode::Light,
        SystemColorScheme::Dark => ThemeMode::Dark,
    }
}

fn scrollbar_show(scrollbar: PreferredScrollbar) -> ScrollbarShow {
    match scrollbar {
        PreferredScrollbar::Scrolling => ScrollbarShow::Scrolling,
        PreferredScrollbar::Hover => ScrollbarShow::Hover,
        PreferredScrollbar::Always => ScrollbarShow::Always,
    }
}

fn repository_open_category(error: &RepositoryOpenError) -> &'static str {
    match error {
        RepositoryOpenError::PathOverrideRequiresPersistent { .. } => "path_override",
        RepositoryOpenError::TemporaryDirectoryTask { .. } => "temporary_task",
        RepositoryOpenError::TemporaryDirectory { .. } => "temporary_directory",
        RepositoryOpenError::Clock(_) => "clock",
        RepositoryOpenError::InvalidJsonPath { .. } => "invalid_json_path",
        RepositoryOpenError::PreferenceSchemaPathCollision { .. } => "schema_path_collision",
        RepositoryOpenError::ArchiveInvalidJson { .. } => "archive_invalid_json",
        RepositoryOpenError::JsonIo { .. } => "json_io",
    }
}

fn repository_open_path(error: &RepositoryOpenError) -> Option<PathBuf> {
    match error {
        RepositoryOpenError::InvalidJsonPath { path }
        | RepositoryOpenError::ArchiveInvalidJson { path, .. }
        | RepositoryOpenError::JsonIo { path, .. } => Some(path.clone()),
        RepositoryOpenError::PreferenceSchemaPathCollision {
            preference_path, ..
        } => Some(preference_path.clone()),
        _ => None,
    }
}

fn store_error_category(error: &gpui_storybook_preferences::PreferenceStoreError) -> &'static str {
    use gpui_storybook_preferences::PreferenceStoreError;
    match error {
        PreferenceStoreError::AlreadyExists { .. } => "already_exists",
        PreferenceStoreError::NotFound { .. } => "not_found",
        PreferenceStoreError::Json { .. } => "json",
        PreferenceStoreError::Io { .. } => "io",
    }
}

/// Converts a public persistence selection into repository options.
#[doc(hidden)]
pub fn repository_options(
    consumer_id: gpui_storybook_preferences::ConsumerId,
    persistence: PersistenceMode,
    json_path: Option<PathBuf>,
    project_root: PathBuf,
) -> RepositoryOptions {
    let mut options = match persistence {
        PersistenceMode::Persistent => RepositoryOptions::persistent(consumer_id),
        PersistenceMode::Temporary => RepositoryOptions::temporary(consumer_id),
        PersistenceMode::Disabled => RepositoryOptions::disabled(consumer_id),
    };
    options.json_path = json_path;
    options.project_root = Some(project_root);
    options
}

/// Converts an explicit locale action payload into validated saved intent.
pub(crate) fn explicit_language(identifier: LanguageIdentifier) -> Option<PreferredLanguage> {
    LanguageTag::new(identifier.to_string())
        .ok()
        .map(PreferredLanguage::Explicit)
}

#[cfg(test)]
mod tests {
    use super::*;
    use es_fluent::{FluentMessage, FluentMessageLookup};
    use gpui::{AppContext as _, Entity, px};
    use gpui_component::ActiveTheme as _;
    use std::{
        cell::{Cell, RefCell},
        sync::atomic::{AtomicU64, Ordering},
    };

    static NEXT_TEST_DIRECTORY: AtomicU64 = AtomicU64::new(0);

    #[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
    enum TestLanguage {
        #[default]
        En,
        EnUs,
    }

    impl strum::IntoEnumIterator for TestLanguage {
        type Iterator = std::array::IntoIter<Self, 2>;

        fn iter() -> Self::Iterator {
            [Self::En, Self::EnUs].into_iter()
        }
    }

    impl From<TestLanguage> for LanguageIdentifier {
        fn from(language: TestLanguage) -> Self {
            match language {
                TestLanguage::En => "en".parse().expect("valid English tag"),
                TestLanguage::EnUs => "en-US".parse().expect("valid regional English tag"),
            }
        }
    }

    impl TryFrom<LanguageIdentifier> for TestLanguage {
        type Error = ();

        fn try_from(identifier: LanguageIdentifier) -> Result<Self, Self::Error> {
            match identifier.to_string().as_str() {
                "en" => Ok(Self::En),
                "en-US" => Ok(Self::EnUs),
                _ => Err(()),
            }
        }
    }

    impl FluentMessage for TestLanguage {
        fn to_fluent_string_with(&self, _: &mut FluentMessageLookup<'_>) -> String {
            match self {
                Self::En => "English".to_owned(),
                Self::EnUs => "English (United States)".to_owned(),
            }
        }
    }

    fn test_options(
        initial_scheme: SystemColorScheme,
        overrides: ResolutionOverrides,
        apply_consumer_locale: Rc<dyn Fn(TestLanguage, &mut App) -> Result<(), String>>,
    ) -> RuntimeOptions<TestLanguage> {
        let en = LanguageTag::new("en").expect("valid English tag");
        let en_us = LanguageTag::new("en-US").expect("valid regional English tag");
        RuntimeOptions {
            repository: RepositoryOptions::disabled(
                gpui_storybook_preferences::ConsumerId::new("runtime-test")
                    .expect("valid test consumer"),
            ),
            languages: vec![
                (TestLanguage::En, en.clone()),
                (TestLanguage::EnUs, en_us.clone()),
            ],
            supported_languages: SupportedLanguages::new([en.clone(), en_us], en)
                .expect("supported test languages"),
            locale_detector: Arc::new(gpui_storybook_preferences::FixedLocaleDetector::new(
                DetectedLocales::from_raw(vec!["en".to_owned()]),
            )),
            initial_scheme,
            overrides,
            apply_consumer_locale,
            localize_consumer_language: Rc::new(|language, _| {
                Some(match language {
                    TestLanguage::En => "Consumer English".to_owned(),
                    TestLanguage::EnUs => "Consumer English (United States)".to_owned(),
                })
            }),
        }
    }

    fn init_test_runtime(cx: &mut App) {
        gpui_component::init(cx);
        crate::i18n::init(cx).expect("Storybook test localization initializes");
        ThemeRegistry::global_mut(cx)
            .load_themes_from_str(include_str!("../assets/themes/solarized.json"))
            .expect("Solarized themes load");
    }

    fn load_configured_test_theme(cx: &mut App) {
        ThemeRegistry::global_mut(cx)
            .load_themes_from_str(
                r#"{
                    "name": "Configured test themes",
                    "themes": [{
                        "name": "Configured Light",
                        "mode": "light",
                        "font.size": 13,
                        "radius": 4,
                        "colors": {}
                    }]
                }"#,
            )
            .expect("configured test theme loads");
    }

    fn non_default_preferences() -> StorybookPreferences {
        StorybookPreferences {
            color_scheme: PreferredColorScheme::Light,
            light_theme: Some(ThemeId::new("Solarized Light").expect("valid light theme")),
            dark_theme: Some(ThemeId::new("Solarized Dark").expect("valid dark theme")),
            language: PreferredLanguage::Explicit(
                LanguageTag::new("en").expect("valid English tag"),
            ),
            scrollbar: PreferredScrollbar::Hover,
        }
    }

    fn successful_callback() -> Rc<dyn Fn(TestLanguage, &mut App) -> Result<(), String>> {
        Rc::new(|_, _| Ok(()))
    }

    #[gpui::test]
    fn selecting_non_current_theme_slot_waits_for_scheme_transition(cx: &mut App) {
        init_test_runtime(cx);
        let mut runtime = Runtime::new(
            test_options(
                SystemColorScheme::Light,
                ResolutionOverrides::default(),
                successful_callback(),
            ),
            cx,
        )
        .expect("runtime resolves");
        runtime.apply_resolved(cx);
        let initial_theme = cx.theme().theme_name().clone();

        runtime.save_in_flight = true;
        runtime.select_theme(
            SystemColorScheme::Dark,
            ThemeId::new("Solarized Dark").expect("valid theme"),
            cx,
        );
        assert_eq!(cx.theme().theme_name(), &initial_theme);
        assert_eq!(
            runtime.state.saved.dark_theme.as_ref().map(ThemeId::as_str),
            Some("Solarized Dark")
        );

        runtime.detected_scheme = SystemColorScheme::Dark;
        runtime.resolve_current(cx).expect("dark slot resolves");
        runtime.apply_resolved(cx);
        assert!(cx.theme().mode.is_dark());
        assert_eq!(cx.theme().theme_name().as_ref(), "Solarized Dark");
    }

    #[gpui::test]
    fn unchanged_effective_theme_preserves_runtime_font_and_radius(cx: &mut App) {
        init_test_runtime(cx);
        load_configured_test_theme(cx);
        let mut runtime = Runtime::new(
            test_options(
                SystemColorScheme::Light,
                ResolutionOverrides::default(),
                successful_callback(),
            ),
            cx,
        )
        .expect("runtime resolves");
        runtime.state.saved.light_theme =
            Some(ThemeId::new("Configured Light").expect("valid configured theme"));
        runtime
            .resolve_current(cx)
            .expect("configured theme resolves");
        runtime.apply_resolved(cx);
        assert_eq!(cx.theme().font_size, px(13.));
        assert_eq!(cx.theme().radius, px(4.));

        Theme::global_mut(cx).font_size = px(21.);
        Theme::global_mut(cx).radius = px(11.);
        runtime.save_in_flight = true;
        runtime.select_scrollbar(PreferredScrollbar::Always, cx);
        runtime.select_language(
            PreferredLanguage::Explicit(
                LanguageTag::new("en-US").expect("valid regional English tag"),
            ),
            cx,
        );

        assert_eq!(cx.theme().font_size, px(21.));
        assert_eq!(cx.theme().radius, px(11.));
        assert_eq!(Theme::global(cx).scrollbar_show, ScrollbarShow::Always);
        assert_eq!(
            runtime.applied_theme,
            Some(AppliedTheme {
                scheme: SystemColorScheme::Light,
                theme: ThemeId::new("Configured Light").expect("valid configured theme"),
            })
        );
    }

    #[gpui::test]
    fn theme_registry_change_reapplies_the_same_effective_theme_once(cx: &mut App) {
        init_test_runtime(cx);
        load_configured_test_theme(cx);
        let mut runtime = Runtime::new(
            test_options(
                SystemColorScheme::Light,
                ResolutionOverrides::default(),
                successful_callback(),
            ),
            cx,
        )
        .expect("runtime resolves");
        runtime.state.saved.light_theme =
            Some(ThemeId::new("Configured Light").expect("valid configured theme"));
        runtime
            .resolve_current(cx)
            .expect("configured theme resolves");
        runtime.apply_resolved(cx);

        Theme::global_mut(cx).font_size = px(21.);
        Theme::global_mut(cx).radius = px(11.);
        runtime.theme_registry_changed(cx);
        assert_eq!(cx.theme().font_size, px(13.));
        assert_eq!(cx.theme().radius, px(4.));

        Theme::global_mut(cx).font_size = px(19.);
        runtime.apply_resolved(cx);
        assert_eq!(cx.theme().font_size, px(19.));
    }

    #[test]
    fn preference_edits_coalesce_latest_values_without_replacing_untouched_fields() {
        let baseline = non_default_preferences();
        let mut edits = PreferenceEdits::default();
        edits.record(PreferenceEdit::ColorScheme(PreferredColorScheme::Dark));
        edits.record(PreferenceEdit::Language(PreferredLanguage::Explicit(
            LanguageTag::new("en-US").expect("valid regional English tag"),
        )));
        edits.record(PreferenceEdit::ColorScheme(PreferredColorScheme::System));
        edits.record(PreferenceEdit::Theme {
            scheme: SystemColorScheme::Light,
            theme: None,
        });

        let mut merged = baseline.clone();
        edits.apply_to(&mut merged);
        assert_eq!(merged.color_scheme, PreferredColorScheme::System);
        assert_eq!(merged.light_theme, None);
        assert_eq!(
            merged.language,
            PreferredLanguage::Explicit(
                LanguageTag::new("en-US").expect("valid regional English tag")
            )
        );
        assert_eq!(merged.dark_theme, baseline.dark_theme);
        assert_eq!(merged.scrollbar, baseline.scrollbar);
    }

    #[test]
    fn schema_collision_has_stable_runtime_diagnostics() {
        let preference_path = PathBuf::from("preferences.schema.json");
        let schema_path = preference_path.clone();
        let error = RepositoryOpenError::PreferenceSchemaPathCollision {
            preference_path: preference_path.clone(),
            schema_path,
        };

        assert_eq!(repository_open_category(&error), "schema_path_collision");
        assert_eq!(repository_open_path(&error), Some(preference_path));
    }

    #[gpui::test]
    fn explicit_scheme_and_overrides_ignore_later_detection(cx: &mut App) {
        init_test_runtime(cx);
        let mut runtime = Runtime::new(
            test_options(
                SystemColorScheme::Light,
                ResolutionOverrides::default(),
                successful_callback(),
            ),
            cx,
        )
        .expect("runtime resolves");
        runtime.state.saved.color_scheme = PreferredColorScheme::Dark;
        runtime.detected_scheme = SystemColorScheme::Light;
        runtime
            .resolve_current(cx)
            .expect("explicit scheme resolves");
        assert_eq!(
            runtime.state.resolved.color_scheme.scheme,
            SystemColorScheme::Dark
        );

        runtime.overrides.color_scheme = Some(SystemColorScheme::Light);
        runtime.state.saved.color_scheme = PreferredColorScheme::Dark;
        runtime.resolve_current(cx).expect("override resolves");
        assert_eq!(
            runtime.state.resolved.color_scheme.scheme,
            SystemColorScheme::Light
        );
        assert_eq!(
            runtime.state.resolved.color_scheme.source,
            gpui_storybook_preferences::ColorSchemeSource::Override
        );
    }

    #[gpui::test]
    fn typed_locale_adapter_tracks_initial_and_later_resolved_languages(cx: &mut App) {
        init_test_runtime(cx);
        let applied = Rc::new(RefCell::new(Vec::new()));
        let callback = {
            let applied = Rc::clone(&applied);
            Rc::new(move |language, _: &mut App| {
                applied.borrow_mut().push(language);
                Ok(())
            }) as Rc<dyn Fn(TestLanguage, &mut App) -> Result<(), String>>
        };
        let mut runtime = Runtime::new(
            test_options(
                SystemColorScheme::Light,
                ResolutionOverrides::default(),
                callback,
            ),
            cx,
        )
        .expect("runtime resolves");
        runtime.apply_resolved(cx);
        assert_eq!(applied.borrow().as_slice(), &[TestLanguage::En]);
        assert_eq!(
            cx.global::<crate::language::CurrentLanguage<TestLanguage>>()
                .0,
            TestLanguage::En
        );
        runtime.state.saved.language = PreferredLanguage::Explicit(
            LanguageTag::new("en-US").expect("valid regional English tag"),
        );
        runtime
            .resolve_current(cx)
            .expect("regional English resolves");
        runtime.apply_resolved(cx);
        assert_eq!(
            applied.borrow().as_slice(),
            &[TestLanguage::En, TestLanguage::EnUs]
        );
        assert_eq!(
            cx.global::<crate::language::CurrentLanguage<TestLanguage>>()
                .0,
            TestLanguage::EnUs
        );
    }

    #[gpui::test]
    fn available_locale_labels_use_the_consumer_localizer(cx: &mut App) {
        init_test_runtime(cx);
        let runtime = Runtime::new(
            test_options(
                SystemColorScheme::Light,
                ResolutionOverrides::default(),
                successful_callback(),
            ),
            cx,
        )
        .expect("runtime resolves");

        assert_eq!(
            runtime.available_locales(cx),
            vec![
                (
                    "Consumer English".to_owned(),
                    LanguageTag::new("en").expect("valid English tag"),
                ),
                (
                    "Consumer English (United States)".to_owned(),
                    LanguageTag::new("en-US").expect("valid regional English tag"),
                ),
            ]
        );
    }

    #[gpui::test]
    fn missing_typed_language_mapping_is_diagnostic_without_fallback_substitution(cx: &mut App) {
        init_test_runtime(cx);
        let attempts = Rc::new(Cell::new(0));
        let callback_attempts = attempts.clone();
        let mut runtime = Runtime::new(
            test_options(
                SystemColorScheme::Light,
                ResolutionOverrides::default(),
                Rc::new(move |_, _| {
                    callback_attempts.set(callback_attempts.get() + 1);
                    Ok(())
                }),
            ),
            cx,
        )
        .expect("runtime resolves");
        runtime.languages.clear();

        runtime.apply_resolved(cx);

        assert_eq!(attempts.get(), 0);
        assert_eq!(runtime.applied_language, None);
        assert!(matches!(
            runtime.state.diagnostics.last(),
            Some(PreferenceDiagnostic::LocaleApplicationFailed { category, .. })
                if category == "typed_language_mapping"
        ));
        assert!(
            cx.try_global::<crate::language::CurrentLanguage<TestLanguage>>()
                .is_none()
        );
    }

    #[gpui::test]
    fn locale_failure_is_diagnostic_without_changing_storage_status(cx: &mut App) {
        init_test_runtime(cx);
        let attempts = Rc::new(std::cell::Cell::new(0));
        let callback_attempts = attempts.clone();
        let mut runtime = Runtime::new(
            test_options(
                SystemColorScheme::Light,
                ResolutionOverrides::default(),
                Rc::new(move |_, _| {
                    callback_attempts.set(callback_attempts.get() + 1);
                    Err("consumer_locale".to_owned())
                }),
            ),
            cx,
        )
        .expect("runtime resolves");
        runtime.apply_resolved(cx);
        assert_eq!(attempts.get(), 1);
        assert_eq!(runtime.state.persistence_status, PersistenceStatus::Loading);
        assert_eq!(runtime.applied_language, None);
        assert_eq!(
            cx.global::<crate::language::CurrentLanguage<TestLanguage>>()
                .0,
            TestLanguage::En
        );
        assert_eq!(&*gpui_component::locale(), "en");
        assert!(matches!(
            runtime.state.diagnostics.last(),
            Some(PreferenceDiagnostic::LocaleApplicationFailed { .. })
        ));

        runtime.apply_resolved(cx);
        assert_eq!(attempts.get(), 2);
        assert_eq!(runtime.state.diagnostics.len(), 1);

        runtime.state.persistence_status = PersistenceStatus::Saving;
        runtime.finish_save(Ok(()), cx);
        assert_eq!(runtime.state.persistence_status, PersistenceStatus::Ready);
    }

    #[gpui::test]
    fn save_status_transitions_to_ready_or_error_without_losing_session_state(cx: &mut App) {
        init_test_runtime(cx);
        let mut runtime = Runtime::new(
            test_options(
                SystemColorScheme::Light,
                ResolutionOverrides::default(),
                successful_callback(),
            ),
            cx,
        )
        .expect("runtime resolves");
        runtime.state.persistence_status = PersistenceStatus::Saving;
        runtime.finish_save(Ok(()), cx);
        assert_eq!(runtime.state.persistence_status, PersistenceStatus::Ready);

        runtime.state.persistence_status = PersistenceStatus::Saving;
        runtime.finish_save(Err("io".to_owned()), cx);
        assert_eq!(runtime.state.persistence_status, PersistenceStatus::Error);
        assert!(matches!(
            runtime.state.diagnostics.last(),
            Some(PreferenceDiagnostic::SaveFailed { category }) if category == "io"
        ));
    }

    #[gpui::test]
    fn failed_save_restores_dirty_fields_with_newer_edits_winning(cx: &mut App) {
        init_test_runtime(cx);
        let mut runtime = Runtime::new(
            test_options(
                SystemColorScheme::Light,
                ResolutionOverrides::default(),
                successful_callback(),
            ),
            cx,
        )
        .expect("runtime resolves");
        runtime
            .in_flight_edits
            .record(PreferenceEdit::Scrollbar(PreferredScrollbar::Hover));
        runtime
            .pending_edits
            .record(PreferenceEdit::Scrollbar(PreferredScrollbar::Always));
        runtime
            .pending_edits
            .record(PreferenceEdit::ColorScheme(PreferredColorScheme::Dark));
        runtime.save_in_flight = true;

        runtime.finish_save(Err("io".to_owned()), cx);

        let mut merged = StorybookPreferences::default();
        runtime.pending_edits.apply_to(&mut merged);
        assert_eq!(merged.scrollbar, PreferredScrollbar::Always);
        assert_eq!(merged.color_scheme, PreferredColorScheme::Dark);
        assert!(!runtime.save_in_flight);
        assert_eq!(runtime.state.persistence_status, PersistenceStatus::Error);
    }

    struct NotificationTestView {
        notifications: Entity<gpui_component::notification::NotificationList>,
    }

    impl gpui::Render for NotificationTestView {
        fn render(
            &mut self,
            _: &mut Window,
            _: &mut gpui::Context<Self>,
        ) -> impl gpui::IntoElement {
            self.notifications.clone()
        }
    }

    #[gpui::test]
    fn save_failure_notification_retries_and_dismisses(cx: &mut gpui::TestAppContext) {
        let retry_count = Rc::new(Cell::new(0));
        cx.update(|cx| {
            gpui_component::init(cx);
            let retry_count = retry_count.clone();
            cx.on_action(move |_: &crate::actions::RetryPreferences, _: &mut App| {
                retry_count.set(retry_count.get() + 1);
            });
        });
        let (view, cx) = cx.add_window_view(|window, cx| {
            let notifications =
                cx.new(|cx| gpui_component::notification::NotificationList::new(window, cx));
            NotificationTestView { notifications }
        });
        let notifications = view.read_with(cx, |view, _| view.notifications.clone());

        notifications.update_in(cx, |notifications, window, cx| {
            notifications.push(
                save_failure_notification("save failed".into(), "Retry save".into()),
                window,
                cx,
            );
        });
        cx.run_until_parked();
        assert_eq!(
            notifications.read_with(cx, |notifications, _| notifications.notifications().len()),
            1
        );

        let retry_bounds = cx
            .debug_bounds("retry-preference-save")
            .expect("retry action should be rendered");
        cx.simulate_click(retry_bounds.center(), gpui::Modifiers::none());
        assert_eq!(retry_count.get(), 1);

        cx.background_executor
            .advance_clock(std::time::Duration::from_millis(200));
        cx.run_until_parked();
        assert_eq!(
            notifications.read_with(cx, |notifications, _| notifications.notifications().len()),
            0
        );
    }

    #[gpui::test]
    fn scrollbar_selection_updates_saved_and_resolved_state(cx: &mut App) {
        init_test_runtime(cx);
        let mut runtime = Runtime::new(
            test_options(
                SystemColorScheme::Light,
                ResolutionOverrides::default(),
                successful_callback(),
            ),
            cx,
        )
        .expect("runtime resolves");
        runtime.save_in_flight = true;
        runtime.select_scrollbar(PreferredScrollbar::Always, cx);
        assert_eq!(runtime.state.saved.scrollbar, PreferredScrollbar::Always);
        assert_eq!(runtime.state.resolved.scrollbar, PreferredScrollbar::Always);
        assert_eq!(Theme::global(cx).scrollbar_show, ScrollbarShow::Always);
    }

    #[gpui::test]
    async fn failed_reopen_can_retry_with_an_available_repository(cx: &mut gpui::TestAppContext) {
        cx.executor().allow_parking();
        cx.update(gpui_tokio::init);
        let repository_task = cx.update(|cx| {
            gpui_tokio::Tokio::spawn(cx, async {
                PreferenceRepository::open(RepositoryOptions::disabled(
                    gpui_storybook_preferences::ConsumerId::new("retry-runtime-test")
                        .expect("valid retry consumer"),
                ))
                .await
                .expect("disabled repository opens")
                .repository
            })
        });
        let repository = repository_task.await.expect("repository task should join");

        cx.update(|cx| {
            init_test_runtime(cx);
            let mut runtime = Runtime::new(
                test_options(
                    SystemColorScheme::Light,
                    ResolutionOverrides::default(),
                    successful_callback(),
                ),
                cx,
            )
            .expect("runtime resolves");
            runtime
                .pending_edits
                .record(PreferenceEdit::Scrollbar(PreferredScrollbar::Always));
            cx.set_global(StorybookPreferencesGlobal(Box::new(runtime)));

            cx.update_global::<StorybookPreferencesGlobal, _>(|global, cx| {
                global.0.finish_reopen(
                    RetryOpen::Failed {
                        category: "io".to_owned(),
                    },
                    cx,
                );
            });
            assert_eq!(
                try_state(cx).expect("runtime state").persistence_status,
                PersistenceStatus::Error
            );

            cx.update_global::<StorybookPreferencesGlobal, _>(|global, cx| {
                global.0.finish_reopen(
                    RetryOpen::Opened {
                        repository,
                        saved: StorybookPreferences::default(),
                        recovery: None,
                    },
                    cx,
                );
            });
            assert_eq!(
                try_state(cx).expect("runtime state").persistence_status,
                PersistenceStatus::Saving
            );
        });
    }

    #[gpui::test]
    async fn startup_retry_reloads_existing_intent_without_overwriting_it(
        cx: &mut gpui::TestAppContext,
    ) {
        cx.executor().allow_parking();
        cx.update(gpui_tokio::init);
        let consumer = gpui_storybook_preferences::ConsumerId::new("startup-retry-runtime-test")
            .expect("valid retry consumer");
        let repository_options = RepositoryOptions::disabled(consumer);
        let expected = StorybookPreferences {
            color_scheme: PreferredColorScheme::Dark,
            scrollbar: PreferredScrollbar::Always,
            ..StorybookPreferences::default()
        };
        let expected_for_setup = expected.clone();
        let options_for_setup = repository_options.clone();
        let repository_task = cx.update(|cx| {
            gpui_tokio::Tokio::spawn(cx, async move {
                let repository = PreferenceRepository::open(options_for_setup)
                    .await
                    .expect("disabled repository opens")
                    .repository;
                repository
                    .upsert(expected_for_setup)
                    .await
                    .expect("existing preferences should be stored");
                repository
            })
        });
        let repository = repository_task.await.expect("repository task should join");
        let repository_for_load = repository.clone();
        let retry_load = cx.update(|cx| {
            gpui_tokio::Tokio::spawn(cx, async move {
                load_preferences(repository_options, Some(repository_for_load)).await
            })
        });
        let loaded = retry_load.await.expect("retry load should join");

        cx.update(|cx| {
            init_test_runtime(cx);
            let mut runtime = Runtime::new(
                test_options(
                    SystemColorScheme::Light,
                    ResolutionOverrides::default(),
                    successful_callback(),
                ),
                cx,
            )
            .expect("runtime resolves");
            runtime.state.persistence_status = PersistenceStatus::Error;
            runtime
                .state
                .diagnostics
                .push(PreferenceDiagnostic::LoadFailed {
                    path: None,
                    category: "io".to_owned(),
                });
            assert_eq!(runtime.retry_operation(), RetryOperation::Reload);

            runtime.finish_reload(loaded, cx);
            assert_eq!(runtime.state.saved, expected);
            assert_eq!(runtime.state.persistence_status, PersistenceStatus::Ready);
            assert!(runtime.pending_edits.is_empty());
        });

        let stored_task = cx.update(|cx| {
            gpui_tokio::Tokio::spawn(cx, async move {
                repository
                    .load()
                    .await
                    .expect("stored preferences should remain readable")
            })
        });
        let stored = stored_task
            .await
            .expect("stored preference verification should join")
            .expect("stored preference row should remain present");
        assert_eq!(stored.preferences, expected);
    }

    #[gpui::test]
    async fn reopen_merges_one_local_edit_over_every_loaded_preference(
        cx: &mut gpui::TestAppContext,
    ) {
        cx.executor().allow_parking();
        cx.update(gpui_tokio::init);
        let directory = std::env::temp_dir().join(format!(
            "gpui-storybook-core-{}-{}",
            std::process::id(),
            NEXT_TEST_DIRECTORY.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&directory).expect("temporary test directory creates");
        let path = directory.join("reopen-merge-preferences.json");
        let consumer = gpui_storybook_preferences::ConsumerId::new("reopen-merge-runtime-test")
            .expect("valid reopen merge consumer");
        let mut repository_options = RepositoryOptions::persistent(consumer);
        repository_options.json_path = Some(path.clone());
        let baseline = non_default_preferences();
        let baseline_for_setup = baseline.clone();
        let options_for_setup = repository_options.clone();
        let repository_task = cx.update(|cx| {
            gpui_tokio::Tokio::spawn(cx, async move {
                let repository = PreferenceRepository::open(options_for_setup)
                    .await
                    .expect("persistent repository opens")
                    .repository;
                repository
                    .upsert(baseline_for_setup)
                    .await
                    .expect("existing preferences should be stored");
            })
        });
        repository_task
            .await
            .expect("repository setup task should join");
        let options_for_runtime = repository_options.clone();

        cx.update(|cx| {
            init_test_runtime(cx);
            let mut runtime = Runtime::new(
                test_options(
                    SystemColorScheme::Light,
                    ResolutionOverrides::default(),
                    successful_callback(),
                ),
                cx,
            )
            .expect("runtime resolves");
            runtime.repository_options = options_for_runtime;
            runtime.state.persistence_status = PersistenceStatus::Error;
            runtime
                .state
                .diagnostics
                .push(PreferenceDiagnostic::LoadFailed {
                    path: None,
                    category: "io".to_owned(),
                });
            cx.set_global(StorybookPreferencesGlobal(Box::new(runtime)));
            cx.update_global::<StorybookPreferencesGlobal, _>(|global, cx| {
                global.0.select_scrollbar(PreferredScrollbar::Always, cx);
            });
        });

        let mut expected = baseline;
        expected.scrollbar = PreferredScrollbar::Always;
        let options_for_verification = repository_options;
        let stored_task = cx.update(|cx| {
            gpui_tokio::Tokio::spawn(cx, async move {
                for _ in 0..1_000 {
                    if let Ok(bytes) = tokio::fs::read(&path).await
                        && let Ok(document) = serde_json::from_slice::<serde_json::Value>(&bytes)
                        && document["preferences"]["scrollbar"] == "always"
                    {
                        return PreferenceRepository::open(options_for_verification)
                            .await
                            .expect("merged repository should reopen")
                            .repository
                            .load()
                            .await
                            .expect("stored preferences should remain readable");
                    }
                    tokio::task::yield_now().await;
                }
                panic!("merged preferences should be stored")
            })
        });
        let stored = stored_task
            .await
            .expect("stored preference verification should join")
            .expect("stored preference row should remain present");
        cx.run_until_parked();
        cx.update(|cx| {
            let state = try_state(cx).expect("runtime state");
            assert_eq!(state.saved, expected);
            assert_eq!(state.persistence_status, PersistenceStatus::Ready);
        });
        assert_eq!(stored.preferences, expected);
        std::fs::remove_dir_all(directory).expect("temporary test directory removes");
    }

    #[gpui::test]
    async fn reload_merges_in_flight_edits_over_loaded_untouched_fields(
        cx: &mut gpui::TestAppContext,
    ) {
        cx.executor().allow_parking();
        cx.update(gpui_tokio::init);
        let baseline = non_default_preferences();
        let baseline_for_setup = baseline.clone();
        let repository_task = cx.update(|cx| {
            gpui_tokio::Tokio::spawn(cx, async move {
                let repository = PreferenceRepository::open(RepositoryOptions::disabled(
                    gpui_storybook_preferences::ConsumerId::new("reload-merge-runtime-test")
                        .expect("valid reload merge consumer"),
                ))
                .await
                .expect("disabled repository opens")
                .repository;
                repository
                    .upsert(baseline_for_setup)
                    .await
                    .expect("existing preferences should be stored");
                repository
            })
        });
        let repository = repository_task.await.expect("repository task should join");
        let repository_for_runtime = repository.clone();
        let baseline_for_runtime = baseline.clone();

        cx.update(|cx| {
            init_test_runtime(cx);
            let mut runtime = Runtime::new(
                test_options(
                    SystemColorScheme::Light,
                    ResolutionOverrides::default(),
                    successful_callback(),
                ),
                cx,
            )
            .expect("runtime resolves");
            runtime.save_in_flight = true;
            runtime.state.persistence_status = PersistenceStatus::Loading;
            runtime.select_color_scheme(PreferredColorScheme::Dark, cx);
            runtime.select_color_scheme(PreferredColorScheme::System, cx);
            runtime.select_language(
                PreferredLanguage::Explicit(
                    LanguageTag::new("en-US").expect("valid regional English tag"),
                ),
                cx,
            );
            cx.set_global(StorybookPreferencesGlobal(Box::new(runtime)));

            cx.update_global::<StorybookPreferencesGlobal, _>(|global, cx| {
                global.0.finish_reload(
                    StartupLoad::Loaded {
                        repository: repository_for_runtime,
                        saved: baseline_for_runtime,
                        recovery: None,
                    },
                    cx,
                );
            });
        });

        let mut expected = baseline;
        expected.color_scheme = PreferredColorScheme::System;
        expected.language = PreferredLanguage::Explicit(
            LanguageTag::new("en-US").expect("valid regional English tag"),
        );
        let expected_for_storage = expected.clone();
        let stored_task = cx.update(|cx| {
            gpui_tokio::Tokio::spawn(cx, async move {
                for _ in 0..1_000 {
                    let stored = repository
                        .load()
                        .await
                        .expect("stored preferences should remain readable");
                    if stored.as_ref().map(|record| &record.preferences)
                        == Some(&expected_for_storage)
                    {
                        return stored;
                    }
                    tokio::task::yield_now().await;
                }
                panic!("merged preferences should be stored")
            })
        });
        let stored = stored_task
            .await
            .expect("stored preference verification should join")
            .expect("stored preference row should remain present");
        cx.run_until_parked();
        cx.update(|cx| {
            let state = try_state(cx).expect("runtime state");
            assert_eq!(state.saved, expected);
            assert_eq!(state.persistence_status, PersistenceStatus::Ready);
        });
        assert_eq!(stored.preferences, expected);
    }
}
