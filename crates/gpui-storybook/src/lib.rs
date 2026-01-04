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

pub fn init<L: Language>(language: L, cx: &mut ::gpui::App) {
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

    inventory::iter::<__registry::StoryEntry>()
        .map(|entry| {
            let section_info = entry
                .section
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
