#[cfg(feature = "macros")]
pub use gpui_storybook_macros::*;

pub use gpui_storybook_core::assets::Assets;
pub use gpui_storybook_core::gallery::Gallery;
pub use gpui_storybook_core::i18n::change_locale;
pub use gpui_storybook_core::story::{Story, StoryContainer, create_new_window, init};

pub use gpui_storybook_core::registry as __registry;

pub mod story_registry {
    use gpui_storybook_core::registry;
    use gpui_storybook_core::story::StoryContainer;

    pub fn init(cx: &mut ::gpui::App) {
        for entry in inventory::iter::<registry::InitEntry> {
            (entry.init_fn)(cx);
        }
    }

    pub fn generate_stories(
        window: &mut ::gpui::Window,
        cx: &mut ::gpui::App,
    ) -> Vec<::gpui::Entity<StoryContainer>> {
        inventory::iter::<registry::StoryEntry>()
            .map(|entry| (entry.create_fn)(window, cx))
            .collect()
    }
}
