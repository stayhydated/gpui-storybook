use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    sync::Arc,
};

use crate::{
    AvailableThemeResolver, ColorSchemeSource, ConsumerId, ConsumerIdError, DetectedLocales,
    FixedLocaleDetector, LanguageSource, LanguageTag, LanguageTagError, LocaleDetector as _,
    MAX_CONSUMER_ID_LEN, MAX_LANGUAGE_TAG_LEN, MAX_THEME_ID_LEN, PersistenceMode, PreferenceClock,
    PreferenceClockError, PreferenceRepository, PreferenceStoreError, PreferredColorScheme,
    PreferredLanguage, PreferredLanguageMode, PreferredScrollbar, RecoveryReason,
    RepositoryOpenError, RepositoryOptions, ResolutionDiagnostic, ResolutionOverrides,
    ResolvePreferencesError, StorybookPreferences, SupportedLanguages, SupportedLanguagesError,
    SystemColorScheme, ThemeId, ThemeIdError, ThemeSource, UnsupportedValueSource,
    persistent_json_path, preference_json_schema, resolve_preferences,
};

const TEST_CONSUMER: &str = "test.storybook";

#[derive(Clone, Copy, Debug)]
struct FixedClock(i64);

impl PreferenceClock for FixedClock {
    fn now_unix_millis(&self) -> Result<i64, PreferenceClockError> {
        Ok(self.0)
    }
}

#[derive(Clone, Copy, Debug)]
struct FailingClock;

impl PreferenceClock for FailingClock {
    fn now_unix_millis(&self) -> Result<i64, PreferenceClockError> {
        Err(PreferenceClockError::BeforeUnixEpoch)
    }
}

#[derive(Debug)]
struct TestThemes {
    light: HashSet<ThemeId>,
    dark: HashSet<ThemeId>,
    light_fallback: Option<ThemeId>,
    dark_fallback: Option<ThemeId>,
}

impl TestThemes {
    fn standard() -> Self {
        Self {
            light: [theme("light-default"), theme("light-paper")]
                .into_iter()
                .collect(),
            dark: [theme("dark-default"), theme("dark-ocean")]
                .into_iter()
                .collect(),
            light_fallback: Some(theme("light-default")),
            dark_fallback: Some(theme("dark-default")),
        }
    }
}

impl AvailableThemeResolver for TestThemes {
    fn is_available(&self, scheme: SystemColorScheme, theme: &ThemeId) -> bool {
        match scheme {
            SystemColorScheme::Light => self.light.contains(theme),
            SystemColorScheme::Dark => self.dark.contains(theme),
        }
    }

    fn fallback(&self, scheme: SystemColorScheme) -> Option<ThemeId> {
        match scheme {
            SystemColorScheme::Light => self.light_fallback.clone(),
            SystemColorScheme::Dark => self.dark_fallback.clone(),
        }
    }
}

fn consumer(value: &str) -> ConsumerId {
    value.parse().expect("test consumer id is valid")
}

fn theme(value: &str) -> ThemeId {
    value.parse().expect("test theme id is valid")
}

fn language(value: &str) -> LanguageTag {
    value.parse().expect("test language tag is valid")
}

fn supported_languages() -> SupportedLanguages {
    SupportedLanguages::new(
        [language("en-US"), language("fr"), language("zh-Hant")],
        language("en-US"),
    )
    .expect("test language set is valid")
}

fn saved_preferences() -> StorybookPreferences {
    StorybookPreferences {
        color_scheme: PreferredColorScheme::System,
        light_theme: Some(theme("light-paper")),
        dark_theme: Some(theme("dark-ocean")),
        language: PreferredLanguage::Explicit(language("fr")),
        scrollbar: PreferredScrollbar::Always,
    }
}

fn persistent_options(
    path: impl Into<PathBuf>,
    consumer_id: &str,
    clock: Arc<dyn PreferenceClock>,
) -> RepositoryOptions {
    let mut options = RepositoryOptions::persistent(consumer(consumer_id));
    options.json_path = Some(path.into());
    options.clock = clock;
    options
}

#[test]
fn preference_json_schema_is_derived_from_the_typed_document() {
    let schema = preference_json_schema();
    assert_eq!(
        schema["$schema"],
        "https://json-schema.org/draft/2020-12/schema"
    );
    assert_eq!(schema["title"], "GPUI Storybook Preferences");
    assert_eq!(
        schema["properties"]["$schema"]["description"],
        "Relative path to the schema that describes this document."
    );
    assert_eq!(
        schema["properties"]["consumer_id"]["$ref"],
        "#/$defs/ConsumerId"
    );
    let properties = schema["properties"]
        .as_object()
        .expect("schema properties should be an object");
    assert!(!properties.contains_key("format_version"));
    assert!(!properties.contains_key("created_at_millis"));
    assert!(!properties.contains_key("updated_at_millis"));
    assert_eq!(
        schema["properties"]["preferences"]["$ref"],
        "#/$defs/StorybookPreferences"
    );
    assert_eq!(schema["$defs"]["StorybookPreferences"]["type"], "object");
    assert_eq!(schema["$defs"]["ConsumerId"]["type"], "string");
    for field in ["light_theme", "dark_theme"] {
        assert_eq!(
            schema["$defs"]["StorybookPreferences"]["properties"][field]["anyOf"][0]["$ref"],
            "#/$defs/ThemeId"
        );
    }
    assert_eq!(schema["$defs"]["ThemeId"]["type"], "string");
    assert_eq!(schema["$defs"]["ThemeId"]["maxLength"], MAX_THEME_ID_LEN);
    assert_eq!(
        schema["$defs"]["PreferredLanguage"]["oneOf"][1]["properties"]["tag"]["$ref"],
        "#/$defs/LanguageTag"
    );
    assert_eq!(schema["$defs"]["LanguageTag"]["type"], "string");
    assert_eq!(schema["$defs"]["LanguageTag"]["format"], "language-tag");
    assert_eq!(
        schema["$defs"]["LanguageTag"]["maxLength"],
        MAX_LANGUAGE_TAG_LEN
    );
    for definition in ["ConsumerId", "ThemeId", "LanguageTag"] {
        assert!(
            schema["$defs"][definition]["description"]
                .as_str()
                .is_some_and(|description| !description.is_empty()),
            "{definition} should have a schema description"
        );
    }
    assert!(
        schema["$defs"]["StorybookPreferences"]["required"]
            .as_array()
            .is_some_and(|required| required.iter().any(|field| field == "color_scheme"))
    );
}

#[test]
fn typed_values_normalize_and_reject_invalid_storage_tokens() {
    assert_eq!(consumer(TEST_CONSUMER).as_str(), TEST_CONSUMER);
    assert_eq!(ConsumerId::new(""), Err(ConsumerIdError::Empty));
    assert_eq!(
        ConsumerId::new("a".repeat(MAX_CONSUMER_ID_LEN + 1)),
        Err(ConsumerIdError::TooLong {
            max: MAX_CONSUMER_ID_LEN,
        })
    );
    assert_eq!(
        ConsumerId::new("-consumer"),
        Err(ConsumerIdError::InvalidStart)
    );
    assert_eq!(
        ConsumerId::new("consumer_"),
        Err(ConsumerIdError::InvalidEnd)
    );
    assert_eq!(
        ConsumerId::new("Consumer"),
        Err(ConsumerIdError::InvalidStart)
    );
    assert_eq!(
        ConsumerId::new("cOnsumer"),
        Err(ConsumerIdError::InvalidCharacter { index: 1 })
    );
    assert_eq!(
        ConsumerId::new("consumer/path"),
        Err(ConsumerIdError::InvalidCharacter { index: 8 })
    );

    assert_eq!(theme("  ocean dusk  ").as_str(), "ocean dusk");
    assert_eq!(ThemeId::new(""), Err(ThemeIdError::Empty));
    assert_eq!(
        ThemeId::new("x".repeat(MAX_THEME_ID_LEN + 1)),
        Err(ThemeIdError::TooLong {
            max: MAX_THEME_ID_LEN,
        })
    );
    assert_eq!(
        ThemeId::new("bad\nname"),
        Err(ThemeIdError::ControlCharacter)
    );

    assert_eq!(language("  zh-hant-tw  ").to_string(), "zh-Hant-TW");
    assert!(matches!(LanguageTag::new(""), Err(LanguageTagError::Empty)));
    assert!(matches!(
        LanguageTag::new("x".repeat(MAX_LANGUAGE_TAG_LEN + 1)),
        Err(LanguageTagError::TooLong {
            max: MAX_LANGUAGE_TAG_LEN,
        })
    ));
    assert!(matches!(
        LanguageTag::new("not a language"),
        Err(LanguageTagError::Invalid { .. })
    ));

    assert_eq!(PreferredColorScheme::System.token(), "system");
    assert_eq!(PreferredColorScheme::Light.token(), "light");
    assert_eq!(PreferredColorScheme::Dark.token(), "dark");
    assert_eq!("dark".parse(), Ok(PreferredColorScheme::Dark));
    assert!("Dark".parse::<PreferredColorScheme>().is_err());
    assert_eq!(PreferredLanguageMode::Explicit.token(), "explicit");
    assert_eq!("system".parse(), Ok(PreferredLanguageMode::System));
    assert_eq!(PreferredScrollbar::Scrolling.token(), "scrolling");
    assert_eq!(PreferredScrollbar::Hover.token(), "hover");
    assert_eq!(PreferredScrollbar::Always.token(), "always");
    assert_eq!("always".parse(), Ok(PreferredScrollbar::Always));
    assert_eq!(PersistenceMode::Temporary.token(), "temporary");
    assert_eq!("disabled".parse(), Ok(PersistenceMode::Disabled));
}

#[test]
fn supported_language_contract_requires_an_embedded_fallback() {
    assert!(matches!(
        SupportedLanguages::new([], language("en-US")),
        Err(SupportedLanguagesError::Empty)
    ));
    assert!(matches!(
        SupportedLanguages::new([language("fr")], language("en-US")),
        Err(SupportedLanguagesError::UnsupportedFallback { .. })
    ));

    let supported = SupportedLanguages::new(
        [language("fr"), language("fr"), language("en-US")],
        language("en-US"),
    )
    .expect("fallback is in the set");
    assert_eq!(supported.available(), [language("fr"), language("en-US")]);
    assert_eq!(supported.fallback(), &language("en-US"));
}

#[test]
fn injected_locale_detector_validates_ordered_bcp47_values_without_leaking_host_state() {
    let locales = DetectedLocales::from_raw(vec![
        " fr-CA ".to_owned(),
        "zh-Hant-TW".to_owned(),
        "en-US".to_owned(),
        "fr-CA".to_owned(),
        String::new(),
        ".".to_owned(),
        "not a language".to_owned(),
    ]);
    assert_eq!(
        locales.candidates,
        [language("fr-CA"), language("zh-Hant-TW"), language("en-US")]
    );
    assert_eq!(locales.rejected_count, 3);

    let locale_detector = FixedLocaleDetector::new(locales.clone());
    assert_eq!(locale_detector.detect(), locales);
}

#[test]
fn system_and_explicit_intent_resolve_against_injected_system_state() {
    let themes = TestThemes::standard();
    let languages = supported_languages();
    let locales = DetectedLocales::from_raw(vec!["en-US".to_owned()]);
    let saved = saved_preferences();

    let dark = resolve_preferences(
        &saved,
        SystemColorScheme::Dark,
        &locales,
        &languages,
        &themes,
        &ResolutionOverrides::default(),
    )
    .expect("dark system intent resolves");
    assert_eq!(dark.color_scheme.scheme, SystemColorScheme::Dark);
    assert_eq!(dark.color_scheme.source, ColorSchemeSource::System);
    assert_eq!(dark.theme.theme, theme("dark-ocean"));
    assert_eq!(dark.theme.source, ThemeSource::Saved);
    assert_eq!(dark.language.language, language("fr"));
    assert_eq!(dark.language.source, LanguageSource::Explicit);
    assert_eq!(dark.scrollbar, PreferredScrollbar::Always);
    assert!(dark.diagnostics.is_empty());

    let light = resolve_preferences(
        &saved,
        SystemColorScheme::Light,
        &locales,
        &languages,
        &themes,
        &ResolutionOverrides::default(),
    )
    .expect("light system intent resolves");
    assert_eq!(light.theme.theme, theme("light-paper"));
    assert_eq!(light.theme.source, ThemeSource::Saved);

    let explicit = StorybookPreferences {
        color_scheme: PreferredColorScheme::Light,
        language: PreferredLanguage::Explicit(language("zh-Hant")),
        ..saved
    };
    let explicit = resolve_preferences(
        &explicit,
        SystemColorScheme::Dark,
        &DetectedLocales::from_raw(vec!["ja-JP".to_owned()]),
        &languages,
        &themes,
        &ResolutionOverrides::default(),
    )
    .expect("explicit intent ignores later system changes");
    assert_eq!(explicit.color_scheme.scheme, SystemColorScheme::Light);
    assert_eq!(explicit.color_scheme.source, ColorSchemeSource::Explicit);
    assert_eq!(explicit.theme.theme, theme("light-paper"));
    assert_eq!(explicit.language.language, language("zh-Hant"));
    assert_eq!(explicit.language.source, LanguageSource::Explicit);
}

#[test]
fn fluent_lookup_uses_order_region_script_primary_and_configured_fallback() {
    let themes = TestThemes::standard();
    let languages = supported_languages();
    let saved = StorybookPreferences::default();

    let exact = resolve_preferences(
        &saved,
        SystemColorScheme::Light,
        &DetectedLocales::from_raw(vec!["en-US".to_owned(), "fr-FR".to_owned()]),
        &languages,
        &themes,
        &ResolutionOverrides::default(),
    )
    .expect("exact locale resolves");
    assert_eq!(exact.language.language, language("en-US"));
    assert_eq!(exact.language.source, LanguageSource::System);

    let script = resolve_preferences(
        &saved,
        SystemColorScheme::Light,
        &DetectedLocales::from_raw(vec!["zh-Hant-TW".to_owned()]),
        &languages,
        &themes,
        &ResolutionOverrides::default(),
    )
    .expect("script-aware locale resolves");
    assert_eq!(script.language.language, language("zh-Hant"));
    assert_eq!(script.language.source, LanguageSource::System);

    let primary_and_ordered = resolve_preferences(
        &saved,
        SystemColorScheme::Light,
        &DetectedLocales::from_raw(vec!["es-MX".to_owned(), "fr-CA".to_owned()]),
        &languages,
        &themes,
        &ResolutionOverrides::default(),
    )
    .expect("later supported platform locale resolves");
    assert_eq!(primary_and_ordered.language.language, language("fr"));
    assert_eq!(primary_and_ordered.language.source, LanguageSource::System);

    let detected = DetectedLocales::from_raw(vec![
        "C".to_owned(),
        "bad locale".to_owned(),
        "ja-JP".to_owned(),
    ]);
    let fallback = resolve_preferences(
        &saved,
        SystemColorScheme::Light,
        &detected,
        &languages,
        &themes,
        &ResolutionOverrides::default(),
    )
    .expect("unsupported locales use configured fallback");
    assert_eq!(fallback.language.language, language("en-US"));
    assert_eq!(fallback.language.source, LanguageSource::Fallback);
    assert_eq!(
        fallback.diagnostics,
        [ResolutionDiagnostic::NoSupportedSystemLocale {
            fallback: language("en-US"),
            rejected_count: 2,
        }]
    );
}

#[test]
fn deterministic_overrides_and_missing_registry_values_are_typed() {
    let themes = TestThemes::standard();
    let languages = supported_languages();
    let mut saved = saved_preferences();
    saved.color_scheme = PreferredColorScheme::Light;
    let overrides = ResolutionOverrides {
        color_scheme: Some(SystemColorScheme::Dark),
        theme: Some(theme("missing-capture-theme")),
        language: Some(language("zh-Hant")),
    };

    let resolved = resolve_preferences(
        &saved,
        SystemColorScheme::Light,
        &DetectedLocales::default(),
        &languages,
        &themes,
        &overrides,
    )
    .expect("deterministic overrides resolve");
    assert_eq!(resolved.color_scheme.scheme, SystemColorScheme::Dark);
    assert_eq!(resolved.color_scheme.source, ColorSchemeSource::Override);
    assert_eq!(resolved.theme.theme, theme("dark-default"));
    assert_eq!(resolved.theme.source, ThemeSource::Fallback);
    assert_eq!(resolved.language.language, language("zh-Hant"));
    assert_eq!(resolved.language.source, LanguageSource::Override);
    assert_eq!(
        resolved.diagnostics,
        [ResolutionDiagnostic::MissingTheme {
            scheme: SystemColorScheme::Dark,
            requested: theme("missing-capture-theme"),
            fallback: theme("dark-default"),
            source: UnsupportedValueSource::Override,
        }]
    );

    saved.color_scheme = PreferredColorScheme::Dark;
    saved.dark_theme = Some(theme("removed-dark-theme"));
    saved.language = PreferredLanguage::Explicit(language("de-DE"));
    let resolved = resolve_preferences(
        &saved,
        SystemColorScheme::Light,
        &DetectedLocales::default(),
        &languages,
        &themes,
        &ResolutionOverrides::default(),
    )
    .expect("removed saved values retain diagnostics and use fallbacks");
    assert_eq!(resolved.theme.theme, theme("dark-default"));
    assert_eq!(resolved.language.language, language("en-US"));
    assert_eq!(
        resolved.diagnostics,
        [
            ResolutionDiagnostic::MissingTheme {
                scheme: SystemColorScheme::Dark,
                requested: theme("removed-dark-theme"),
                fallback: theme("dark-default"),
                source: UnsupportedValueSource::Saved,
            },
            ResolutionDiagnostic::UnsupportedLanguage {
                requested: language("de-DE"),
                fallback: language("en-US"),
                source: UnsupportedValueSource::Saved,
            },
        ]
    );

    let unavailable_fallbacks = TestThemes {
        light: HashSet::new(),
        dark: HashSet::new(),
        light_fallback: Some(theme("not-registered")),
        dark_fallback: None,
    };
    assert_eq!(
        resolve_preferences(
            &StorybookPreferences::default(),
            SystemColorScheme::Light,
            &DetectedLocales::default(),
            &languages,
            &unavailable_fallbacks,
            &ResolutionOverrides::default(),
        ),
        Err(ResolvePreferencesError::MissingFallbackTheme {
            scheme: SystemColorScheme::Light,
        })
    );

    assert_eq!(ColorSchemeSource::Override.token(), "override");
    assert_eq!(ThemeSource::Fallback.token(), "fallback");
    assert_eq!(LanguageSource::System.token(), "system");
    assert_eq!(UnsupportedValueSource::Saved.token(), "saved");
}

#[tokio::test]
async fn json_repository_supports_typed_crud_reopen_and_generated_schema() {
    let directory = tempfile::tempdir().expect("temporary directory creates");
    let path = directory.path().join("test.storybook.json");
    let clock: Arc<dyn PreferenceClock> = Arc::new(FixedClock(1_000));
    let options = persistent_options(&path, TEST_CONSUMER, clock);

    let opened = PreferenceRepository::open(options.clone())
        .await
        .expect("JSON repository opens");
    assert!(opened.recovery.is_none());
    let repository = opened.repository;
    assert_eq!(repository.persistence(), PersistenceMode::Persistent);
    assert_eq!(repository.path(), Some(path.as_path()));
    let schema_path = directory.path().join("preferences.schema.json");
    assert_eq!(repository.schema_path(), Some(schema_path.as_path()));
    assert_eq!(
        tokio::fs::read_to_string(&schema_path)
            .await
            .expect("generated schema reads"),
        crate::preference_json_schema_pretty()
    );
    assert_eq!(
        repository.load().await.expect("empty repository loads"),
        None
    );

    let created = repository
        .create(saved_preferences())
        .await
        .expect("typed preferences create");
    assert_eq!(created.preferences, saved_preferences());
    assert!(matches!(
        repository.create(saved_preferences()).await,
        Err(PreferenceStoreError::AlreadyExists { .. })
    ));

    let document: serde_json::Value =
        serde_json::from_slice(&tokio::fs::read(&path).await.expect("preference JSON reads"))
            .expect("preference JSON parses");
    assert_eq!(document["$schema"], "preferences.schema.json");
    assert_eq!(document["consumer_id"], TEST_CONSUMER);
    assert_eq!(document.get("format_version"), None);
    assert_eq!(document.get("created_at_millis"), None);
    assert_eq!(document.get("updated_at_millis"), None);
    assert_eq!(document["preferences"]["color_scheme"], "system");
    assert_eq!(document["preferences"]["language"]["mode"], "explicit");
    assert_eq!(document["preferences"]["language"]["tag"], "fr");

    let mut changed = saved_preferences();
    changed.color_scheme = PreferredColorScheme::Dark;
    let updated = repository
        .update(changed.clone())
        .await
        .expect("typed preferences update");
    assert_eq!(updated.preferences, changed);

    let reopened = PreferenceRepository::open(options)
        .await
        .expect("JSON repository reopens")
        .repository;
    assert_eq!(
        reopened
            .load()
            .await
            .expect("reopened JSON loads")
            .expect("saved document exists")
            .preferences,
        changed
    );
    assert!(reopened.delete().await.expect("saved JSON deletes"));
    assert!(!path.exists());
    assert!(schema_path.exists());
    assert!(!reopened.delete().await.expect("missing JSON stays deleted"));
}

#[tokio::test]
async fn invalid_json_is_archived_byte_for_byte_and_defaults_remain_available() {
    let directory = tempfile::tempdir().expect("temporary directory creates");
    let path = directory.path().join("preferences.json");
    let invalid = br#"{"consumer_id":"test.storybook","unexpected":true}"#;
    tokio::fs::write(&path, invalid)
        .await
        .expect("invalid JSON fixture writes");

    let opened = PreferenceRepository::open(persistent_options(
        &path,
        TEST_CONSUMER,
        Arc::new(FixedClock(7_654_321)),
    ))
    .await
    .expect("invalid JSON recovers");
    let diagnostic = opened.recovery.expect("recovery is reported");
    let archived_path = directory.path().join("preferences.json.corrupt-7654321");
    assert_eq!(diagnostic.json_path, path);
    assert_eq!(diagnostic.archived_path, archived_path);
    assert_eq!(diagnostic.reason, RecoveryReason::InvalidJson);
    assert_eq!(diagnostic.reason.token(), "invalid_json");
    assert_eq!(
        tokio::fs::read(&diagnostic.archived_path)
            .await
            .expect("archived bytes read"),
        invalid
    );
    assert_eq!(
        opened
            .repository
            .load()
            .await
            .expect("recovered repository loads"),
        None
    );
    assert!(opened.repository.schema_path().is_some_and(Path::exists));
}

#[tokio::test]
async fn explicit_schema_path_collision_preserves_the_preference_file() {
    let directory = tempfile::tempdir().expect("temporary directory creates");
    let path = directory.path().join("preferences.schema.json");
    let original = br#"{"consumer_id":"test.storybook","keep":"these bytes"}"#;
    tokio::fs::write(&path, original)
        .await
        .expect("preference fixture writes");

    let error = PreferenceRepository::open(persistent_options(
        &path,
        TEST_CONSUMER,
        Arc::new(FixedClock(1_234)),
    ))
    .await
    .expect_err("schema path collision is rejected");
    match error {
        RepositoryOpenError::PreferenceSchemaPathCollision {
            preference_path,
            schema_path,
        } => {
            assert_eq!(preference_path, path);
            assert_eq!(schema_path, path);
        },
        other => panic!("expected schema path collision, got {other:?}"),
    }
    assert_eq!(
        tokio::fs::read(&path)
            .await
            .expect("original preference bytes remain readable"),
        original
    );
    let mut entries = tokio::fs::read_dir(directory.path())
        .await
        .expect("directory inventory opens");
    assert_eq!(
        entries
            .next_entry()
            .await
            .expect("directory inventory reads")
            .map(|entry| entry.path()),
        Some(path)
    );
    assert!(
        entries
            .next_entry()
            .await
            .expect("directory inventory completes")
            .is_none()
    );
}

#[tokio::test]
async fn default_schema_path_collision_precedes_schema_write_and_recovery() {
    let directory = tempfile::tempdir().expect("temporary directory creates");
    let storybook_directory = directory.path().join(".gpui-storybook");
    tokio::fs::create_dir(&storybook_directory)
        .await
        .expect("Storybook directory creates");
    let path = storybook_directory.join("preferences.schema.json");
    let original = b"invalid preference bytes";
    tokio::fs::write(&path, original)
        .await
        .expect("preference fixture writes");
    let mut options = RepositoryOptions::persistent(consumer("preferences.schema"));
    options.project_root = Some(directory.path().to_path_buf());
    options.clock = Arc::new(FixedClock(9_876));

    let error = PreferenceRepository::open(options)
        .await
        .expect_err("default schema path collision is rejected");
    assert!(matches!(
        error,
        RepositoryOpenError::PreferenceSchemaPathCollision {
            preference_path,
            schema_path,
        } if preference_path == path && schema_path == path
    ));
    assert_eq!(
        tokio::fs::read(&path)
            .await
            .expect("original preference bytes remain readable"),
        original
    );
    assert!(!storybook_directory.join(".gitignore").exists());
    let mut entries = tokio::fs::read_dir(&storybook_directory)
        .await
        .expect("Storybook directory inventory opens");
    assert_eq!(
        entries
            .next_entry()
            .await
            .expect("Storybook directory inventory reads")
            .map(|entry| entry.path()),
        Some(path)
    );
    assert!(
        entries
            .next_entry()
            .await
            .expect("Storybook directory inventory completes")
            .is_none()
    );
}

#[tokio::test]
async fn schema_path_collision_check_is_case_insensitive() {
    let directory = tempfile::tempdir().expect("temporary directory creates");
    let preference_path = directory.path().join("Preferences.Schema.JSON");
    let schema_path = directory.path().join("preferences.schema.json");

    let error = PreferenceRepository::open(persistent_options(
        &preference_path,
        TEST_CONSUMER,
        Arc::new(FixedClock(5_678)),
    ))
    .await
    .expect_err("case-only schema path collision is rejected");
    assert!(matches!(
        error,
        RepositoryOpenError::PreferenceSchemaPathCollision {
            preference_path: actual_preference_path,
            schema_path: actual_schema_path,
        } if actual_preference_path == preference_path && actual_schema_path == schema_path
    ));
    assert!(!preference_path.exists());
    assert!(!schema_path.exists());
}

#[tokio::test]
async fn a_document_for_another_consumer_is_recovered_as_invalid_json() {
    let directory = tempfile::tempdir().expect("temporary directory creates");
    let path = directory.path().join("preferences.json");
    let first = PreferenceRepository::open(persistent_options(
        &path,
        "first.storybook",
        Arc::new(FixedClock(10)),
    ))
    .await
    .expect("first repository opens")
    .repository;
    first
        .upsert(saved_preferences())
        .await
        .expect("first consumer saves");
    drop(first);

    let second = PreferenceRepository::open(persistent_options(
        &path,
        "second.storybook",
        Arc::new(FixedClock(11)),
    ))
    .await
    .expect("consumer mismatch recovers");
    assert_eq!(
        second.recovery.as_ref().map(|value| value.reason),
        Some(RecoveryReason::InvalidJson)
    );
    assert_eq!(
        second
            .repository
            .load()
            .await
            .expect("second repository loads"),
        None
    );
}

#[tokio::test]
async fn persistence_modes_and_json_path_contracts_are_explicit_and_host_safe() {
    let directory = tempfile::tempdir().expect("temporary directory creates");
    let project_root = directory.path().join("workspace");
    tokio::fs::create_dir(&project_root)
        .await
        .expect("project root creates");
    let standard_path = persistent_json_path(&project_root, &consumer(TEST_CONSUMER));
    assert_eq!(
        standard_path,
        project_root
            .join(".gpui-storybook")
            .join("test.storybook.json")
    );
    assert_eq!(
        standard_path.file_name().and_then(|value| value.to_str()),
        Some("test.storybook.json")
    );
    assert!(
        standard_path
            .parent()
            .expect("persistent file has a parent")
            .ends_with(".gpui-storybook")
    );

    let mut default_path_options = RepositoryOptions::persistent(consumer(TEST_CONSUMER));
    default_path_options.project_root = Some(project_root.clone());
    let persistent = PreferenceRepository::open(default_path_options)
        .await
        .expect("default persistent repository opens")
        .repository;
    assert_eq!(persistent.path(), Some(standard_path.as_path()));
    assert_eq!(
        persistent.schema_path(),
        Some(
            project_root
                .join(".gpui-storybook/preferences.schema.json")
                .as_path()
        )
    );
    let gitignore_path = project_root.join(".gpui-storybook/.gitignore");
    assert_eq!(
        tokio::fs::read_to_string(&gitignore_path)
            .await
            .expect("generated gitignore reads"),
        "*\n"
    );
    tokio::fs::write(&gitignore_path, "# consumer-owned\n")
        .await
        .expect("custom gitignore writes");
    let mut second_options = RepositoryOptions::persistent(consumer("second.storybook"));
    second_options.project_root = Some(project_root);
    let second = PreferenceRepository::open(second_options)
        .await
        .expect("second persistent repository opens")
        .repository;
    assert_eq!(second.schema_path(), persistent.schema_path());
    assert_eq!(
        tokio::fs::read_to_string(gitignore_path)
            .await
            .expect("custom gitignore remains readable"),
        "# consumer-owned\n"
    );

    let temporary =
        PreferenceRepository::open(RepositoryOptions::temporary(consumer(TEST_CONSUMER)))
            .await
            .expect("temporary JSON repository opens")
            .repository;
    let temporary_path = temporary.path().expect("temporary JSON path").to_path_buf();
    let temporary_schema = temporary
        .schema_path()
        .expect("temporary schema path")
        .to_path_buf();
    assert!(temporary_schema.exists());
    temporary
        .upsert(saved_preferences())
        .await
        .expect("temporary JSON saves");
    assert!(temporary_path.exists());
    drop(temporary);
    assert!(!temporary_path.exists());
    assert!(!temporary_schema.exists());

    let disabled = PreferenceRepository::open(RepositoryOptions::disabled(consumer(TEST_CONSUMER)))
        .await
        .expect("disabled repository opens")
        .repository;
    assert_eq!(disabled.path(), None);
    assert_eq!(disabled.schema_path(), None);
    disabled
        .upsert(saved_preferences())
        .await
        .expect("disabled mode keeps typed memory state");
    assert_eq!(
        disabled
            .load()
            .await
            .expect("disabled mode loads")
            .expect("memory state exists")
            .preferences,
        saved_preferences()
    );

    let mut invalid_options = RepositoryOptions::temporary(consumer(TEST_CONSUMER));
    invalid_options.json_path = Some(PathBuf::from("portable/preferences.json"));
    assert!(matches!(
        PreferenceRepository::open(invalid_options).await,
        Err(RepositoryOpenError::PathOverrideRequiresPersistent {
            persistence: PersistenceMode::Temporary,
        })
    ));

    let failing_directory = tempfile::tempdir().expect("temporary directory creates");
    let failing_path = failing_directory.path().join("preferences.json");
    tokio::fs::write(&failing_path, b"{}")
        .await
        .expect("invalid JSON fixture writes");
    assert!(matches!(
        PreferenceRepository::open(persistent_options(
            &failing_path,
            TEST_CONSUMER,
            Arc::new(FailingClock),
        ))
        .await,
        Err(RepositoryOpenError::Clock(
            PreferenceClockError::BeforeUnixEpoch
        ))
    ));
}

#[tokio::test]
async fn ordinary_filesystem_failures_do_not_archive_unrelated_input() {
    let directory = tempfile::tempdir().expect("temporary directory creates");
    let blocking_parent = directory.path().join("not-a-directory");
    tokio::fs::write(&blocking_parent, b"keep me")
        .await
        .expect("blocking file writes");
    let nested_json = blocking_parent.join("preferences.json");

    let error = PreferenceRepository::open(persistent_options(
        &nested_json,
        TEST_CONSUMER,
        Arc::new(FixedClock(4_242)),
    ))
    .await
    .expect_err("directory preparation fails");
    assert!(matches!(error, RepositoryOpenError::JsonIo { .. }));
    assert_eq!(
        tokio::fs::read(&blocking_parent)
            .await
            .expect("blocking file remains"),
        b"keep me"
    );
    let mut entries = tokio::fs::read_dir(directory.path())
        .await
        .expect("directory inventory opens");
    let entry = entries
        .next_entry()
        .await
        .expect("directory inventory reads")
        .expect("blocking file remains");
    assert_eq!(entry.path(), blocking_parent);
    assert!(
        entries
            .next_entry()
            .await
            .expect("directory inventory completes")
            .is_none()
    );
}

#[tokio::test]
async fn concurrent_writes_leave_one_complete_typed_document() {
    let directory = tempfile::tempdir().expect("temporary directory creates");
    let path = directory.path().join("preferences.json");
    let repository = PreferenceRepository::open(persistent_options(
        &path,
        TEST_CONSUMER,
        Arc::new(FixedClock(20)),
    ))
    .await
    .expect("JSON repository opens")
    .repository;

    let first = {
        let repository = repository.clone();
        let mut preferences = saved_preferences();
        preferences.color_scheme = PreferredColorScheme::Light;
        tokio::spawn(async move { repository.upsert(preferences).await })
    };
    let second = {
        let repository = repository.clone();
        let mut preferences = saved_preferences();
        preferences.color_scheme = PreferredColorScheme::Dark;
        tokio::spawn(async move { repository.upsert(preferences).await })
    };
    first
        .await
        .expect("first task joins")
        .expect("first write succeeds");
    second
        .await
        .expect("second task joins")
        .expect("second write succeeds");

    let bytes = tokio::fs::read(&path)
        .await
        .expect("final JSON document reads");
    let value: serde_json::Value =
        serde_json::from_slice(&bytes).expect("final JSON document is complete");
    assert_eq!(value["consumer_id"], TEST_CONSUMER);
    let final_preferences = PreferenceRepository::open(persistent_options(
        &path,
        TEST_CONSUMER,
        Arc::new(FixedClock(30)),
    ))
    .await
    .expect("final JSON document reopens")
    .repository
    .load()
    .await
    .expect("final document loads")
    .expect("final record exists")
    .preferences;
    assert!(matches!(
        final_preferences.color_scheme,
        PreferredColorScheme::Light | PreferredColorScheme::Dark
    ));
}

#[gpui::test]
async fn gpui_tokio_runs_json_repository_work_without_blocking_the_foreground(
    cx: &mut gpui::TestAppContext,
) {
    cx.executor().allow_parking();

    let directory = tempfile::tempdir().expect("temporary directory creates");
    let path = directory.path().join("gpui-tokio-preferences.json");
    let options = persistent_options(&path, TEST_CONSUMER, Arc::new(FixedClock(8_000)));
    let expected = saved_preferences();
    let expected_for_task = expected.clone();
    let (start_sender, start_receiver) = tokio::sync::oneshot::channel();
    cx.update(gpui_tokio::init);

    let storage_task = cx.update(|cx| {
        gpui_tokio::Tokio::spawn(cx, async move {
            start_receiver
                .await
                .expect("GPUI foreground releases repository task");
            let repository = PreferenceRepository::open(options)
                .await
                .expect("repository opens on GPUI's Tokio runtime")
                .repository;
            repository
                .upsert(expected_for_task)
                .await
                .expect("repository saves on GPUI's Tokio runtime");
            repository
                .load()
                .await
                .expect("repository loads on GPUI's Tokio runtime")
                .expect("saved document exists")
        })
    });

    start_sender
        .send(())
        .expect("Tokio spawn returned control to the GPUI foreground");
    let record = storage_task.await.expect("Tokio repository task joins");
    assert_eq!(record.preferences, expected);
}
