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
        .load_themes_from_str(include_str!("../../../assets/themes/solarized.json"))
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
        window_mode: StorybookWindowMode::Dock,
        color_scheme: PreferredColorScheme::Light,
        light_theme: Some(ThemeId::new("Solarized Light").expect("valid light theme")),
        dark_theme: Some(ThemeId::new("Solarized Dark").expect("valid dark theme")),
        language: PreferredLanguage::Explicit(LanguageTag::new("en").expect("valid English tag")),
        scrollbar: PreferredScrollbar::Hover,
    }
}

fn successful_callback() -> Rc<dyn Fn(TestLanguage, &mut App) -> Result<(), String>> {
    Rc::new(|_, _| Ok(()))
}

#[gpui::test]
fn selecting_theme_activates_its_matching_appearance(cx: &mut App) {
    init_test_runtime(cx);
    let mut runtime = Runtime::new(
        test_options(
            SystemColorScheme::Dark,
            ResolutionOverrides::default(),
            successful_callback(),
        ),
        cx,
    )
    .expect("runtime resolves");
    runtime.apply_resolved(cx);
    assert!(cx.theme().mode.is_dark());

    runtime.save_in_flight = true;
    runtime.select_theme(
        SystemColorScheme::Light,
        ThemeId::new("Solarized Light").expect("valid light theme"),
        cx,
    );
    assert_eq!(
        runtime.state.saved.color_scheme,
        PreferredColorScheme::Light
    );
    assert_eq!(
        runtime
            .state
            .saved
            .light_theme
            .as_ref()
            .map(ThemeId::as_str),
        Some("Solarized Light")
    );
    assert_eq!(
        runtime.state.resolved.color_scheme.scheme,
        SystemColorScheme::Light
    );
    assert!(!cx.theme().mode.is_dark());
    assert_eq!(cx.theme().theme_name().as_ref(), "Solarized Light");

    runtime.select_theme(
        SystemColorScheme::Dark,
        ThemeId::new("Solarized Dark").expect("valid dark theme"),
        cx,
    );
    assert_eq!(runtime.state.saved.color_scheme, PreferredColorScheme::Dark);
    assert_eq!(
        runtime.state.saved.dark_theme.as_ref().map(ThemeId::as_str),
        Some("Solarized Dark")
    );
    assert_eq!(
        runtime.state.resolved.color_scheme.scheme,
        SystemColorScheme::Dark
    );
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
        PreferredLanguage::Explicit(LanguageTag::new("en-US").expect("valid regional English tag")),
        cx,
    );

    assert_eq!(cx.theme().font_size, px(21.));
    assert_eq!(cx.theme().radius, px(11.));
    assert_eq!(Theme::global(cx).scrollbar_mode, ScrollbarMode::Always);
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
    edits.record(PreferenceEdit::WindowMode(StorybookWindowMode::Gallery));
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
    assert_eq!(merged.window_mode, StorybookWindowMode::Gallery);
    assert_eq!(merged.color_scheme, PreferredColorScheme::System);
    assert_eq!(merged.light_theme, None);
    assert_eq!(
        merged.language,
        PreferredLanguage::Explicit(LanguageTag::new("en-US").expect("valid regional English tag"))
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
    runtime.state.saved.language =
        PreferredLanguage::Explicit(LanguageTag::new("en-US").expect("valid regional English tag"));
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
    fn render(&mut self, _: &mut Window, _: &mut gpui::Context<Self>) -> impl gpui::IntoElement {
        self.notifications.clone()
    }
}

#[gpui::test]
fn save_failure_notification_retries_and_dismisses(cx: &mut gpui::TestAppContext) {
    let retry_count = Rc::new(Cell::new(0));
    cx.update(|cx| {
        gpui_component::init(cx);
        cx.set_reduce_motion(true);
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
    cx.update(|window, cx| {
        window.activate_window();
        window.draw(cx).clear(cx);
    });

    let retry_bounds = cx
        .debug_bounds("retry-preference-save")
        .expect("retry action should be rendered");
    cx.simulate_click(retry_bounds.center(), gpui::Modifiers::none());
    cx.run_until_parked();
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
    assert_eq!(Theme::global(cx).scrollbar_mode, ScrollbarMode::Always);
}

#[gpui::test]
fn window_mode_selection_updates_saved_state(cx: &mut App) {
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

    runtime.select_window_mode(StorybookWindowMode::Dock, cx);

    assert_eq!(runtime.state.saved.window_mode, StorybookWindowMode::Dock);
    assert_eq!(
        runtime.pending_edits.window_mode,
        Some(StorybookWindowMode::Dock)
    );
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
async fn reopen_merges_one_local_edit_over_every_loaded_preference(cx: &mut gpui::TestAppContext) {
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
    let (save_completion, save_completed) = tokio::sync::oneshot::channel();

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
        runtime.next_save_completion = Some(save_completion);
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
    save_completed
        .await
        .expect("save completion should be reported")
        .expect("merged preferences should be stored");
    let stored_task = cx.update(|cx| {
        gpui_tokio::Tokio::spawn(cx, async move {
            PreferenceRepository::open(options_for_verification)
                .await
                .expect("merged repository should reopen")
                .repository
                .load()
                .await
                .expect("stored preferences should remain readable")
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
async fn reload_merges_in_flight_edits_over_loaded_untouched_fields(cx: &mut gpui::TestAppContext) {
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
    let (save_completion, save_completed) = tokio::sync::oneshot::channel();

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
        runtime.next_save_completion = Some(save_completion);
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
    expected.language =
        PreferredLanguage::Explicit(LanguageTag::new("en-US").expect("valid regional English tag"));
    save_completed
        .await
        .expect("save completion should be reported")
        .expect("merged preferences should be stored");
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
    cx.run_until_parked();
    cx.update(|cx| {
        let state = try_state(cx).expect("runtime state");
        assert_eq!(state.saved, expected);
        assert_eq!(state.persistence_status, PersistenceStatus::Ready);
    });
    assert_eq!(stored.preferences, expected);
}
