use es_fluent::{EsFluent, ToFluentString as _};
use gpui::*;
use gpui_component::StyledExt as _;

#[derive(EsFluent)]
enum Story2Items {
    Title,
    Hi,
}

#[gpui_storybook::story]
pub struct HelloWorld2 {
    focus_handle: FocusHandle,
}

impl Focusable for HelloWorld2 {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl gpui_storybook::Story for HelloWorld2 {
    fn title() -> String {
        Story2Items::Title.to_fluent_string()
    }
    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render + Focusable> {
        Self::view(window, cx)
    }
}

impl HelloWorld2 {
    pub fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| Self::new(window, cx))
    }
    pub fn new(_window: &mut Window, cx: &mut App) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
        }
    }
}

impl Render for HelloWorld2 {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .v_flex()
            .gap_2()
            .size_full()
            .items_center()
            .justify_center()
            .text_center()
            .child(Story2Items::Hi.to_fluent_string())
    }
}
