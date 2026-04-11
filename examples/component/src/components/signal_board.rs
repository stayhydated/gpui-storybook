use gpui::{
    App, IntoElement, ParentElement as _, RenderOnce, SharedString, Styled as _, Window, div, px,
};
use gpui_component::{ActiveTheme as _, Sizable as _, h_flex, tag::Tag, v_flex};

#[derive(gpui_storybook::ComponentStory, IntoElement)]
#[storybook(
    title = "Signal Board",
    description = "A custom dashboard strip with no relation to the story example widgets",
    section = crate::StorySection::Signals,
    example = SignalBoard::example(),
)]
pub struct SignalBoard {
    headline: SharedString,
    tiles: Vec<SignalTile>,
}

impl SignalBoard {
    pub fn new(headline: impl Into<SharedString>, tiles: Vec<SignalTile>) -> Self {
        Self {
            headline: headline.into(),
            tiles,
        }
    }

    pub fn example() -> Self {
        Self::new(
            "Tonight's room tone",
            vec![
                SignalTile::new("Doors", "128", "steady foot traffic", SignalState::Warm),
                SignalTile::new(
                    "Merch",
                    "$2.4k",
                    "up 18% from yesterday",
                    SignalState::Bright,
                ),
                SignalTile::new(
                    "Queue",
                    "14 min",
                    "falling after the opener",
                    SignalState::Calm,
                ),
            ],
        )
    }
}

impl RenderOnce for SignalBoard {
    fn render(self, _: &mut Window, cx: &mut App) -> impl gpui::IntoElement {
        v_flex()
            .gap_4()
            .max_w(px(760.))
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(self.headline),
            )
            .child(
                h_flex()
                    .gap_3()
                    .flex_wrap()
                    .children(self.tiles.into_iter().map(|tile| tile.into_card(cx))),
            )
    }
}

pub struct SignalTile {
    label: SharedString,
    value: SharedString,
    note: SharedString,
    state: SignalState,
}

impl SignalTile {
    pub fn new(
        label: impl Into<SharedString>,
        value: impl Into<SharedString>,
        note: impl Into<SharedString>,
        state: SignalState,
    ) -> Self {
        Self {
            label: label.into(),
            value: value.into(),
            note: note.into(),
            state,
        }
    }

    fn into_card(self, cx: &App) -> impl gpui::IntoElement {
        let badge = self.state.badge();

        div()
            .p_4()
            .w(px(230.))
            .border_1()
            .border_color(cx.theme().border)
            .rounded(cx.theme().radius)
            .bg(cx.theme().background)
            .child(badge)
            .child(
                div()
                    .mt_3()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(self.label),
            )
            .child(
                div()
                    .mt_1()
                    .text_xl()
                    .font_weight(gpui::FontWeight::BOLD)
                    .child(self.value),
            )
            .child(
                div()
                    .mt_2()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(self.note),
            )
    }
}

pub enum SignalState {
    Bright,
    Warm,
    Calm,
}

impl SignalState {
    fn badge(&self) -> Tag {
        match self {
            Self::Bright => Tag::success().outline().child("Bright"),
            Self::Warm => Tag::warning().outline().child("Warm"),
            Self::Calm => Tag::new().outline().child("Calm"),
        }
        .xsmall()
    }
}
