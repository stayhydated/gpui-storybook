use gpui::{
    App, AppContext as _, ClickEvent, Context, ElementId, EventEmitter, InteractiveElement as _,
    IntoElement, ParentElement as _, Render, SharedString, StatefulInteractiveElement as _,
    Styled as _, Window, div, prelude::FluentBuilder as _,
};
use gpui_component::{ActiveTheme as _, Collapsible, StyledExt as _, h_flex, sidebar::SidebarItem};
use std::rc::Rc;

#[derive(Clone)]
pub struct StoryDrag {
    story_klass: SharedString,
    label: SharedString,
}

impl StoryDrag {
    pub fn new(story_klass: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            story_klass: story_klass.into(),
            label: label.into(),
        }
    }

    pub fn story_klass(&self) -> &str {
        self.story_klass.as_ref()
    }
}

impl EventEmitter<()> for StoryDrag {}

impl Render for StoryDrag {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        h_flex()
            .id("storybook-story-drag")
            .h_7()
            .px_2()
            .items_center()
            .border_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().sidebar_accent)
            .text_color(cx.theme().sidebar_accent_foreground)
            .rounded(cx.theme().radius)
            .child(self.label.clone())
    }
}

#[derive(Clone)]
pub struct StorySidebarItem {
    label: SharedString,
    story_klass: SharedString,
    handler: Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>,
    active: bool,
    collapsed: bool,
    disabled: bool,
    indented: bool,
    section_heading: bool,
}

impl StorySidebarItem {
    pub fn new(label: impl Into<SharedString>, story_klass: impl Into<SharedString>) -> Self {
        Self {
            label: label.into(),
            story_klass: story_klass.into(),
            handler: Rc::new(|_, _, _| {}),
            active: false,
            collapsed: false,
            disabled: false,
            indented: false,
            section_heading: false,
        }
    }

    pub fn on_click(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.handler = Rc::new(handler);
        self
    }

    pub fn active(mut self, active: bool) -> Self {
        self.active = active;
        self
    }

    pub fn disable(mut self, disable: bool) -> Self {
        self.disabled = disable;
        self
    }

    pub fn indented(mut self, indented: bool) -> Self {
        self.indented = indented;
        self
    }

    pub fn section_heading(mut self, section_heading: bool) -> Self {
        self.section_heading = section_heading;
        self
    }
}

impl Collapsible for StorySidebarItem {
    fn is_collapsed(&self) -> bool {
        self.collapsed
    }

    fn collapsed(mut self, collapsed: bool) -> Self {
        self.collapsed = collapsed;
        self
    }
}

impl SidebarItem for StorySidebarItem {
    fn render(
        self,
        id: impl Into<ElementId>,
        _window: &mut Window,
        cx: &mut App,
    ) -> impl IntoElement {
        let id = id.into();
        let is_hoverable = !self.active && !self.disabled;
        let handler = self.handler.clone();
        let drag = StoryDrag::new(self.story_klass.clone(), self.label.clone());

        div().id(id).w_full().child(
            h_flex()
                .size_full()
                .id("item")
                .overflow_x_hidden()
                .flex_shrink_0()
                .p_2()
                .gap_x_2()
                .rounded(cx.theme().radius)
                .text_sm()
                .when(self.indented && !self.collapsed, |this| this.pl_6())
                .when(is_hoverable, |this| {
                    this.hover(|this| {
                        this.bg(cx.theme().sidebar_accent.opacity(0.8))
                            .text_color(cx.theme().sidebar_accent_foreground)
                    })
                })
                .when(self.active, |this| {
                    this.font_medium()
                        .bg(cx.theme().sidebar_accent)
                        .text_color(cx.theme().sidebar_accent_foreground)
                })
                .when(self.section_heading, |this| {
                    this.h_6()
                        .text_xs()
                        .font_medium()
                        .text_color(cx.theme().muted_foreground)
                })
                .when(self.collapsed, |this| this.justify_center())
                .when(!self.collapsed, |this| {
                    this.when(!self.section_heading, |this| this.h_7()).child(
                        h_flex()
                            .flex_1()
                            .gap_x_2()
                            .justify_between()
                            .overflow_x_hidden()
                            .child(
                                h_flex()
                                    .flex_1()
                                    .overflow_x_hidden()
                                    .child(self.label.clone()),
                            ),
                    )
                })
                .when(self.disabled, |this| {
                    this.text_color(cx.theme().muted_foreground)
                })
                .when(!self.disabled, |this| {
                    this.on_click(move |ev, window, cx| {
                        handler(ev, window, cx);
                    })
                    .on_drag(drag, |drag, _, _, cx| {
                        cx.stop_propagation();
                        cx.new(|_| drag.clone())
                    })
                }),
        )
    }
}
