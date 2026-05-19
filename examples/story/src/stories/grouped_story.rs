use gpui::{
    App, AppContext as _, Context, Entity, FocusHandle, Focusable, IntoElement, ParentElement as _,
    Render, Styled as _, Window, div, px,
};
use gpui_component::{ActiveTheme as _, h_flex, v_flex};

enum GroupedVariant {
    Summary,
    Details,
}

fn metric(label: &'static str, value: &'static str) -> impl IntoElement {
    v_flex()
        .gap_1()
        .p_3()
        .border_1()
        .rounded(px(6.))
        .child(div().text_xs().child(label))
        .child(div().text_lg().child(value))
}

#[gpui_storybook::story(crate::StorySection::Grouped)]
pub struct GroupedSummaryStory {
    focus_handle: FocusHandle,
}

impl GroupedSummaryStory {
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| Self {
            focus_handle: cx.focus_handle(),
        })
    }
}

impl gpui_storybook::Story for GroupedSummaryStory {
    fn title(_: &App) -> String {
        "Grouped Story".into()
    }

    fn description(_: &App) -> String {
        "Summary variant".into()
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render + Focusable> {
        Self::view(window, cx)
    }
}

impl Focusable for GroupedSummaryStory {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for GroupedSummaryStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        render_variant(GroupedVariant::Summary, cx)
    }
}

#[gpui_storybook::story(crate::StorySection::Grouped)]
pub struct GroupedDetailsStory {
    focus_handle: FocusHandle,
}

impl GroupedDetailsStory {
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| Self {
            focus_handle: cx.focus_handle(),
        })
    }
}

impl gpui_storybook::Story for GroupedDetailsStory {
    fn title(_: &App) -> String {
        "Grouped Story".into()
    }

    fn description(_: &App) -> String {
        "Details variant".into()
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render + Focusable> {
        Self::view(window, cx)
    }
}

impl Focusable for GroupedDetailsStory {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for GroupedDetailsStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        render_variant(GroupedVariant::Details, cx)
    }
}

fn render_variant(variant: GroupedVariant, cx: &App) -> impl IntoElement {
    let (title, accent, children) = match variant {
        GroupedVariant::Summary => (
            "Runtime summary",
            cx.theme().magenta,
            vec![
                metric("Requests", "128").into_any_element(),
                metric("Errors", "2").into_any_element(),
                metric("Latency", "14 ms").into_any_element(),
            ],
        ),
        GroupedVariant::Details => (
            "Runtime details",
            cx.theme().magenta,
            vec![
                metric("Queue", "Clear").into_any_element(),
                metric("Workers", "4").into_any_element(),
                metric("Uptime", "03:42").into_any_element(),
            ],
        ),
    };

    v_flex()
        .gap_4()
        .p_4()
        .border_1()
        .border_color(cx.theme().border)
        .rounded(cx.theme().radius_lg)
        .child(
            h_flex()
                .gap_3()
                .items_center()
                .child(div().w(px(10.)).h(px(10.)).rounded_full().bg(accent))
                .child(div().text_lg().child(title)),
        )
        .child(h_flex().gap_3().children(children))
}
