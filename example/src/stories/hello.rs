use es_fluent::{EsFluent, ToFluentString as _};
use gpui::*;
use gpui_component::{
    StyledExt as _,
    button::{Button, ButtonVariants as _},
};

#[derive(EsFluent)]
enum StoryItems {
    Title,
    SubTitle,
    ButtonLabel,
    ButtonOnClick,
}

#[gpui_storybook::story]
pub struct HelloWorld {
    focus_handle: FocusHandle,
}

impl Focusable for HelloWorld {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl gpui_storybook::Story for HelloWorld {
    fn title() -> String {
        StoryItems::Title.to_fluent_string()
    }
    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render + Focusable> {
        Self::view(window, cx)
    }
}

impl HelloWorld {
    pub fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| Self::new(window, cx))
    }
    pub fn new(_window: &mut Window, cx: &mut App) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
        }
    }
}

impl Render for HelloWorld {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .v_flex()
            .gap_2()
            .size_full()
            .items_center()
            .justify_center()
            .text_center()
            .child(StoryItems::SubTitle.to_fluent_string())
            .child(
                Button::new("ok")
                    .primary()
                    .label(StoryItems::ButtonLabel.to_fluent_string())
                    .on_click(|_, _, _| {
                        println!("{}", StoryItems::ButtonOnClick.to_fluent_string())
                    }),
            )
    }
}
