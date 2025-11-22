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
    for entry in inventory::iter::<__registry::InitEntry> {
        (entry.init_fn)(cx);
    }
}

pub fn generate_stories(
    window: &mut ::gpui::Window,
    cx: &mut ::gpui::App,
) -> Vec<::gpui::Entity<StoryContainer>> {
    inventory::iter::<__registry::StoryEntry>()
        .map(|entry| (entry.create_fn)(window, cx))
        .collect()
}
