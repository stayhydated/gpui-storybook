use gpui::{
    Action, AnyElement, AnyView, App, AppContext as _, ClickEvent, Div, Entity, EventEmitter,
    Focusable, Hsla, InteractiveElement as _, IntoElement, ParentElement, Render, RenderOnce,
    ScrollHandle, SharedString, StatefulInteractiveElement as _, StyleRefinement, Styled, Window,
    div, prelude::FluentBuilder as _, rems,
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
use crate::{
    capture_region::{
        capture_scroll_scope, capture_story_view, capture_story_view_with_scroll, capture_substory,
        capture_substory_with_key, current_capture_scroll_handle,
    },
    registry::{RegisteredStoryMetadata, StoryKey, StoryName},
};

pub const STORY_LIST_KLASS_PREFIX: &str = "__gpui_storybook_list__:";

#[derive(Action, Clone, Debug, Default, Eq, PartialEq)]
#[action(namespace = story)]
pub struct ShowPanelInfo;

/// Stable descriptor for a capture-addressable section inside a story.
///
/// Derive this with `#[derive(gpui_storybook::Substory)]` on a fieldless enum,
/// then pass variants to [`section`] or [`StorySectionBase::new`] so capture
/// routes use stable enum-derived keys instead of display-title slugs.
pub trait Substory: 'static {
    /// Stable route segment used in `story-key/substory-key` capture routes.
    fn capture_key(&self) -> &'static str;

    /// Visible section title shown in the story UI.
    fn title(&self) -> SharedString;
}

/// Input accepted by [`section`] and [`StorySectionBase::new`] for visible
/// titles and stable capture keys.
#[derive(Clone, Debug)]
pub struct StorySectionTitle {
    title: SharedString,
    capture_key: Option<SharedString>,
}

impl StorySectionTitle {
    /// Create a section whose capture key is derived from the visible title.
    pub fn new(title: impl Into<SharedString>) -> Self {
        Self {
            title: title.into(),
            capture_key: None,
        }
    }

    /// Create a section with an explicit stable capture key.
    pub fn with_capture_key(
        capture_key: impl Into<SharedString>,
        title: impl Into<SharedString>,
    ) -> Self {
        Self {
            title: title.into(),
            capture_key: Some(capture_key.into()),
        }
    }

    /// Split the descriptor into its visible title and optional capture key.
    pub fn into_parts(self) -> (SharedString, Option<SharedString>) {
        (self.title, self.capture_key)
    }
}

impl From<&str> for StorySectionTitle {
    fn from(title: &str) -> Self {
        Self::new(title)
    }
}

impl From<String> for StorySectionTitle {
    fn from(title: String) -> Self {
        Self::new(title)
    }
}

impl From<SharedString> for StorySectionTitle {
    fn from(title: SharedString) -> Self {
        Self::new(title)
    }
}

impl<T: Substory> From<T> for StorySectionTitle {
    fn from(substory: T) -> Self {
        Self::with_capture_key(substory.capture_key(), substory.title())
    }
}

/// Base capture metadata for a user-defined story section component.
///
/// Store this inside a custom section component, render the component with the
/// app's own layout and chrome, then call [`capture`](Self::capture) with the
/// rendered element from `RenderOnce`. The styled [`section`] helper uses this
/// same base type internally.
#[derive(Clone, Debug)]
pub struct StorySectionBase {
    title: SharedString,
    capture_key: Option<SharedString>,
}

impl StorySectionBase {
    /// Create capture metadata from a visible title, explicit section title, or
    /// `#[derive(Substory)]` enum variant.
    pub fn new(title: impl Into<StorySectionTitle>) -> Self {
        let (title, capture_key) = title.into().into_parts();

        Self { title, capture_key }
    }

    /// Visible title supplied for this section.
    pub fn title(&self) -> &SharedString {
        &self.title
    }

    /// Explicit stable capture key, when one was supplied by a `Substory`
    /// variant or [`StorySectionTitle::with_capture_key`].
    pub fn capture_key(&self) -> Option<&SharedString> {
        self.capture_key.as_ref()
    }

    /// Wrap a rendered custom section in the capture marker.
    pub fn capture(self, child: impl IntoElement) -> AnyElement {
        if let Some(capture_key) = self.capture_key {
            capture_substory_with_key(capture_key, child).into_any_element()
        } else {
            capture_substory(self.title, child).into_any_element()
        }
    }
}

#[derive(IntoElement)]
pub struct StorySection {
    capture: StorySectionBase,
    base: Div,
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
        let capture = self.capture;
        let title = capture.title().clone();
        let group = GroupBox::new()
            .id(title.clone())
            .outline()
            .title(
                h_flex()
                    .justify_between()
                    .w_full()
                    .gap_4()
                    .child(title)
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

        capture.capture(group)
    }
}

pub fn section(title: impl Into<StorySectionTitle>) -> StorySection {
    StorySection {
        capture: StorySectionBase::new(title),
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
    pub(crate) list_members: Vec<Entity<StoryContainer>>,
    scroll_handle: ScrollHandle,
    width: Option<gpui::Pixels>,
    height: Option<gpui::Pixels>,
    tab_panel: Option<gpui::WeakEntity<gpui_component::dock::TabPanel>>,
    story: Option<AnyView>,
    pub story_klass: Option<SharedString>,
    registration_metadata: Option<RegisteredStoryMetadata>,
    pub story_key: Option<SharedString>,
    pub story_name: Option<SharedString>,
    pub crate_name: Option<SharedString>,
    pub source_file: Option<SharedString>,
    pub source_line: Option<u32>,
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
                        let story_key = story.story_key_label().map(str::to_owned);

                        let item = v_flex()
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
                            });

                        if let Some(story_key) = story_key {
                            capture_story_view_with_scroll(
                                story_key,
                                current_capture_scroll_handle(),
                                item,
                            )
                            .into_any_element()
                        } else {
                            item.into_any_element()
                        }
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
            list_members: Vec::new(),
            scroll_handle: ScrollHandle::new(),
            width: None,
            height: None,
            tab_panel: None,
            story: None,
            story_klass: None,
            registration_metadata: None,
            story_key: None,
            story_name: None,
            crate_name: None,
            source_file: None,
            source_line: None,
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
        let list_members = stories.clone();
        let list = cx.new(|cx| StoryList::new(stories, cx));
        let focus_handle = list.focus_handle(cx);

        cx.new(|cx| {
            let mut story = Self::new(window, cx)
                .story(list.into(), story_klass)
                .on_active(StoryList::on_active_any);
            story.focus_handle = focus_handle;
            story.name = name;
            story.description = description.into();
            story.list_members = list_members;
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

    /// Store typed registry metadata on this runtime container.
    ///
    /// This also keeps the legacy string metadata fields populated for callers
    /// that still read them directly.
    pub fn set_registration_metadata(&mut self, metadata: RegisteredStoryMetadata) {
        self.story_key = Some(metadata.key().as_str().into());
        self.story_name = Some(metadata.name().as_str().into());
        self.crate_name = Some(metadata.crate_name().into());
        self.source_file = Some(metadata.source_file().into());
        self.source_line = Some(metadata.source_line());
        self.registration_metadata = Some(metadata);
    }

    /// Returns the typed metadata copied from the inventory registry.
    pub fn registration_metadata(&self) -> Option<RegisteredStoryMetadata> {
        self.registration_metadata
    }

    /// Returns this story's typed stable key when it came from the registry.
    pub fn story_key(&self) -> Option<StoryKey> {
        self.registration_metadata.map(RegisteredStoryMetadata::key)
    }

    /// Returns this story's typed registered name when it came from the
    /// registry.
    pub fn story_name(&self) -> Option<StoryName> {
        self.registration_metadata
            .map(RegisteredStoryMetadata::name)
    }

    /// Returns this story's stable key as a string label.
    pub fn story_key_label(&self) -> Option<&str> {
        self.registration_metadata
            .map(|metadata| metadata.key().as_str())
            .or_else(|| self.story_key.as_ref().map(|story_key| story_key.as_ref()))
    }

    /// Returns this story's registered name as a string label.
    pub fn story_name_label(&self) -> Option<&str> {
        self.registration_metadata
            .map(|metadata| metadata.name().as_str())
            .or_else(|| {
                self.story_name
                    .as_ref()
                    .map(|story_name| story_name.as_ref())
            })
    }

    /// Returns the crate package name that registered this story.
    pub fn crate_name_label(&self) -> Option<&str> {
        self.registration_metadata
            .map(RegisteredStoryMetadata::crate_name)
            .or_else(|| {
                self.crate_name
                    .as_ref()
                    .map(|crate_name| crate_name.as_ref())
            })
    }

    /// Returns the source file recorded for this story.
    pub fn source_file_label(&self) -> Option<&str> {
        self.registration_metadata
            .map(RegisteredStoryMetadata::source_file)
            .or_else(|| {
                self.source_file
                    .as_ref()
                    .map(|source_file| source_file.as_ref())
            })
    }

    /// Returns the source line recorded for this story.
    pub fn source_line(&self) -> Option<u32> {
        self.registration_metadata
            .map(RegisteredStoryMetadata::source_line)
            .or(self.source_line)
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
        let scroll_handle = self.scroll_handle.clone();
        let story_key = self.story_key_label().map(str::to_owned);
        let content = div()
            .id("story-container")
            .size_full()
            .track_scroll(&scroll_handle)
            .overflow_y_scrollbar()
            .track_focus(&self.focus_handle)
            .when_some(self.story.clone(), |this, story| {
                this.child(div().size_full().p_4().child(story))
            });

        if let Some(story_key) = story_key {
            capture_story_view(story_key.to_string(), scroll_handle, content).into_any_element()
        } else {
            capture_scroll_scope(scroll_handle, content).into_any_element()
        }
    }
}
