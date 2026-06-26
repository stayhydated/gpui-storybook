use gpui::{
    AnyElement, App, AppContext as _, Context, Entity, FocusHandle, Focusable, IntoElement,
    ParentElement, Render, RenderOnce, SharedString, Styled as _, Window, div, px, rems,
};
use gpui_component::{ActiveTheme as _, h_flex, v_flex};
use gpui_storybook::{StorySectionBase, StorySectionTitle};

#[derive(gpui_storybook::Substory)]
enum CustomSectionSubstory {
    ProductMetrics,
    #[substory(title = "Health Signals")]
    HealthSignals,
}

#[derive(IntoElement)]
struct MetricSection {
    base: StorySectionBase,
    eyebrow: SharedString,
    children: Vec<AnyElement>,
}

fn metric_section(
    title: impl Into<StorySectionTitle>,
    eyebrow: impl Into<SharedString>,
) -> MetricSection {
    MetricSection {
        base: StorySectionBase::new(title),
        eyebrow: eyebrow.into(),
        children: Vec::new(),
    }
}

impl ParentElement for MetricSection {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl RenderOnce for MetricSection {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let base = self.base;
        let title = base.title().clone();
        let section = v_flex()
            .gap_4()
            .w_full()
            .max_w(rems(48.))
            .p_4()
            .border_1()
            .border_color(cx.theme().border)
            .rounded(cx.theme().radius_lg)
            .child(
                h_flex()
                    .justify_between()
                    .items_start()
                    .gap_4()
                    .child(
                        v_flex()
                            .gap_1()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(self.eyebrow),
                            )
                            .child(div().text_lg().child(title)),
                    )
                    .child(
                        div()
                            .text_xs()
                            .px_2()
                            .py_1()
                            .rounded(cx.theme().radius)
                            .bg(cx.theme().muted.opacity(0.35))
                            .child("Live"),
                    ),
            )
            .child(h_flex().gap_3().flex_wrap().children(self.children));

        base.capture(section)
    }
}

fn metric_tile(
    label: &'static str,
    value: &'static str,
    detail: &'static str,
    cx: &App,
) -> AnyElement {
    v_flex()
        .gap_1()
        .min_w(px(144.))
        .flex_1()
        .p_3()
        .rounded(cx.theme().radius)
        .bg(cx.theme().muted.opacity(0.35))
        .child(
            div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(label),
        )
        .child(div().text_lg().child(value))
        .child(
            div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(detail),
        )
        .into_any_element()
}

#[gpui_storybook::story(crate::StorySection::CustomSections)]
pub struct CustomSectionStory {
    focus_handle: FocusHandle,
}

impl CustomSectionStory {
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| Self {
            focus_handle: cx.focus_handle(),
        })
    }
}

impl gpui_storybook::Story for CustomSectionStory {
    fn title(_: &App) -> String {
        "Custom Section".into()
    }

    fn description(_: &App) -> String {
        "Custom section component using StorySectionBase".into()
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render + Focusable> {
        Self::view(window, cx)
    }
}

impl Focusable for CustomSectionStory {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for CustomSectionStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .gap_6()
            .size_full()
            .items_center()
            .child(
                metric_section(CustomSectionSubstory::ProductMetrics, "Revenue")
                    .child(metric_tile("MRR", "$42.8k", "+8.2% month over month", cx))
                    .child(metric_tile("Activation", "71%", "+3.4% from last week", cx))
                    .child(metric_tile("Expansion", "$6.1k", "24 active accounts", cx)),
            )
            .child(
                metric_section(CustomSectionSubstory::HealthSignals, "Operations")
                    .child(metric_tile("Latency", "42 ms", "p95 over 24 hours", cx))
                    .child(metric_tile("Errors", "0.08%", "-12 incidents", cx))
                    .child(metric_tile("Queue", "Clear", "all workers available", cx)),
            )
    }
}
