//! Public facade for building GPUI storybook binaries.
//!
//! Most applications should depend on this crate rather than the lower-level
//! runtime, macro, or TOML crates. It re-exports the standard runtime shell,
//! story traits, locale helpers, window helpers, and, with the default
//! `macros` feature, the story registration macros.
//!
//! Story registration flows through `inventory`: `#[story]` and
//! `#[derive(ComponentStory)]` submit story entries, `#[derive(Substory)]`
//! derives stable capture keys for styled `section` or custom
//! `StorySectionBase` regions inside a story, and
//! `#[story_init]` submits one-time setup hooks. The hidden `__registry` and
//! `__inventory` re-exports are the stable expansion path used by those
//! macros.
//!
//! `generate_stories` loads crate-local `storybook.toml` files for discovered
//! story crates, selects a runtime config by matching the running binary name
//! against registered story crate names, applies `allow` and `disable_story`
//! filtering, then materializes sorted [`StoryContainer`] values. A story crate
//! config's `group` becomes the sidebar's outer group; a story's declared
//! section remains the nested label.
//!
//! Macro-generated stories carry stable [`StoryKey`] values in the form
//! `{crate-package-name}-{registered-story-name}`. These keys are copied into
//! generated [`StoryContainer`] values as typed [`RegisteredStoryMetadata`] for
//! automation and capture routes.
//!
//! Feature boundaries:
//!
//! - `macros`: re-exports proc macros from `gpui-storybook-macros`
//! - `dock`: re-exports the dock workspace helpers from `gpui-storybook-core`
//! - `mcp`: installs a default automation controller during [`init`] and
//!   re-exports MCP automation and capture helpers

#[cfg(feature = "macros")]
pub use gpui_storybook_macros::*;

use gpui_storybook_core::locale::LocaleStore;
use std::{
    collections::{BTreeMap, HashMap},
    fmt,
    path::PathBuf,
};

#[cfg(feature = "dock")]
pub use gpui_storybook_core::dock_gallery::{
    StoryWorkspace, create_dock_window, register_story_panels,
};
pub use gpui_storybook_core::registry::{
    RegisteredStoryMetadata, StoryKey, StoryName, StorySectionName,
};
#[cfg(feature = "dock")]
pub use gpui_storybook_core::window_view::DockWindowView;
pub use gpui_storybook_core::{
    assets::Assets,
    capture_region::{
        capture_route_slug, capture_substory, capture_substory_route_id,
        capture_substory_route_id_with_key, capture_substory_with_key,
    },
    gallery::Gallery,
    i18n::change_locale,
    i18n::localize_message,
    language::{CurrentLanguage, Language},
    story::{
        Story, StoryContainer, StorySection, StorySectionBase, StorySectionTitle, Substory,
        create_new_window, create_new_window_with_ui, section,
    },
    storybook_window_ui::{StorybookWindow, StorybookWindowUi},
    window_view::SimpleWindowView,
};

#[doc(hidden)]
pub use gpui_storybook_core::registry as __registry;

#[doc(hidden)]
pub use inventory as __inventory;

#[cfg(feature = "mcp")]
pub mod mcp {
    pub use gpui_storybook_mcp::*;
}

#[cfg(feature = "mcp")]
pub mod capture {
    pub use gpui_storybook_mcp::capture::*;
}

struct ResolvedStoryEntry {
    entry: &'static __registry::StoryEntry,
    group: Option<String>,
    section: Option<String>,
}

#[derive(Debug)]
struct DuplicateStoryKeyError {
    key: StoryKey,
    first: StoryRegistrationLocation,
    second: StoryRegistrationLocation,
}

#[derive(Clone, Debug)]
struct StoryRegistrationLocation {
    crate_name: &'static str,
    story_name: StoryName,
    file: &'static str,
    line: u32,
}

impl From<&'static __registry::StoryEntry> for StoryRegistrationLocation {
    fn from(entry: &'static __registry::StoryEntry) -> Self {
        Self {
            crate_name: entry.crate_name,
            story_name: entry.name,
            file: entry.file,
            line: entry.line,
        }
    }
}

impl fmt::Display for DuplicateStoryKeyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "duplicate story key `{}` registered by {}::{} at {}:{} and {}::{} at {}:{}",
            self.key,
            self.first.crate_name,
            self.first.story_name,
            self.first.file,
            self.first.line,
            self.second.crate_name,
            self.second.story_name,
            self.second.file,
            self.second.line,
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct StoryGroupKey {
    group: Option<String>,
    section: Option<String>,
    title: String,
}

fn group_duplicate_story_titles(
    stories: Vec<::gpui::Entity<StoryContainer>>,
    window: &mut ::gpui::Window,
    cx: &mut ::gpui::App,
) -> Vec<::gpui::Entity<StoryContainer>> {
    let mut grouped: Vec<(StoryGroupKey, Vec<::gpui::Entity<StoryContainer>>)> = Vec::new();

    for story in stories {
        let key = {
            let story_data = story.read(cx);
            StoryGroupKey {
                group: story_data.group.as_ref().map(ToString::to_string),
                section: story_data.section.as_ref().map(ToString::to_string),
                title: story_data.display_title(cx),
            }
        };

        if let Some((_, bucket)) = grouped
            .iter_mut()
            .find(|(existing_key, _)| *existing_key == key)
        {
            bucket.push(story);
        } else {
            grouped.push((key, vec![story]));
        }
    }

    grouped
        .into_iter()
        .map(|(key, bucket)| {
            if bucket.len() == 1 {
                return bucket.into_iter().next().expect("bucket has one story");
            }

            let panel = StoryContainer::list_panel(key.title, bucket, window, cx);
            panel.update(cx, |container, _| {
                container.group = key.group.map(Into::into);
                container.section = key.section.map(Into::into);
            });
            panel
        })
        .collect()
}

fn load_storybook_config(
    entry: &__registry::StoryEntry,
) -> Option<gpui_storybook_toml::StorybookToml> {
    match gpui_storybook_toml::load_from_dir(entry.crate_dir) {
        Ok(config) => config,
        Err(err) => {
            tracing::warn!(
                "Failed to load storybook.toml for crate '{}' ({}): {}",
                entry.crate_name,
                entry.crate_dir,
                err
            );
            None
        },
    }
}

fn current_binary_name() -> Option<String> {
    let argv0 = std::env::args_os().next()?;
    PathBuf::from(argv0)
        .file_stem()
        .map(|stem| stem.to_string_lossy().to_string())
}

fn load_runtime_storybook_config(
    all_entries: &[&'static __registry::StoryEntry],
    crate_configs: &mut HashMap<&'static str, Option<gpui_storybook_toml::StorybookToml>>,
) -> Option<gpui_storybook_toml::StorybookToml> {
    let bin_name = current_binary_name()?;
    let entry = all_entries
        .iter()
        .copied()
        .find(|entry| entry.crate_name == bin_name)?;

    crate_configs
        .entry(entry.crate_dir)
        .or_insert_with(|| load_storybook_config(entry))
        .clone()
}

fn resolve_story_entry(
    entry: &'static __registry::StoryEntry,
    crate_group: Option<&str>,
    runtime_config: Option<&gpui_storybook_toml::StorybookToml>,
) -> Option<ResolvedStoryEntry> {
    let entry_section = entry.section.map(StorySectionName::as_str);
    let filter_group = crate_group.or(entry_section);

    if let Some(runtime_config) = runtime_config
        && !runtime_config.allows_group(filter_group)
    {
        tracing::debug!(
            "Skipping story '{}' from crate '{}' because group '{:?}' is not listed in runtime allow",
            entry.name,
            entry.crate_name,
            filter_group
        );
        return None;
    }

    if let Some(runtime_config) = runtime_config
        && runtime_config.is_story_disabled(entry.name.as_str())
    {
        tracing::debug!(
            "Skipping story '{}' from crate '{}' because it is listed in runtime disable_story",
            entry.name,
            entry.crate_name
        );
        return None;
    }

    Some(ResolvedStoryEntry {
        entry,
        group: crate_group.map(str::to_string),
        section: entry.section.map(|section| section.as_str().to_string()),
    })
}

fn compare_resolved_story_entries(
    a: &ResolvedStoryEntry,
    b: &ResolvedStoryEntry,
) -> std::cmp::Ordering {
    match (a.entry.section_order, b.entry.section_order) {
        (Some(order_a), Some(order_b)) => order_a
            .cmp(&order_b)
            .then_with(|| a.entry.name.cmp(&b.entry.name)),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => match (&a.section, &b.section) {
            (Some(sec_a), Some(sec_b)) => sec_a
                .cmp(sec_b)
                .then_with(|| a.entry.name.cmp(&b.entry.name)),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => a.entry.name.cmp(&b.entry.name),
        },
    }
}

/// Initializes Storybook runtime state and locale wiring.
///
/// Call this once before creating the story window or calling
/// [`generate_stories`]. The function stores the current language, installs the
/// locale manager, initializes the core runtime shell, registers dock panel
/// types when the `dock` feature is enabled, and then runs all discovered
/// `#[story_init]` hooks.
pub fn init<L>(cx: &mut ::gpui::App, language: L)
where
    L: Language,
{
    cx.set_global(CurrentLanguage(language));
    cx.set_global(
        Box::new(gpui_storybook_core::locale::LocaleManager::<L>::new()) as Box<dyn LocaleStore>,
    );
    gpui_storybook_core::story::init(cx);
    #[cfg(feature = "dock")]
    register_story_panels(cx);
    #[cfg(feature = "mcp")]
    init_mcp_automation(cx);

    let global_init_count = inventory::iter::<__registry::InitEntry>().count();
    if global_init_count > 0 {
        tracing::info!("Discovered {} global init function(s)", global_init_count);
        for entry in inventory::iter::<__registry::InitEntry>() {
            tracing::info!("Init fn: {} ({}:{})", entry.fn_name, entry.file, entry.line);
            (entry.init_fn)(cx);
        }
    }
}

#[cfg(feature = "mcp")]
fn init_mcp_automation(cx: &mut ::gpui::App) {
    let automation = gpui_storybook_core::automation::default_storybook_automation(cx)
        .unwrap_or_else(|| {
            gpui_storybook_core::automation::set_default_storybook_automation(
                cx,
                gpui_storybook_mcp::StorybookAutomation::new(),
            )
        });

    if gpui_storybook_mcp::stdio_requested()
        && let Err(error) = gpui_storybook_mcp::start_stdio(automation.clone())
    {
        eprintln!("failed to start gpui-storybook MCP stdio server: {error}");
    }

    if let Err(error) = gpui_storybook_mcp::start_capture_session_from_env(automation) {
        eprintln!("failed to start storybook capture session: {error}");
    }
}

/// Discovers registered stories, applies `storybook.toml` filtering, and
/// returns runtime story containers.
///
/// The active runtime config is selected from the registered story crate whose
/// package name matches the running binary. If no registered story crate
/// matches the binary name, crate-local groups are still attached to stories
/// but runtime `allow` and `disable_story` filters are not applied.
///
/// Stories are sorted by enum-section order when available, then by section and
/// registered story name. Stories with the same title in the same group and
/// section are grouped into one list panel.
pub fn generate_stories(
    window: &mut ::gpui::Window,
    cx: &mut ::gpui::App,
) -> Vec<::gpui::Entity<StoryContainer>> {
    let story_count = inventory::iter::<__registry::StoryEntry>().count();
    let init_count = inventory::iter::<__registry::InitEntry>().count();

    tracing::info!("Discovered {} story(ies)", story_count);
    tracing::info!(
        "Init functions registered: {}",
        if init_count > 0 {
            format!("{} function(s)", init_count)
        } else {
            "none".to_string()
        }
    );

    let all_entries: Vec<_> = inventory::iter::<__registry::StoryEntry>().collect();
    validate_unique_story_keys(&all_entries)
        .unwrap_or_else(|error| panic!("invalid storybook registry: {error}"));
    let mut crate_configs: HashMap<&'static str, Option<gpui_storybook_toml::StorybookToml>> =
        HashMap::new();
    let runtime_config = load_runtime_storybook_config(&all_entries, &mut crate_configs);

    if let Some(runtime_config) = runtime_config.as_ref()
        && let Some(group) = runtime_config.group()
    {
        tracing::info!(
            "Using runtime storybook.toml with group '{}' and allow {:?}",
            group,
            runtime_config.allow.as_ref()
        );
    }

    let mut entries: Vec<_> = all_entries
        .into_iter()
        .filter_map(|entry| {
            let config = crate_configs
                .entry(entry.crate_dir)
                .or_insert_with(|| load_storybook_config(entry));

            resolve_story_entry(
                entry,
                config
                    .as_ref()
                    .and_then(gpui_storybook_toml::StorybookToml::group),
                runtime_config.as_ref(),
            )
        })
        .collect();

    tracing::info!(
        "Collected {} story(ies) after storybook.toml filtering",
        entries.len()
    );

    entries.sort_by(compare_resolved_story_entries);

    let stories = entries
        .into_iter()
        .map(|resolved| {
            let section_info = resolved
                .section
                .as_ref()
                .map(|s| format!(", section: \"{}\"", s))
                .unwrap_or_default();
            let group_info = resolved
                .group
                .as_ref()
                .map(|group| format!(", group: \"{}\"", group))
                .unwrap_or_default();

            tracing::info!(
                "Story: {} (key: {}){}{} ({}:{}) [{}]",
                resolved.entry.name,
                resolved.entry.key(),
                section_info,
                group_info,
                resolved.entry.file,
                resolved.entry.line,
                resolved.entry.crate_name
            );

            let container = (resolved.entry.create_fn)(window, cx);
            container.update(cx, |c, _| {
                c.group = resolved.group.clone().map(Into::into);
                c.section = resolved.section.clone().map(Into::into);
                c.set_registration_metadata(resolved.entry.metadata());
            });
            container
        })
        .collect();

    group_duplicate_story_titles(stories, window, cx)
}

fn validate_unique_story_keys(
    entries: &[&'static __registry::StoryEntry],
) -> Result<(), DuplicateStoryKeyError> {
    let mut seen = BTreeMap::new();

    for entry in entries {
        if let Some(first) = seen.insert(entry.key(), *entry) {
            return Err(DuplicateStoryKeyError {
                key: entry.key(),
                first: StoryRegistrationLocation::from(first),
                second: StoryRegistrationLocation::from(*entry),
            });
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        path::Path,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn unused_create_fn(
        _: &mut ::gpui::Window,
        _: &mut ::gpui::App,
    ) -> ::gpui::Entity<StoryContainer> {
        unreachable!("story creation is not used in these tests");
    }

    static SECTIONED_ENTRY: __registry::StoryEntry = __registry::StoryEntry::new(
        "component-example-SectionedStory",
        "SectionedStory",
        Some("Notes"),
        None,
        unused_create_fn,
        "component-example",
        "/tmp/component-example",
        "examples/component/src/components/field_notes.rs",
        10,
    );

    static UNSECTIONED_ENTRY: __registry::StoryEntry = __registry::StoryEntry::new(
        "component-example-UnsectionedStory",
        "UnsectionedStory",
        None,
        None,
        unused_create_fn,
        "component-example",
        "/tmp/component-example",
        "examples/component/src/components/field_notes.rs",
        42,
    );

    static ORDERED_FIRST: __registry::StoryEntry = __registry::StoryEntry::new(
        "component-example-ZStory",
        "ZStory",
        Some("Zed"),
        Some(1),
        unused_create_fn,
        "component-example",
        "/tmp/component-example",
        "src/z.rs",
        1,
    );

    static ORDERED_SECOND: __registry::StoryEntry = __registry::StoryEntry::new(
        "component-example-AStory",
        "AStory",
        Some("Alpha"),
        Some(2),
        unused_create_fn,
        "component-example",
        "/tmp/component-example",
        "src/a.rs",
        2,
    );

    static ORDERED_FIRST_ALPHA: __registry::StoryEntry = __registry::StoryEntry::new(
        "component-example-AStory",
        "AStory",
        Some("Alpha"),
        Some(1),
        unused_create_fn,
        "component-example",
        "/tmp/component-example",
        "src/a.rs",
        3,
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
            allow: Some(allow.iter().map(|group| (*group).to_string()).collect()),
            disable_story: Vec::new(),
        }
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
            "component-example",
            "/tmp/component-example",
            "src/first.rs",
            10,
        );
        static SECOND_ENTRY: __registry::StoryEntry = __registry::StoryEntry::new(
            "component-example-ButtonStory",
            "ButtonStory",
            None,
            None,
            unused_create_fn,
            "component-example",
            "/tmp/component-example",
            "src/second.rs",
            20,
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
            resolve_story_entry(&SECTIONED_ENTRY, Some("component-example"), Some(&config))
                .is_none()
        );
    }

    #[test]
    fn declared_section_is_the_filter_group_without_crate_config() {
        let resolved =
            resolve_story_entry(&SECTIONED_ENTRY, None, Some(&runtime_config(&["Notes"])))
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
                "temp",
                crate_dir,
                "src/lib.rs",
                1,
            );

            assert_eq!(load_storybook_config(&entry), None);

            std::fs::write(dir.join("storybook.toml"), "group = \"Temp\"\n")
                .expect("valid config should be written");
            assert_eq!(
                load_storybook_config(&entry).map(|config| config.group),
                Some("Temp".to_string())
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
                    crate_name,
                    crate_dir,
                    "src/lib.rs",
                    1,
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
}
