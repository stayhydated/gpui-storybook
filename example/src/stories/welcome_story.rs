use es_fluent::{EsFluent, ToFluentString as _};
use gpui::{App, AppContext, Context, Entity, FocusHandle, Focusable, Render, Styled as _, Window};

use gpui_component::text::markdown;

#[gpui_storybook::story("readme")]
pub struct WelcomeStory {
    focus_handle: FocusHandle,
}

#[derive(EsFluent)]
enum WelcomeStoryItems {
    Title,
}

impl WelcomeStory {
    pub fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| Self::new(window, cx))
    }

    fn new(_: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
        }
    }
}

impl gpui_storybook::Story for WelcomeStory {
    fn title() -> String {
        WelcomeStoryItems::Title.to_fluent_string()
    }
    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render + Focusable> {
        Self::view(window, cx)
    }
}

impl Focusable for WelcomeStory {
    fn focus_handle(&self, _: &gpui::App) -> gpui::FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for WelcomeStory {
    fn render(
        &mut self,
        _: &mut gpui::Window,
        _: &mut gpui::Context<Self>,
    ) -> impl gpui::IntoElement {
        markdown(include_str!("../../../README.md"))
            .px_4()
            .scrollable(true)
            .selectable(true)
    }
}
