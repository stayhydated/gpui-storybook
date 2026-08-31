use crate::*;

use super::support::*;

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
    assert_eq!(
        schema["$defs"]["StorybookPreferences"]["properties"]["window_mode"]["$ref"],
        "#/$defs/StorybookWindowMode"
    );
    assert_eq!(
        schema["$defs"]["StorybookWindowMode"]["oneOf"][0]["const"],
        "gallery"
    );
    assert_eq!(
        schema["$defs"]["StorybookWindowMode"]["oneOf"][1]["const"],
        "dock"
    );
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
    assert_eq!(StorybookWindowMode::Gallery.token(), "gallery");
    assert_eq!(StorybookWindowMode::Dock.token(), "dock");
    assert_eq!("dock".parse(), Ok(StorybookWindowMode::Dock));
    assert!("Dock".parse::<StorybookWindowMode>().is_err());
    assert_eq!(PersistenceMode::Temporary.token(), "temporary");
    assert_eq!("disabled".parse(), Ok(PersistenceMode::Disabled));
}

#[test]
fn omitted_window_mode_defaults_to_gallery() {
    let mut value = serde_json::to_value(saved_preferences()).expect("preferences serialize");
    value
        .as_object_mut()
        .expect("preferences serialize as an object")
        .remove("window_mode");

    let preferences: StorybookPreferences =
        serde_json::from_value(value).expect("window mode may be omitted");

    assert_eq!(preferences.window_mode, StorybookWindowMode::Gallery);
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
