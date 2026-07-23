mod font_size_selector;

use crate::app_menus;
use font_size_selector::FontSizeSelector;
use gpui::{
    AnyElement, App, AppContext as _, Context, Entity, InteractiveElement as _, IntoElement,
    MouseButton, ParentElement as _, Render, SharedString, Styled as _, Window, div,
};
use gpui_component::{TitleBar, menu::AppMenuBar};
use std::rc::Rc;

use crate::storybook_window_ui::StorybookWindowUi;

pub struct AppTitleBar {
    app_menu_bar: Entity<AppMenuBar>,
    font_size_selector: Entity<FontSizeSelector>,
    child: Rc<dyn Fn(&mut Window, &mut App) -> AnyElement>,
}

impl AppTitleBar {
    pub fn new(
        title: impl Into<SharedString>,
        ui: StorybookWindowUi,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let app_menu_bar = app_menus::init(title, ui.app_menu_items.clone(), cx);
        let font_size_selector = cx.new(|cx| FontSizeSelector::new(window, cx));

        Self {
            app_menu_bar,
            font_size_selector,
            child: ui
                .title_bar_items
                .unwrap_or_else(|| Rc::new(|_, _| div().into_any_element())),
        }
    }

    pub fn child<F, E>(mut self, f: F) -> Self
    where
        E: IntoElement,
        F: Fn(&mut Window, &mut App) -> E + 'static,
    {
        self.child = Rc::new(move |window, cx| f(window, cx).into_any_element());
        self
    }
}

impl Render for AppTitleBar {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        TitleBar::new()
            .child(div().flex().items_center().child(self.app_menu_bar.clone()))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_end()
                    .px_2()
                    .gap_2()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child((self.child.clone())(window, cx))
                    .child(self.font_size_selector.clone()),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn init_test_globals(cx: &mut App) {
        gpui_component::init(cx);
    }

    #[gpui::test]
    fn title_bar_builds_default_and_custom_content(cx: &mut App) {
        init_test_globals(cx);
        let custom: gpui::WindowHandle<AppTitleBar> = cx
            .open_window(Default::default(), |window, cx| {
                cx.new(|cx| {
                    AppTitleBar::new(
                        "Storybook",
                        StorybookWindowUi::new()
                            .with_title_bar_items(|_, _| div().child("Initial")),
                        window,
                        cx,
                    )
                    .child(|_, _| div().child("Override"))
                })
            })
            .expect("custom title bar should open");
        custom
            .update(cx, |title_bar, _, _| {
                assert_ne!(
                    title_bar.app_menu_bar.entity_id(),
                    title_bar.font_size_selector.entity_id()
                );
            })
            .expect("custom title bar should update");

        let default: gpui::WindowHandle<AppTitleBar> = cx
            .open_window(Default::default(), |window, cx| {
                cx.new(|cx| AppTitleBar::new("Storybook", StorybookWindowUi::default(), window, cx))
            })
            .expect("default title bar should open");
        default
            .update(cx, |title_bar, window, cx| {
                let _ = (title_bar.child.clone())(window, cx);
            })
            .expect("default child should render");
    }
}
