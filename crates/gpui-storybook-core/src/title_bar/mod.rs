mod font_size_selector;

use crate::app_menus;
use font_size_selector::FontSizeSelector;
use gpui::{
    AnyElement, App, AppContext as _, Context, Entity, InteractiveElement as _, IntoElement,
    MouseButton, ParentElement as _, Render, SharedString, Styled as _, Window, div,
};
use gpui_component::{TitleBar, menu::AppMenuBar};
use std::rc::Rc;

pub struct AppTitleBar {
    app_menu_bar: Entity<AppMenuBar>,
    font_size_selector: Entity<FontSizeSelector>,
    child: Rc<dyn Fn(&mut Window, &mut App) -> AnyElement>,
}

impl AppTitleBar {
    pub fn new(
        title: impl Into<SharedString>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let app_menu_bar = app_menus::init(title, cx);
        let font_size_selector = cx.new(|cx| FontSizeSelector::new(window, cx));

        Self {
            app_menu_bar,
            font_size_selector,
            child: Rc::new(|_, _| div().into_any_element()),
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
