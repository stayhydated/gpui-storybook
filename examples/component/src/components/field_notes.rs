use gpui::{
    App, IntoElement, ParentElement as _, RenderOnce, SharedString, Styled as _, Window, div, px,
};
use gpui_component::{ActiveTheme as _, Sizable as _, tag::Tag, v_flex};

#[derive(gpui_storybook::ComponentStory, IntoElement)]
#[storybook(
    title = String::from("Field Notes"),
    description = "An annotated stack of note cards with no story-specific machinery",
    section = crate::StorySection::Notes,
    example = FieldNotes::example(),
)]
pub struct FieldNotes {
    heading: SharedString,
    notes: Vec<NoteCard>,
}

impl FieldNotes {
    pub fn new(heading: impl Into<SharedString>, notes: Vec<NoteCard>) -> Self {
        Self {
            heading: heading.into(),
            notes,
        }
    }

    pub fn example() -> Self {
        Self::new(
            "Three things worth keeping",
            vec![
                NoteCard::new(
                    "Crowd",
                    "People stay longer near the side windows.",
                    "The quieter corner turns into the conversation pocket once the lights drop.",
                ),
                NoteCard::new(
                    "Sound",
                    "Low synths travel farther than expected.",
                    "A softer intro gives the room time to settle before the brighter percussion lands.",
                ),
                NoteCard::new(
                    "Print",
                    "The red stock disappears first.",
                    "Next run should start with the poster colorway instead of treating it like the variant.",
                ),
            ],
        )
    }
}

impl RenderOnce for FieldNotes {
    fn render(self, _: &mut Window, cx: &mut App) -> impl gpui::IntoElement {
        v_flex()
            .gap_3()
            .max_w(px(720.))
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(self.heading),
            )
            .children(self.notes.into_iter().map(|note| note.into_card(cx)))
    }
}

pub struct NoteCard {
    category: SharedString,
    title: SharedString,
    body: SharedString,
}

impl NoteCard {
    pub fn new(
        category: impl Into<SharedString>,
        title: impl Into<SharedString>,
        body: impl Into<SharedString>,
    ) -> Self {
        Self {
            category: category.into(),
            title: title.into(),
            body: body.into(),
        }
    }

    fn into_card(self, cx: &App) -> impl gpui::IntoElement {
        div()
            .p_4()
            .border_1()
            .border_color(cx.theme().border)
            .rounded(cx.theme().radius)
            .bg(cx.theme().secondary)
            .child(Tag::new().outline().xsmall().child(self.category))
            .child(
                div()
                    .mt_3()
                    .text_lg()
                    .font_weight(gpui::FontWeight::BOLD)
                    .child(self.title),
            )
            .child(
                div()
                    .mt_2()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(self.body),
            )
    }
}
