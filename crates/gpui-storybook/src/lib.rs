#[cfg(feature = "macros")]
pub use gpui_storybook_macros::*;

pub use gpui_storybook_core::assets::Assets;
pub use gpui_storybook_core::gallery::Gallery;
pub use gpui_storybook_core::i18n::change_locale;
pub use gpui_storybook_core::story::{Story, StoryContainer, create_new_window};

#[doc(hidden)]
pub use gpui_storybook_core::registry as __registry;

#[doc(hidden)]
pub use inventory as __inventory;

pub fn init(cx: &mut ::gpui::App) {
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
