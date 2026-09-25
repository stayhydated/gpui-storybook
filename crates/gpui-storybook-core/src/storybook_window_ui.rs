use gpui_kit::{AnyElement, App, Entity, IntoElement, MenuItem, Window};

use crate::story::StoryContainer;
use std::rc::Rc;

pub type AppMenuItemsBuilder = Rc<dyn Fn(&App) -> Vec<MenuItem>>;
pub type TitleBarItemsBuilder = Rc<dyn Fn(&mut Window, &mut App) -> AnyElement>;

#[derive(Clone, Default)]
pub struct StorybookWindowUi {
    pub(crate) app_menu_items: Option<AppMenuItemsBuilder>,
    pub(crate) title_bar_items: Option<TitleBarItemsBuilder>,
}

impl StorybookWindowUi {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_app_menu_items<F>(mut self, build: F) -> Self
    where
        F: Fn(&App) -> Vec<MenuItem> + 'static,
    {
        self.app_menu_items = Some(Rc::new(build));
        self
    }

    pub fn with_title_bar_items<F, E>(mut self, render: F) -> Self
    where
        E: IntoElement,
        F: Fn(&mut Window, &mut App) -> E + 'static,
    {
        self.title_bar_items = Some(Rc::new(move |window, cx| {
            render(window, cx).into_any_element()
        }));
        self
    }
}

/// Stories and window-level presentation used by [`create_storybook_window`].
///
/// Construct this in the window callback after generating stories.
///
/// [`create_storybook_window`]: crate::story::create_storybook_window
pub struct StorybookWindow {
    pub(crate) stories: Vec<Entity<StoryContainer>>,
    pub(crate) ui: StorybookWindowUi,
}

impl StorybookWindow {
    /// Creates a standard Storybook window from generated stories.
    pub fn new(stories: Vec<Entity<StoryContainer>>) -> Self {
        Self {
            stories,
            ui: StorybookWindowUi::default(),
        }
    }

    /// Adds application-owned menu and title-bar content.
    pub fn with_ui(mut self, ui: StorybookWindowUi) -> Self {
        self.ui = ui;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui_kit::{ParentElement as _, div};

    #[gpui_kit::test]
    fn window_ui_and_wrapper_preserve_custom_builders(cx: &mut App) {
        let default_ui = StorybookWindowUi::new();
        assert!(default_ui.app_menu_items.is_none());
        assert!(default_ui.title_bar_items.is_none());

        let ui = default_ui
            .with_app_menu_items(|_| Vec::new())
            .with_title_bar_items(|_, _| div().child("Custom"));
        assert!(
            ui.app_menu_items
                .as_ref()
                .expect("menu builder should exist")(cx)
            .is_empty()
        );
        assert!(ui.title_bar_items.is_some());

        let wrapper = StorybookWindow::new(Vec::new()).with_ui(ui);
        assert!(wrapper.stories.is_empty());
        assert!(wrapper.ui.app_menu_items.is_some());
        assert!(wrapper.ui.title_bar_items.is_some());
    }
}
