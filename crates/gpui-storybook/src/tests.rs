use super::*;
use es_fluent::{FluentMessage, FluentMessageLookup};
use std::{
    convert::Infallible,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};
use unic_langid::LanguageIdentifier;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum TestLanguage {
    #[default]
    English,
}

impl strum::IntoEnumIterator for TestLanguage {
    type Iterator = std::array::IntoIter<Self, 1>;

    fn iter() -> Self::Iterator {
        [Self::English].into_iter()
    }
}

impl From<TestLanguage> for LanguageIdentifier {
    fn from(_: TestLanguage) -> Self {
        "en".parse().expect("test language tag should be valid")
    }
}

impl TryFrom<LanguageIdentifier> for TestLanguage {
    type Error = ();

    fn try_from(identifier: LanguageIdentifier) -> Result<Self, Self::Error> {
        (identifier.language.as_str() == "en")
            .then_some(Self::English)
            .ok_or(())
    }
}

impl FluentMessage for TestLanguage {
    fn to_fluent_string_with(&self, _: &mut FluentMessageLookup<'_>) -> String {
        "English".to_owned()
    }
}

fn test_options() -> StorybookOptions<TestLanguage> {
    StorybookOptions::new(
        ConsumerId::new("facade-preference-test").expect("test consumer id should be valid"),
        TestLanguage::English,
        |_, _| Ok::<(), Infallible>(()),
    )
}

fn unused_create_fn(
    _: &mut ::gpui_kit::Window,
    _: &mut ::gpui_kit::App,
) -> ::gpui_kit::Entity<StoryContainer> {
    unreachable!("story creation is not used in these tests");
}

static SECTIONED_ENTRY: __registry::StoryEntry = __registry::StoryEntry::new(
    "component-example-SectionedStory",
    "SectionedStory",
    Some("Notes"),
    None,
    unused_create_fn,
    __registry::StoryRegistrationSource::new(
        "component-example",
        "/tmp/component-example",
        "examples/component/src/components/field_notes.rs",
        10,
    ),
);

static UNSECTIONED_ENTRY: __registry::StoryEntry = __registry::StoryEntry::new(
    "component-example-UnsectionedStory",
    "UnsectionedStory",
    None,
    None,
    unused_create_fn,
    __registry::StoryRegistrationSource::new(
        "component-example",
        "/tmp/component-example",
        "examples/component/src/components/field_notes.rs",
        42,
    ),
);

static ORDERED_FIRST: __registry::StoryEntry = __registry::StoryEntry::new(
    "component-example-ZStory",
    "ZStory",
    Some("Zed"),
    Some(1),
    unused_create_fn,
    __registry::StoryRegistrationSource::new(
        "component-example",
        "/tmp/component-example",
        "src/z.rs",
        1,
    ),
);

static ORDERED_SECOND: __registry::StoryEntry = __registry::StoryEntry::new(
    "component-example-AStory",
    "AStory",
    Some("Alpha"),
    Some(2),
    unused_create_fn,
    __registry::StoryRegistrationSource::new(
        "component-example",
        "/tmp/component-example",
        "src/a.rs",
        2,
    ),
);

static ORDERED_FIRST_ALPHA: __registry::StoryEntry = __registry::StoryEntry::new(
    "component-example-AStory",
    "AStory",
    Some("Alpha"),
    Some(1),
    unused_create_fn,
    __registry::StoryRegistrationSource::new(
        "component-example",
        "/tmp/component-example",
        "src/a.rs",
        3,
    ),
);

fn with_temp_dir(test_fn: impl FnOnce(&Path)) {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time should move forward")
        .as_nanos();
    let path = std::env::temp_dir().join(format!("gpui_storybook_facade_{timestamp}"));
    std::fs::create_dir_all(&path).expect("temp directory should be created");
    test_fn(&path);
    std::fs::remove_dir_all(path).expect("temp directory should be removed");
}

fn runtime_config(allow: &[&str]) -> gpui_storybook_toml::StorybookToml {
    gpui_storybook_toml::StorybookToml {
        group: "storybook-app".into(),
        window_mode: None,
        allow: Some(allow.iter().map(|group| (*group).to_string()).collect()),
        disable_story: Vec::new(),
        overrides: gpui_storybook_toml::StorybookPreferenceOverrides::default(),
    }
}

#[test]
fn toml_preference_overrides_map_to_typed_runtime_values() {
    let config = gpui_storybook_toml::StorybookToml {
        group: "storybook-app".to_owned(),
        overrides: gpui_storybook_toml::StorybookPreferenceOverrides {
            color_scheme: Some(gpui_storybook_toml::StorybookColorScheme::Dark),
            theme: Some("Default Dark".to_owned()),
            language: Some("en".to_owned()),
        },
        ..gpui_storybook_toml::StorybookToml::default()
    };
    let mut overrides = PreferenceOverrides::<TestLanguage>::default();

    apply_toml_preference_overrides(&mut overrides, &config)
        .expect("valid TOML overrides should map to typed values");

    assert_eq!(overrides.color_scheme, Some(SystemColorScheme::Dark));
    assert_eq!(
        overrides.theme.as_ref().map(ThemeId::as_str),
        Some("Default Dark")
    );
    assert_eq!(overrides.language, Some(TestLanguage::English));
}

#[test]
fn programmatic_overrides_take_precedence_over_toml() {
    let config = gpui_storybook_toml::StorybookToml {
        group: "storybook-app".to_owned(),
        overrides: gpui_storybook_toml::StorybookPreferenceOverrides {
            color_scheme: Some(gpui_storybook_toml::StorybookColorScheme::Light),
            theme: Some("Default Light".to_owned()),
            language: Some("fr".to_owned()),
        },
        ..gpui_storybook_toml::StorybookToml::default()
    };
    let mut overrides = PreferenceOverrides {
        color_scheme: Some(SystemColorScheme::Dark),
        theme: Some(ThemeId::new("Custom Dark").expect("test theme id should be valid")),
        language: Some(TestLanguage::English),
    };

    apply_toml_preference_overrides(&mut overrides, &config)
        .expect("programmatic values should bypass conflicting TOML values");

    assert_eq!(overrides.color_scheme, Some(SystemColorScheme::Dark));
    assert_eq!(
        overrides.theme.as_ref().map(ThemeId::as_str),
        Some("Custom Dark")
    );
    assert_eq!(overrides.language, Some(TestLanguage::English));
}

#[test]
fn unsupported_toml_language_is_an_initialization_error() {
    let config = gpui_storybook_toml::StorybookToml {
        group: "storybook-app".to_owned(),
        overrides: gpui_storybook_toml::StorybookPreferenceOverrides {
            language: Some("fr".to_owned()),
            ..gpui_storybook_toml::StorybookPreferenceOverrides::default()
        },
        ..gpui_storybook_toml::StorybookToml::default()
    };
    let mut overrides = PreferenceOverrides::<TestLanguage>::default();

    let error = apply_toml_preference_overrides(&mut overrides, &config)
        .expect_err("unsupported typed language should fail initialization");

    assert!(matches!(
        error,
        StorybookInitError::InvalidTomlOverride {
            field: "overrides.language",
            value,
        } if value == "fr"
    ));
}

#[cfg(all(feature = "mcp", any(target_os = "linux", target_os = "macos")))]
#[test]
fn capture_profile_is_deterministic_and_disables_storage() {
    let mut persistence = PersistenceMode::Persistent;
    let mut json_path = Some(PathBuf::from("portable/preferences.json"));
    let mut overrides = PreferenceOverrides {
        color_scheme: Some(SystemColorScheme::Dark),
        theme: Some(ThemeId::new("Custom Dark").expect("test theme id should be valid")),
        language: Some(1_u8),
    };

    apply_automation_preference_profile(
        AutomationPreferenceProfile::Capture,
        &mut persistence,
        &mut json_path,
        &mut overrides,
        7_u8,
    )
    .expect("the built-in deterministic theme should be valid");

    assert_eq!(persistence, PersistenceMode::Disabled);
    assert_eq!(json_path, None);
    assert_eq!(overrides.color_scheme, Some(SystemColorScheme::Light));
    assert_eq!(
        overrides.theme.as_ref().map(ThemeId::as_str),
        Some("Default Light")
    );
    assert_eq!(overrides.language, Some(7));
}

#[cfg(all(feature = "mcp", any(target_os = "linux", target_os = "macos")))]
#[test]
fn stdio_profile_is_deterministic_and_uses_temporary_storage() {
    let mut persistence = PersistenceMode::Persistent;
    let mut json_path = Some(PathBuf::from("portable/preferences.json"));
    let mut overrides = PreferenceOverrides {
        color_scheme: Some(SystemColorScheme::Dark),
        theme: Some(ThemeId::new("Custom Dark").expect("test theme id should be valid")),
        language: Some(1_u8),
    };

    apply_automation_preference_profile(
        AutomationPreferenceProfile::Stdio,
        &mut persistence,
        &mut json_path,
        &mut overrides,
        7_u8,
    )
    .expect("the built-in deterministic theme should be valid");

    assert_eq!(persistence, PersistenceMode::Temporary);
    assert_eq!(json_path, None);
    assert_eq!(overrides.color_scheme, Some(SystemColorScheme::Light));
    assert_eq!(
        overrides.theme.as_ref().map(ThemeId::as_str),
        Some("Default Light")
    );
    assert_eq!(overrides.language, Some(7));
}

#[gpui_kit::test]
fn init_rejects_a_path_override_for_non_persistent_storage(cx: &mut ::gpui_kit::App) {
    let options = test_options()
        .with_persistence(PersistenceMode::Temporary)
        .with_json_path("portable/preferences.json");

    let result = init(cx, options);

    assert!(matches!(
        result,
        Err(StorybookInitError::PathOverrideRequiresPersistent)
    ));
    assert!(cx.try_global::<StorybookInitialized>().is_none());
}

#[gpui_kit::test]
async fn init_rejects_a_second_initialization(cx: &mut ::gpui_kit::TestAppContext) {
    cx.executor().allow_parking();
    let first = cx.update(|cx| {
        init(
            cx,
            test_options().with_persistence(PersistenceMode::Disabled),
        )
        .expect("the first initialization should start")
    });

    let second = cx.update(|cx| {
        init(
            cx,
            test_options().with_persistence(PersistenceMode::Disabled),
        )
    });
    assert!(matches!(
        second,
        Err(StorybookInitError::AlreadyInitialized)
    ));

    let _ready = first.await;
}

#[gpui_kit::test]
async fn readiness_installs_live_automation_before_the_caller_constructs_a_window(
    cx: &mut ::gpui_kit::TestAppContext,
) {
    cx.executor().allow_parking();
    assert!(cx.windows().is_empty());
    assert!(cx.update(|cx| {
        gpui_storybook_core::automation::default_storybook_automation(cx).is_none()
    }));

    let readiness = cx.update(|cx| {
        init(
            cx,
            test_options().with_persistence(PersistenceMode::Disabled),
        )
        .expect("valid facade options should initialize")
    });
    assert!(cx.windows().is_empty());
    let automation = cx
        .update(|cx| gpui_storybook_core::automation::default_storybook_automation(cx))
        .expect("baseline initialization should install live automation");

    let ready = readiness.await;
    assert_eq!(ready.persistence_status, PersistenceStatus::Ready);
    assert!(cx.windows().is_empty());
    let ready_automation = cx
        .update(|cx| gpui_storybook_core::automation::default_storybook_automation(cx))
        .expect("readiness should retain live automation");
    assert!(std::sync::Arc::ptr_eq(&automation, &ready_automation));

    cx.update(|cx| {
        cx.open_window(Default::default(), |window, cx| {
            gpui_storybook_core::gallery::Gallery::view(Vec::new(), None, window, cx)
        })
        .expect("caller should be able to create a window after readiness")
    });
    assert_eq!(cx.windows().len(), 1);

    let error = automation
        .read_controls()
        .await
        .expect_err("the empty gallery should have no active story");
    assert_eq!(error, StorybookAutomationError::NoActiveStory);
}

#[test]
fn crate_group_filters_without_overwriting_declared_section() {
    let resolved = resolve_story_entry(
        &SECTIONED_ENTRY,
        Some("gpui-storybook-example-component"),
        Some(&runtime_config(&["gpui-storybook-example-component"])),
    )
    .expect("crate group should satisfy runtime allow");

    assert_eq!(
        resolved.group.as_deref(),
        Some("gpui-storybook-example-component")
    );
    assert_eq!(resolved.section.as_deref(), Some("Notes"));
}

#[test]
fn unsectioned_stories_keep_crate_group_without_faking_a_section() {
    let resolved = resolve_story_entry(
        &UNSECTIONED_ENTRY,
        Some("gpui-storybook-example-component"),
        Some(&runtime_config(&["gpui-storybook-example-component"])),
    )
    .expect("crate group should satisfy runtime allow");

    assert_eq!(
        resolved.group.as_deref(),
        Some("gpui-storybook-example-component")
    );
    assert_eq!(resolved.section.as_deref(), None);
}

#[test]
fn duplicate_story_key_validator_reports_both_registrations() {
    static FIRST_ENTRY: __registry::StoryEntry = __registry::StoryEntry::new(
        "component-example-ButtonStory",
        "ButtonStory",
        None,
        None,
        unused_create_fn,
        __registry::StoryRegistrationSource::new(
            "component-example",
            "/tmp/component-example",
            "src/first.rs",
            10,
        ),
    );
    static SECOND_ENTRY: __registry::StoryEntry = __registry::StoryEntry::new(
        "component-example-ButtonStory",
        "ButtonStory",
        None,
        None,
        unused_create_fn,
        __registry::StoryRegistrationSource::new(
            "component-example",
            "/tmp/component-example",
            "src/second.rs",
            20,
        ),
    );

    let error = validate_unique_story_keys(&[&FIRST_ENTRY, &SECOND_ENTRY])
        .expect_err("duplicate keys should be rejected");

    assert_eq!(error.key.as_str(), "component-example-ButtonStory");
    assert_eq!(
        error.to_string(),
        "duplicate story key `component-example-ButtonStory` registered by component-example::ButtonStory at src/first.rs:10 and component-example::ButtonStory at src/second.rs:20"
    );
}

#[test]
fn unique_story_key_validator_accepts_empty_and_distinct_registrations() {
    assert!(validate_unique_story_keys(&[]).is_ok());
    assert!(validate_unique_story_keys(&[&SECTIONED_ENTRY, &UNSECTIONED_ENTRY]).is_ok());
}

#[test]
fn runtime_filters_reject_unlisted_groups_and_disabled_stories() {
    assert!(
        resolve_story_entry(
            &SECTIONED_ENTRY,
            Some("component-example"),
            Some(&runtime_config(&["other"])),
        )
        .is_none()
    );

    let mut config = runtime_config(&["component-example"]);
    config.disable_story.push("SectionedStory".to_string());
    assert!(
        resolve_story_entry(&SECTIONED_ENTRY, Some("component-example"), Some(&config)).is_none()
    );
}

#[test]
fn declared_section_is_the_filter_group_without_crate_config() {
    let resolved = resolve_story_entry(&SECTIONED_ENTRY, None, Some(&runtime_config(&["Notes"])))
        .expect("declared section should satisfy the allow list");

    assert_eq!(resolved.group, None);
    assert_eq!(resolved.section.as_deref(), Some("Notes"));
    assert!(resolve_story_entry(&UNSECTIONED_ENTRY, None, None).is_some());
}

#[test]
fn resolved_entries_sort_by_order_section_then_name() {
    let ordered_first = ResolvedStoryEntry {
        entry: &ORDERED_FIRST,
        group: None,
        section: Some("Zed".to_string()),
    };
    let ordered_second = ResolvedStoryEntry {
        entry: &ORDERED_SECOND,
        group: None,
        section: Some("Alpha".to_string()),
    };
    let ordered_first_alpha = ResolvedStoryEntry {
        entry: &ORDERED_FIRST_ALPHA,
        group: None,
        section: Some("Alpha".to_string()),
    };
    let sectioned = ResolvedStoryEntry {
        entry: &SECTIONED_ENTRY,
        group: None,
        section: Some("Notes".to_string()),
    };
    let sectioned_alpha = ResolvedStoryEntry {
        entry: &ORDERED_SECOND,
        group: None,
        section: Some("Alpha".to_string()),
    };
    let unsectioned = ResolvedStoryEntry {
        entry: &UNSECTIONED_ENTRY,
        group: None,
        section: None,
    };

    assert!(compare_resolved_story_entries(&ordered_first, &ordered_second).is_lt());
    assert!(compare_resolved_story_entries(&ordered_second, &ordered_first).is_gt());
    assert!(compare_resolved_story_entries(&ordered_first_alpha, &ordered_first).is_lt());
    assert!(compare_resolved_story_entries(&ordered_first, &sectioned).is_lt());
    assert!(compare_resolved_story_entries(&sectioned, &ordered_first).is_gt());
    assert!(compare_resolved_story_entries(&sectioned_alpha, &sectioned).is_lt());
    assert!(compare_resolved_story_entries(&sectioned, &unsectioned).is_lt());
    assert!(compare_resolved_story_entries(&unsectioned, &sectioned).is_gt());

    let unsectioned_alpha = ResolvedStoryEntry {
        entry: &ORDERED_SECOND,
        group: None,
        section: None,
    };
    assert!(compare_resolved_story_entries(&unsectioned_alpha, &unsectioned).is_lt());
}

#[test]
fn config_loading_handles_valid_missing_and_invalid_files() {
    with_temp_dir(|dir| {
        let crate_dir: &'static str =
            Box::leak(dir.to_string_lossy().into_owned().into_boxed_str());
        let entry = __registry::StoryEntry::new(
            "temp-Story",
            "Story",
            None,
            None,
            unused_create_fn,
            __registry::StoryRegistrationSource::new("temp", crate_dir, "src/lib.rs", 1),
        );

        assert_eq!(load_storybook_config(&entry), None);

        std::fs::write(
            dir.join("storybook.toml"),
            "group = \"Temp\"\nwindow_mode = \"dock\"\n",
        )
        .expect("valid config should be written");
        let config = load_storybook_config(&entry).expect("valid config should load");
        assert_eq!(config.group, "Temp");
        assert_eq!(
            config.window_mode,
            Some(gpui_storybook_toml::StorybookWindowMode::Dock)
        );

        std::fs::write(dir.join("storybook.toml"), "invalid = true\n")
            .expect("invalid config should be written");
        assert_eq!(load_storybook_config(&entry), None);
    });
}

#[test]
fn runtime_config_matches_the_current_test_binary_and_populates_cache() {
    with_temp_dir(|dir| {
        std::fs::write(dir.join("storybook.toml"), "group = \"Test Binary\"\n")
            .expect("runtime config should be written");
        let bin_name = current_binary_name().expect("test binary should have a file stem");
        let crate_name: &'static str = Box::leak(bin_name.into_boxed_str());
        let crate_dir: &'static str =
            Box::leak(dir.to_string_lossy().into_owned().into_boxed_str());
        let entry: &'static __registry::StoryEntry =
            Box::leak(Box::new(__registry::StoryEntry::new(
                "test-Story",
                "Story",
                None,
                None,
                unused_create_fn,
                __registry::StoryRegistrationSource::new(crate_name, crate_dir, "src/lib.rs", 1),
            )));
        let mut cache = HashMap::new();

        let config = load_runtime_storybook_config(&[entry], &mut cache)
            .expect("matching binary config should load");
        assert_eq!(config.group(), Some("Test Binary"));
        assert!(cache.contains_key(crate_dir));

        let unmatched = load_runtime_storybook_config(&[], &mut cache);
        assert_eq!(unmatched, None);
    });
}

#[test]
fn project_root_prefers_the_workspace_and_supports_standalone_crates() {
    with_temp_dir(|dir| {
        let workspace = dir.join("workspace");
        let member = workspace.join("crates/member");
        let member_source = member.join("src");
        std::fs::create_dir_all(&member_source).expect("workspace member directories create");
        std::fs::write(
            workspace.join("Cargo.toml"),
            "[workspace]\nmembers = [\"crates/member\"]\n",
        )
        .expect("workspace manifest writes");
        std::fs::write(
            member.join("Cargo.toml"),
            "[package]\nname = \"member\"\nversion = \"0.1.0\"\n",
        )
        .expect("member manifest writes");
        assert_eq!(find_cargo_project_root(&member_source), workspace);

        let standalone = dir.join("standalone");
        let standalone_source = standalone.join("src");
        std::fs::create_dir_all(&standalone_source).expect("standalone source directory creates");
        std::fs::write(
            standalone.join("Cargo.toml"),
            "[package]\nname = \"standalone\"\nversion = \"0.1.0\"\n",
        )
        .expect("standalone manifest writes");
        assert_eq!(find_cargo_project_root(&standalone_source), standalone);

        let no_manifest = dir.join("no-manifest");
        assert_eq!(find_cargo_project_root(&no_manifest), no_manifest);
    });
}
