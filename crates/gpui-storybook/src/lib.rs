#[cfg(feature = "macros")]
pub use gpui_storybook_macros::*;

use gpui_storybook_core::locale::LocaleStore;

pub use gpui_storybook_core::{
    assets::Assets,
    gallery::Gallery,
    i18n::change_locale,
    language::{CurrentLanguage, Language},
    story::{Story, StoryContainer, create_new_window},
};

#[doc(hidden)]
pub use gpui_storybook_core::registry as __registry;

#[doc(hidden)]
pub use inventory as __inventory;

pub fn init<L>(language: L, cx: &mut ::gpui::App)
where
    L: Language,
{
    cx.set_global(CurrentLanguage(language));
    cx.set_global(
        Box::new(gpui_storybook_core::locale::LocaleManager::<L>::new()) as Box<dyn LocaleStore>,
    );
    gpui_storybook_core::story::init(cx);

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

    // Collect and sort stories
    let mut entries: Vec<_> = inventory::iter::<__registry::StoryEntry>().collect();

    entries.sort_by(|a, b| {
        // First sort by section_order (if both have it)
        match (a.section_order, b.section_order) {
            (Some(order_a), Some(order_b)) => {
                // Both have order, compare by order then by name
                order_a.cmp(&order_b).then_with(|| a.name.cmp(b.name))
            },
            (Some(_), None) => std::cmp::Ordering::Less, // With order comes before without
            (None, Some(_)) => std::cmp::Ordering::Greater, // Without order comes after with
            (None, None) => {
                // Neither has order, sort by section name (if present) then by story name
                match (&a.section, &b.section) {
                    (Some(sec_a), Some(sec_b)) => sec_a.cmp(sec_b).then_with(|| a.name.cmp(b.name)),
                    (Some(_), None) => std::cmp::Ordering::Less,
                    (None, Some(_)) => std::cmp::Ordering::Greater,
                    (None, None) => a.name.cmp(b.name),
                }
            },
        }
    });

    entries
        .into_iter()
        .map(|entry| {
            let section_info = entry
                .section
                .as_ref()
                .map(|s| format!(", section: \"{}\"", s))
                .unwrap_or_default();

            tracing::info!(
                "Story: {}{} ({}:{})",
                entry.name,
                section_info,
                entry.file,
                entry.line
            );

            let container = (entry.create_fn)(window, cx);
            if let Some(section) = entry.section {
                container.update(cx, |c, _| {
                    c.section = Some(section.into());
                });
            }
            container
        })
        .collect()
}
