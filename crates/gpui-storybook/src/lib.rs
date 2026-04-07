#[cfg(feature = "macros")]
pub use gpui_storybook_macros::*;

use gpui_storybook_core::locale::LocaleStore;
use std::collections::HashMap;

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
    story::{Story, StoryContainer, create_new_window},
    window_view::SimpleWindowView,
};

#[doc(hidden)]
pub use gpui_storybook_core::registry as __registry;

#[doc(hidden)]
pub use inventory as __inventory;

struct ResolvedStoryEntry {
    entry: &'static __registry::StoryEntry,
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

    let mut crate_configs: HashMap<&'static str, Option<gpui_storybook_toml::StorybookToml>> =
        HashMap::new();

    let mut entries: Vec<_> = inventory::iter::<__registry::StoryEntry>()
        .filter_map(|entry| {
            let config = crate_configs
                .entry(entry.crate_dir)
                .or_insert_with(|| load_storybook_config(entry));

            let section = config
                .as_ref()
                .and_then(gpui_storybook_toml::StorybookToml::group)
                .or(entry.section);

            if let Some(config) = config.as_ref()
                && !config.allows_group(section)
            {
                tracing::debug!(
                    "Skipping story '{}' from crate '{}' because group is not listed in allow",
                    entry.name,
                    entry.crate_name
                );
                return None;
            }

            if let Some(config) = config.as_ref()
                && config.is_story_disabled(entry.name)
            {
                tracing::debug!(
                    "Skipping story '{}' from crate '{}' because it is listed in disable_story",
                    entry.name,
                    entry.crate_name
                );
                return None;
            }

            let section = section.map(str::to_string);

            Some(ResolvedStoryEntry { entry, section })
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

            tracing::info!(
                "Story: {}{} ({}:{}) [{}]",
                resolved.entry.name,
                section_info,
                resolved.entry.file,
                resolved.entry.line,
                resolved.entry.crate_name
            );

            let container = (resolved.entry.create_fn)(window, cx);
            if let Some(section) = resolved.section {
                container.update(cx, |c, _| {
                    c.section = Some(section.into());
                });
            }
            container
        })
        .collect()
}
