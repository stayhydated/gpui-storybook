#[cfg(feature = "macros")]
pub use gpui_storybook_macros::*;

use gpui_storybook_core::locale::LocaleStore;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

#[cfg(feature = "dock")]
pub use gpui_storybook_core::dock_gallery::{
    StoryWorkspace, create_dock_window, register_story_panels,
};
#[cfg(feature = "dock")]
pub use gpui_storybook_core::window_view::DockWindowView;
pub use gpui_storybook_core::{
    assets::Assets,
    gallery::Gallery,
    i18n::change_locale,
    language::{CurrentLanguage, Language},
    story::{Story, StoryContainer, create_new_window, create_new_window_with_ui},
    storybook_window_ui::{StorybookWindow, StorybookWindowUi},
    window_view::SimpleWindowView,
};

#[doc(hidden)]
pub use gpui_storybook_core::registry as __registry;

#[doc(hidden)]
pub use inventory as __inventory;

struct ResolvedStoryEntry {
    entry: &'static __registry::StoryEntry,
    group: Option<String>,
    section: Option<String>,
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

fn load_storybook_config_from_working_directory() -> Option<gpui_storybook_toml::StorybookToml> {
    let mut current = std::env::current_dir().ok()?;
    loop {
        match gpui_storybook_toml::load_from_dir(&current) {
            Ok(Some(config)) => return Some(config),
            Ok(None) => {},
            Err(err) => {
                tracing::warn!(
                    "Failed to load storybook.toml from working directory '{}' path '{}': {}",
                    std::env::current_dir()
                        .ok()
                        .as_deref()
                        .unwrap_or_else(|| Path::new("<unknown>"))
                        .display(),
                    current.display(),
                    err
                );
                return None;
            },
        }

        if !current.pop() {
            break;
        }
    }

    None
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
    if let Some(bin_name) = current_binary_name()
        && let Some(entry) = all_entries
            .iter()
            .copied()
            .find(|entry| entry.crate_name == bin_name)
    {
        return crate_configs
            .entry(entry.crate_dir)
            .or_insert_with(|| load_storybook_config(entry))
            .clone();
    }

    load_storybook_config_from_working_directory()
}

fn resolve_story_entry(
    entry: &'static __registry::StoryEntry,
    crate_group: Option<&str>,
    runtime_config: Option<&gpui_storybook_toml::StorybookToml>,
) -> Option<ResolvedStoryEntry> {
    let filter_group = crate_group.or(entry.section);

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
        && runtime_config.is_story_disabled(entry.name)
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
        section: entry.section.map(str::to_string),
    })
}

pub fn init<L>(language: L, cx: &mut ::gpui::App)
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

    let global_init_count = inventory::iter::<__registry::InitEntry>().count();
    if global_init_count > 0 {
        tracing::info!("Discovered {} global init function(s)", global_init_count);
        for entry in inventory::iter::<__registry::InitEntry>() {
            tracing::info!("Init fn: {} ({}:{})", entry.fn_name, entry.file, entry.line);
            (entry.init_fn)(cx);
        }
    }
}

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

    entries.sort_by(|a, b| {
        // First sort by section_order (if both have it)
        match (a.entry.section_order, b.entry.section_order) {
            (Some(order_a), Some(order_b)) => {
                // Both have order, compare by order then by name
                order_a
                    .cmp(&order_b)
                    .then_with(|| a.entry.name.cmp(b.entry.name))
            },
            (Some(_), None) => std::cmp::Ordering::Less, // With order comes before without
            (None, Some(_)) => std::cmp::Ordering::Greater, // Without order comes after with
            (None, None) => {
                // Neither has order, sort by section name (if present) then by story name
                match (&a.section, &b.section) {
                    (Some(sec_a), Some(sec_b)) => sec_a
                        .cmp(sec_b)
                        .then_with(|| a.entry.name.cmp(b.entry.name)),
                    (Some(_), None) => std::cmp::Ordering::Less,
                    (None, Some(_)) => std::cmp::Ordering::Greater,
                    (None, None) => a.entry.name.cmp(b.entry.name),
                }
            },
        }
    });

    entries
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
                "Story: {}{}{} ({}:{}) [{}]",
                resolved.entry.name,
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
            });
            container
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unused_create_fn(
        _: &mut ::gpui::Window,
        _: &mut ::gpui::App,
    ) -> ::gpui::Entity<StoryContainer> {
        unreachable!("story creation is not used in these tests");
    }

    static SECTIONED_ENTRY: __registry::StoryEntry = __registry::StoryEntry {
        name: "SectionedStory",
        section: Some("Notes"),
        section_order: None,
        create_fn: unused_create_fn,
        crate_name: "component-example",
        crate_dir: "/tmp/component-example",
        file: "examples/component/src/components/field_notes.rs",
        line: 10,
    };

    static UNSECTIONED_ENTRY: __registry::StoryEntry = __registry::StoryEntry {
        name: "UnsectionedStory",
        section: None,
        section_order: None,
        create_fn: unused_create_fn,
        crate_name: "component-example",
        crate_dir: "/tmp/component-example",
        file: "examples/component/src/components/field_notes.rs",
        line: 42,
    };

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
}
