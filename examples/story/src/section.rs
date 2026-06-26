use gpui::{
    AnyElement, App, Div, IntoElement, ParentElement, RenderOnce, SharedString, StyleRefinement,
    Styled, Window, rems,
};

use gpui_component::{
    ActiveTheme as _,
    group_box::{GroupBox, GroupBoxVariants as _},
    h_flex,
};

#[derive(IntoElement)]
pub struct StorySection {
    base: Div,
    title: SharedString,
    capture_key: Option<SharedString>,
    sub_title: Vec<AnyElement>,
    children: Vec<AnyElement>,
}

impl StorySection {
    pub fn sub_title(mut self, sub_title: impl IntoElement) -> Self {
        self.sub_title.push(sub_title.into_any_element());
        self
    }

    #[allow(unused)]
    pub fn max_w_md(mut self) -> Self {
        self.base = self.base.max_w(rems(48.));
        self
    }

    #[allow(unused)]
    pub fn max_w_lg(mut self) -> Self {
        self.base = self.base.max_w(rems(64.));
        self
    }

    #[allow(unused)]
    pub fn max_w_xl(mut self) -> Self {
        self.base = self.base.max_w(rems(80.));
        self
    }

    #[allow(unused)]
    pub fn max_w_2xl(mut self) -> Self {
        self.base = self.base.max_w(rems(96.));
        self
    }
}

impl ParentElement for StorySection {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl Styled for StorySection {
    fn style(&mut self) -> &mut gpui::StyleRefinement {
        self.base.style()
    }
}

impl RenderOnce for StorySection {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let title = self.title.clone();
        let capture_key = self.capture_key.clone();
        let group = GroupBox::new()
            .id(self.title.clone())
            .outline()
            .title(
                h_flex()
                    .justify_between()
                    .w_full()
                    .gap_4()
                    .child(self.title)
                    .children(self.sub_title),
            )
            .content_style(
                StyleRefinement::default()
                    .rounded(cx.theme().radius_lg)
                    .overflow_x_hidden()
                    .items_center()
                    .justify_center(),
            )
            .child(self.base.children(self.children));

        if let Some(capture_key) = capture_key {
            gpui_storybook::capture_substory_with_key(capture_key, group).into_any_element()
        } else {
            gpui_storybook::capture_substory(title, group).into_any_element()
        }
    }
}

pub fn section(title: impl Into<gpui_storybook::StorySectionTitle>) -> StorySection {
    let title = title.into();
    let (title, capture_key) = title.into_parts();

    StorySection {
        title,
        capture_key,
        sub_title: vec![],
        base: h_flex()
            .flex_wrap()
            .justify_center()
            .items_center()
            .w_full()
            .gap_4(),
        children: vec![],
    }
}
