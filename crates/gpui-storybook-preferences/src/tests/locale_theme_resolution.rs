use crate::*;

use super::support::*;

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

    assert_eq!(
        resolve_preferences(
            &StorybookPreferences::default(),
            SystemColorScheme::Light,
            &DetectedLocales::default(),
            &languages,
            &TestThemes::unavailable_fallbacks(),
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
