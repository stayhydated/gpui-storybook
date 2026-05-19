use gpui::{
    Action, AnyElement, AnyView, App, AppContext as _, ClickEvent, Div, Entity, EventEmitter,
    Focusable, Hsla, InteractiveElement as _, IntoElement, ParentElement, Render, RenderOnce,
    SharedString, StyleRefinement, Styled, Window, div, prelude::FluentBuilder as _, rems,
};

use serde::{Deserialize, Serialize};
use std::{borrow::Borrow, sync::Arc};

use gpui_component::{
    ActiveTheme as _, IconName, Sizable as _,
    button::{Button, ButtonVariants as _},
    dock::{Panel, PanelControl, PanelEvent, PanelInfo, PanelState, PanelView, TitleStyle},
    group_box::{GroupBox, GroupBoxVariants as _},
    h_flex,
    menu::PopupMenu,
    scroll::ScrollableElement as _,
    v_flex,
};

use super::state::AppState;

pub const STORY_LIST_KLASS_PREFIX: &str = "__gpui_storybook_list__:";

#[derive(Action, Clone, Debug, Default, Eq, PartialEq)]
#[action(namespace = story)]
pub struct ShowPanelInfo;

#[derive(IntoElement)]
pub struct StorySection {
    base: Div,
    title: SharedString,
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
        GroupBox::new()
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
            .child(self.base.children(self.children))
    }
}

pub fn section(title: impl Into<SharedString>) -> StorySection {
    StorySection {
        title: title.into(),
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

pub struct StoryContainer {
    focus_handle: gpui::FocusHandle,
    pub name: SharedString,
    pub group: Option<SharedString>,
    pub section: Option<SharedString>,
    pub title_bg: Option<Hsla>,
    pub description: SharedString,
    width: Option<gpui::Pixels>,
    height: Option<gpui::Pixels>,
    tab_panel: Option<gpui::WeakEntity<gpui_component::dock::TabPanel>>,
    story: Option<AnyView>,
    pub story_klass: Option<SharedString>,
    closable: bool,
    is_active: bool,
    zoomable: Option<PanelControl>,
    on_active: Option<fn(AnyView, bool, &mut Window, &mut App)>,
    pub title_fn: Option<Box<dyn Fn(&App) -> String>>,
    pub description_fn: Option<Box<dyn Fn(&App) -> String>>,
}

pub fn story_list_klass(stories: &[Entity<StoryContainer>], cx: &App) -> SharedString {
    let mut klasses = stories
        .iter()
        .filter_map(|story| story.read(cx).story_klass.clone())
        .map(|klass| klass.to_string())
        .collect::<Vec<_>>();
    klasses.sort();

    format!("{}{}", STORY_LIST_KLASS_PREFIX, klasses.join("|")).into()
}

#[cfg(feature = "dock")]
pub fn parse_story_list_klass(story_klass: &str) -> Option<Vec<String>> {
    let members = story_klass.strip_prefix(STORY_LIST_KLASS_PREFIX)?;
    Some(
        members
            .split('|')
            .filter(|member| !member.is_empty())
            .map(str::to_string)
            .collect(),
    )
}

pub struct StoryList {
    focus_handle: gpui::FocusHandle,
    stories: Vec<Entity<StoryContainer>>,
}

impl StoryList {
    pub fn new(stories: Vec<Entity<StoryContainer>>, cx: &mut gpui::Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            stories,
        }
    }

    fn on_active_any(view: AnyView, active: bool, window: &mut Window, cx: &mut App) {
        if let Ok(list) = view.downcast::<Self>() {
            cx.update_entity(&list, |list, cx| {
                for story_entity in &list.stories {
                    story_entity.update(cx, |story, cx| {
                        story.is_active = active;
                        if let Some(on_active) = story.on_active
                            && let Some(story_view) = story.story.clone()
                        {
                            on_active(story_view, active, window, cx);
                        }
                    });
                }
            });
        }
    }
}

impl Focusable for StoryList {
    fn focus_handle(&self, _: &App) -> gpui::FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for StoryList {
    fn render(&mut self, _: &mut Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        v_flex()
            .id("storybook-story-list")
            .w_full()
            .gap_4()
            .children(
                self.stories
                    .iter()
                    .enumerate()
                    .map(|(index, story_entity)| {
                        let story = story_entity.read(cx);
                        let title = story.display_title(cx);
                        let description = story.display_description(cx);
                        let story_klass = story.story_klass.clone().unwrap_or_default();
                        let story_view = story.story.clone();

                        v_flex()
                            .id(format!("storybook-story-list-item-{index}"))
                            .w_full()
                            .border_1()
                            .border_color(cx.theme().border)
                            .rounded(cx.theme().radius)
                            .overflow_hidden()
                            .child(
                                v_flex()
                                    .w_full()
                                    .gap_1()
                                    .p_3()
                                    .border_b_1()
                                    .border_color(cx.theme().border)
                                    .bg(cx.theme().muted.opacity(0.35))
                                    .child(
                                        h_flex().justify_between().gap_3().child(title).child(
                                            div()
                                                .text_xs()
                                                .text_color(cx.theme().muted_foreground)
                                                .child(story_klass),
                                        ),
                                    )
                                    .when(!description.is_empty(), |this| {
                                        this.child(
                                            div()
                                                .text_sm()
                                                .text_color(cx.theme().muted_foreground)
                                                .child(description),
                                        )
                                    }),
                            )
                            .when_some(story_view, |this, story| {
                                this.child(div().w_full().p_4().child(story))
                            })
                    }),
            )
    }
}

#[derive(Debug)]
pub enum ContainerEvent {
    Close,
}

pub trait Story: Focusable + Render + Sized {
    fn klass() -> &'static str {
        let type_name = std::any::type_name::<Self>();
        type_name.rsplit("::").next().unwrap_or(type_name)
    }

    fn title(cx: &App) -> String;
    fn description(cx: &App) -> String {
        let _ = cx;
        "".to_owned()
    }
    fn closable() -> bool {
        true
    }
    fn zoomable() -> Option<PanelControl> {
        Some(PanelControl::default())
    }
    fn title_bg() -> Option<Hsla> {
        None
    }
    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render + Focusable>;

    fn on_active(&mut self, active: bool, window: &mut Window, cx: &mut App) {
        let _ = active;
        let _ = window;
        let _ = cx;
    }
    fn on_active_any(view: AnyView, active: bool, window: &mut Window, cx: &mut App)
    where
        Self: 'static,
    {
        if let Ok(story) = view.downcast::<Self>() {
            cx.update_entity(&story, |story, cx| {
                story.on_active(active, window, cx);
            });
        }
    }
}

impl EventEmitter<ContainerEvent> for StoryContainer {}

impl StoryContainer {
    pub fn new(_window: &mut Window, cx: &mut App) -> Self {
        let focus_handle = cx.focus_handle();

        Self {
            focus_handle,
            name: "".into(),
            group: None,
            section: None,
            title_bg: None,
            description: "".into(),
            width: None,
            height: None,
            tab_panel: None,
            story: None,
            story_klass: None,
            closable: true,
            is_active: false,
            zoomable: Some(PanelControl::default()),
            on_active: None,
            title_fn: None,
            description_fn: None,
        }
    }

    pub fn section(mut self, section: impl Into<SharedString>) -> Self {
        self.section = Some(section.into());
        self
    }

    pub fn group(mut self, group: impl Into<SharedString>) -> Self {
        self.group = Some(group.into());
        self
    }

    pub fn sidebar_group(&self) -> Option<SharedString> {
        self.group.clone().or(self.section.clone())
    }

    pub fn sidebar_section(&self) -> Option<SharedString> {
        match (&self.group, &self.section) {
            (Some(group), Some(section)) if group != section => Some(section.clone()),
            _ => None,
        }
    }

    pub fn panel<S: Story>(window: &mut Window, cx: &mut App) -> Entity<Self> {
        let name = S::title(cx);
        let description = S::description(cx);
        let story = S::new_view(window, cx);
        let story_klass = S::klass();
        let focus_handle = story.focus_handle(cx);

        cx.new(|cx| {
            let mut story = Self::new(window, cx)
                .story(story.into(), story_klass)
                .on_active(S::on_active_any);
            story.focus_handle = focus_handle;
            story.closable = S::closable();
            story.zoomable = S::zoomable();
            story.name = name.into();
            story.description = description.into();
            story.title_bg = S::title_bg();
            story.title_fn = Some(Box::new(S::title));
            story.description_fn = Some(Box::new(S::description));
            story
        })
    }

    pub fn list_panel(
        name: impl Into<SharedString>,
        stories: Vec<Entity<StoryContainer>>,
        window: &mut Window,
        cx: &mut App,
    ) -> Entity<Self> {
        let name = name.into();
        let story_klass = story_list_klass(&stories, cx);
        let description = format!("{} story variants", stories.len());
        let list = cx.new(|cx| StoryList::new(stories, cx));
        let focus_handle = list.focus_handle(cx);

        cx.new(|cx| {
            let mut story = Self::new(window, cx)
                .story(list.into(), story_klass)
                .on_active(StoryList::on_active_any);
            story.focus_handle = focus_handle;
            story.name = name;
            story.description = description.into();
            story
        })
    }

    pub fn width(mut self, width: gpui::Pixels) -> Self {
        self.width = Some(width);
        self
    }

    pub fn height(mut self, height: gpui::Pixels) -> Self {
        self.height = Some(height);
        self
    }

    pub fn story(mut self, story: AnyView, story_klass: impl Into<SharedString>) -> Self {
        self.story = Some(story);
        self.story_klass = Some(story_klass.into());
        self
    }

    pub fn on_active(mut self, on_active: fn(AnyView, bool, &mut Window, &mut App)) -> Self {
        self.on_active = Some(on_active);
        self
    }

    pub fn display_title(&self, cx: &impl Borrow<App>) -> String {
        if let Some(title_fn) = &self.title_fn {
            title_fn(cx.borrow())
        } else {
            self.name.to_string()
        }
    }

    pub fn display_description(&self, cx: &impl Borrow<App>) -> String {
        if let Some(description_fn) = &self.description_fn {
            description_fn(cx.borrow())
        } else {
            self.description.to_string()
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct StoryState {
    pub story_klass: SharedString,
}

impl StoryState {
    fn to_value(&self) -> serde_json::Value {
        serde_json::json!({
            "story_klass": self.story_klass,
        })
    }
}

impl Panel for StoryContainer {
    fn panel_name(&self) -> &'static str {
        "StoryContainer"
    }

    fn title(&mut self, _window: &mut Window, _cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let tab_panel = self.tab_panel.clone();
        let story_panel = _cx.entity().downgrade();
        let title = self.display_title(_cx).into_any_element();

        h_flex()
            .items_center()
            .gap_1()
            .child(title)
            .when(self.closable && self.is_active, |this| {
                this.child(
                    Button::new(format!(
                        "close-story-tab-{}",
                        self.story_klass.clone().unwrap_or_default()
                    ))
                    .icon(IconName::Close)
                    .xsmall()
                    .ghost()
                    .tab_stop(false)
                    .on_click(
                        move |_: &ClickEvent, window: &mut Window, cx: &mut App| {
                            cx.stop_propagation();
                            let Some(tab_panel) = tab_panel.clone().and_then(|tab| tab.upgrade())
                            else {
                                return;
                            };
                            let Some(story_panel) = story_panel.upgrade() else {
                                return;
                            };
                            tab_panel.update(cx, |tab_panel, cx| {
                                tab_panel.remove_panel(Arc::new(story_panel.clone()), window, cx);
                            });
                        },
                    ),
                )
            })
    }

    fn title_style(&self, cx: &App) -> Option<TitleStyle> {
        self.title_bg.map(|bg| TitleStyle {
            background: bg,
            foreground: cx.theme().foreground,
        })
    }

    fn closable(&self, _cx: &App) -> bool {
        self.closable
    }

    fn zoomable(&self, _cx: &App) -> Option<PanelControl> {
        self.zoomable
    }

    fn visible(&self, cx: &App) -> bool {
        !AppState::global(cx)
            .invisible_panels
            .read(cx)
            .contains(&self.name)
    }

    fn set_zoomed(&mut self, zoomed: bool, _window: &mut Window, _cx: &mut gpui::Context<Self>) {
        println!("panel: {} zoomed: {}", self.name, zoomed);
    }

    fn set_active(&mut self, active: bool, _window: &mut Window, cx: &mut gpui::Context<Self>) {
        println!("panel: {} active: {}", self.name, active);
        self.is_active = active;
        if let Some(on_active) = self.on_active
            && let Some(story) = self.story.clone()
        {
            on_active(story, active, _window, cx);
        }
    }

    fn on_added_to(
        &mut self,
        tab_panel: gpui::WeakEntity<gpui_component::dock::TabPanel>,
        _window: &mut Window,
        _cx: &mut gpui::Context<Self>,
    ) {
        self.tab_panel = Some(tab_panel);
    }

    fn on_removed(&mut self, _window: &mut Window, _cx: &mut gpui::Context<Self>) {
        self.tab_panel = None;
        self.is_active = false;
    }

    fn dropdown_menu(
        &mut self,
        menu: PopupMenu,
        _window: &mut Window,
        _cx: &mut gpui::Context<Self>,
    ) -> PopupMenu {
        menu.menu("Info", Box::new(ShowPanelInfo))
    }

    fn dump(&self, _cx: &App) -> PanelState {
        let mut state = PanelState::new(self);
        if let Some(story_klass) = self.story_klass.clone() {
            let story_state = StoryState { story_klass };
            state.info = PanelInfo::panel(story_state.to_value());
        }
        state
    }
}

pub fn reveal_story_panel(
    story: &Entity<StoryContainer>,
    window: &mut Window,
    cx: &mut App,
) -> bool {
    let (is_active, tab_panel) = {
        let story = story.read(cx);
        (story.is_active, story.tab_panel.clone())
    };

    if is_active {
        return true;
    }

    let Some(tab_panel) = tab_panel.and_then(|tab| tab.upgrade()) else {
        return false;
    };

    let panel: Arc<dyn PanelView> = Arc::new(story.clone());
    tab_panel.update(cx, |tab_panel, cx| {
        tab_panel.remove_panel(panel.clone(), window, cx);
        tab_panel.add_panel(panel, window, cx);
    });

    true
}

impl EventEmitter<PanelEvent> for StoryContainer {}
impl Focusable for StoryContainer {
    fn focus_handle(&self, _: &App) -> gpui::FocusHandle {
        self.focus_handle.clone()
    }
}
impl Render for StoryContainer {
    fn render(&mut self, _: &mut Window, _cx: &mut gpui::Context<Self>) -> impl IntoElement {
        div()
            .id("story-container")
            .size_full()
            .overflow_y_scrollbar()
            .track_focus(&self.focus_handle)
            .when_some(self.story.clone(), |this, story| {
                this.child(div().size_full().p_4().child(story))
            })
    }
}
