use gpui::{AnyElement, App, Entity, IntoElement, MenuItem, Window};
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

pub struct StorybookWindow<V> {
    pub(crate) view: Entity<V>,
    pub(crate) ui: StorybookWindowUi,
}

impl<V> StorybookWindow<V> {
    pub fn new(view: Entity<V>) -> Self {
        Self {
            view,
            ui: StorybookWindowUi::default(),
        }
    }

    pub fn with_ui(mut self, ui: StorybookWindowUi) -> Self {
        self.ui = ui;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{AppContext as _, Context, ParentElement as _, Render, div};

    struct TestView;

    impl Render for TestView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div()
        }
    }

    #[gpui::test]
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

        let view = cx.new(|_| TestView);
        let wrapper = StorybookWindow::new(view.clone()).with_ui(ui);
        assert_eq!(wrapper.view, view);
        assert!(wrapper.ui.app_menu_items.is_some());
        assert!(wrapper.ui.title_bar_items.is_some());
    }
}
