use gpui::{
    App, IntoElement, ParentElement as _, RenderOnce, SharedString, Styled as _, Window, div, px,
};
use gpui_component::ActiveTheme as _;

#[derive(gpui_storybook::ComponentStory, IntoElement)]
#[storybook(
    title = "Welcome Card",
    description = String::from("A quiet editorial card registered without a custom story view"),
    section = crate::StorySection::Intro,
    example = WelcomeCard::example(),
)]
pub struct WelcomeCard {
    eyebrow: SharedString,
    title: SharedString,
    message: SharedString,
}

impl WelcomeCard {
    pub fn new(
        eyebrow: impl Into<SharedString>,
        title: impl Into<SharedString>,
        message: impl Into<SharedString>,
    ) -> Self {
        Self {
            eyebrow: eyebrow.into(),
            title: title.into(),
            message: message.into(),
        }
    }

    pub fn example() -> Self {
        Self::new(
            "Residency",
            "Component registration should feel invisible.",
            "Storybook owns the wrapper view; the component only owns its own markup and data.",
        )
    }
}

impl RenderOnce for WelcomeCard {
    fn render(self, _: &mut Window, cx: &mut App) -> impl gpui::IntoElement {
        div()
            .p_6()
            .max_w(px(520.))
            .border_1()
            .border_color(cx.theme().border)
            .rounded(cx.theme().radius_lg)
            .bg(cx.theme().secondary)
            .text_color(cx.theme().foreground)
            .child(div().text_sm().child(self.eyebrow))
            .child(
                div()
                    .mt_3()
                    .text_xl()
                    .font_weight(gpui::FontWeight::BOLD)
                    .child(self.title),
            )
            .child(
                div()
                    .mt_2()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(self.message),
            )
    }
}
