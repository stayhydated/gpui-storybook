use gpui::{
    Action, AnyElement, AnyView, App, AppContext as _, Axis, Bounds, ClickEvent, Div,
    DragMoveEvent, Empty, Entity, EntityId, EventEmitter, Focusable, Hsla, InteractiveElement as _,
    IntoElement, ParentElement, Pixels, Point, Render, RenderOnce, ScrollHandle, SharedString,
    Size, StatefulInteractiveElement as _, StyleRefinement, Styled, Window, div, hsla,
    prelude::FluentBuilder as _, px, rems, size,
};

use serde::{Deserialize, Serialize};
use std::{borrow::Borrow, rc::Rc};

use gpui_component::{
    ActiveTheme as _, ElementExt as _, IconName, Sizable as _,
    button::{Button, ButtonVariants as _},
    dock::{
        BasePanel, Panel, PanelControl, PanelEvent, PanelId, PanelInfo, PanelState, TabGroup,
        TitleStyle, panel_handle,
    },
    group_box::{GroupBox, GroupBoxVariants as _},
    h_flex,
    menu::PopupMenu,
    scroll::{ScrollableElement as _, ScrollableMask, ScrollbarAxis},
    v_flex,
};

use super::state::AppState;
use crate::{
    capture_region::{
        capture_scroll_scope, capture_story_view, capture_story_view_with_scroll, capture_substory,
        capture_substory_with_key, current_capture_scroll_handle,
    },
    controls::{ControlTarget, EntityControlTarget, StoryControls},
    presentation::{StoryCanvasBackground, StoryPresentation},
    registry::{RegisteredStoryMetadata, StoryKey, StoryName},
};

pub const STORY_LIST_KLASS_PREFIX: &str = "__gpui_storybook_list__:";
const STORY_CANVAS_MIN_SIZE: Size<Pixels> = size(px(240.), px(160.));
const STORY_CANVAS_RESIZE_GUTTER: Pixels = px(32.);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum StoryCanvasResizeAxis {
    Horizontal,
    Vertical,
    Both,
}

#[derive(Clone, Copy)]
struct DragStoryCanvasResize {
    entity_id: EntityId,
    axis: StoryCanvasResizeAxis,
}

impl Render for DragStoryCanvasResize {
    fn render(&mut self, _: &mut Window, _: &mut gpui::Context<Self>) -> impl IntoElement {
        Empty
    }
}

#[derive(Clone, Copy)]
struct StoryCanvasResizeDrag {
    start_position: Point<Pixels>,
    start_size: Size<Pixels>,
    horizontal_scale: f32,
    vertical_scale: f32,
}

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
    story_scroll_handle: ScrollHandle,
    width: Option<gpui::Pixels>,
    height: Option<gpui::Pixels>,
    tab_group: Option<gpui::WeakEntity<TabGroup>>,
    story: Option<AnyView>,
    control_target: Option<Rc<dyn ControlTarget>>,
    presentation: StoryPresentation,
    responsive_size: Option<Size<Pixels>>,
    canvas_bounds: Option<Bounds<Pixels>>,
    canvas_stage_bounds: Option<Bounds<Pixels>>,
    canvas_resize_drag: Option<StoryCanvasResizeDrag>,
    automation_size: Option<gpui::Size<gpui::Pixels>>,
    workbench_state: Option<gpui::WeakEntity<crate::workbench::WorkbenchState>>,
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

pub trait Story: Focusable + Render + StoryControls + Sized {
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
    fn new_view(window: &mut Window, cx: &mut App) -> Entity<Self>;

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
            story_scroll_handle: ScrollHandle::new(),
            width: None,
            height: None,
            tab_group: None,
            story: None,
            control_target: None,
            presentation: StoryPresentation::default(),
            responsive_size: None,
            canvas_bounds: None,
            canvas_stage_bounds: None,
            canvas_resize_drag: None,
            automation_size: None,
            workbench_state: None,
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
        let control_target = EntityControlTarget::optional(story.clone(), cx);
        let story_klass = S::klass();
        let focus_handle = story.focus_handle(cx);

        cx.new(|cx| {
            let mut story = Self::new(window, cx)
                .story(story.into(), story_klass)
                .on_active(S::on_active_any);
            story.control_target = control_target;
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

    /// Returns the controls for this concrete story instance.
    pub fn control_target(&self) -> Option<Rc<dyn ControlTarget>> {
        self.control_target.clone()
    }

    pub(crate) fn set_presentation(&mut self, presentation: StoryPresentation) {
        self.presentation = presentation;
    }

    pub(crate) fn set_responsive_size(&mut self, responsive_size: Option<Size<Pixels>>) {
        self.responsive_size = responsive_size;
    }

    pub(crate) fn set_automation_size(&mut self, size: Option<gpui::Size<gpui::Pixels>>) {
        self.automation_size = size;
    }

    pub fn presentation(&self) -> StoryPresentation {
        self.presentation
    }

    pub(crate) fn set_workbench_state(
        &mut self,
        state: gpui::WeakEntity<crate::workbench::WorkbenchState>,
    ) {
        self.workbench_state = Some(state);
    }

    fn viewport_size(&self) -> Option<Size<Pixels>> {
        self.presentation
            .viewport
            .dimensions()
            .map(|(width, height)| size(px(width as f32), px(height as f32)))
            .or(self.responsive_size)
    }

    fn begin_canvas_resize(&mut self, start_position: Point<Pixels>) {
        let (Some(canvas_bounds), Some(stage_bounds)) =
            (self.canvas_bounds, self.canvas_stage_bounds)
        else {
            return;
        };
        let gutter = STORY_CANVAS_RESIZE_GUTTER * 2.;
        let available_stage_size = size(
            (stage_bounds.size.width - gutter).max(px(0.)),
            (stage_bounds.size.height - gutter).max(px(0.)),
        );
        self.canvas_resize_drag = Some(StoryCanvasResizeDrag {
            start_position,
            start_size: canvas_bounds.size,
            horizontal_scale: if canvas_bounds.size.width < available_stage_size.width {
                2.
            } else {
                1.
            },
            vertical_scale: if canvas_bounds.size.height < available_stage_size.height {
                2.
            } else {
                1.
            },
        });
    }

    fn resize_canvas(
        &mut self,
        axis: StoryCanvasResizeAxis,
        position: Point<Pixels>,
        cx: &mut gpui::Context<Self>,
    ) {
        if self.presentation.viewport != crate::presentation::StoryViewportPreset::Responsive {
            return;
        }
        let Some(drag) = self.canvas_resize_drag else {
            return;
        };
        let delta = position - drag.start_position;
        let width = match axis {
            StoryCanvasResizeAxis::Horizontal | StoryCanvasResizeAxis::Both => {
                (drag.start_size.width + delta.x * drag.horizontal_scale)
                    .max(STORY_CANVAS_MIN_SIZE.width)
            },
            StoryCanvasResizeAxis::Vertical => drag.start_size.width,
        };
        let height = match axis {
            StoryCanvasResizeAxis::Vertical | StoryCanvasResizeAxis::Both => {
                (drag.start_size.height + delta.y * drag.vertical_scale)
                    .max(STORY_CANVAS_MIN_SIZE.height)
            },
            StoryCanvasResizeAxis::Horizontal => drag.start_size.height,
        };
        let responsive_size = size(width, height);
        self.responsive_size = Some(responsive_size);
        if let Some(workbench_state) = self
            .workbench_state
            .as_ref()
            .and_then(gpui::WeakEntity::upgrade)
        {
            workbench_state.update(cx, |state, cx| {
                state.set_responsive_size(responsive_size, cx);
            });
        }
        cx.notify();
    }

    fn render_canvas_resize_handle(
        &self,
        axis: StoryCanvasResizeAxis,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        let entity_id = cx.entity_id();
        let story_for_mouse_down = cx.entity();
        let (id, selector) = match axis {
            StoryCanvasResizeAxis::Horizontal => {
                ("story-canvas-resize-width", "story-canvas-resize-width")
            },
            StoryCanvasResizeAxis::Vertical => {
                ("story-canvas-resize-height", "story-canvas-resize-height")
            },
            StoryCanvasResizeAxis::Both => {
                ("story-canvas-resize-corner", "story-canvas-resize-corner")
            },
        };

        div()
            .id(id)
            .absolute()
            .debug_selector(move || selector.to_owned())
            .map(|this| match axis {
                StoryCanvasResizeAxis::Horizontal => this
                    .top_0()
                    .right(px(-4.))
                    .h_full()
                    .w(px(9.))
                    .cursor_ew_resize(),
                StoryCanvasResizeAxis::Vertical => this
                    .left_0()
                    .bottom(px(-4.))
                    .w_full()
                    .h(px(9.))
                    .cursor_ns_resize(),
                StoryCanvasResizeAxis::Both => this
                    .right(px(-5.))
                    .bottom(px(-5.))
                    .size(px(12.))
                    .cursor_nwse_resize(),
            })
            .on_mouse_down(gpui::MouseButton::Left, move |event, _, cx| {
                cx.stop_propagation();
                story_for_mouse_down.update(cx, |story, _| {
                    story.begin_canvas_resize(event.position);
                });
            })
            .on_drag(
                DragStoryCanvasResize { entity_id, axis },
                move |drag, _, _, cx| {
                    cx.stop_propagation();
                    cx.new(|_| *drag)
                },
            )
            .on_drag_move(cx.listener(
                move |this, event: &DragMoveEvent<DragStoryCanvasResize>, _, cx| {
                    let drag = event.drag(cx);
                    if drag.entity_id == entity_id && drag.axis == axis {
                        this.resize_canvas(axis, event.event.position, cx);
                    }
                },
            ))
    }

    /// Store typed registry metadata on this runtime container.
    ///
    /// This also populates the string metadata fields exposed by this
    /// container.
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

impl BasePanel for StoryContainer {
    fn panel_name(&self) -> &'static str {
        "StoryContainer"
    }

    fn closable(&self, _cx: &App) -> bool {
        self.closable
    }

    fn zoomable(&self, _cx: &App) -> bool {
        self.zoomable.is_some()
    }

    fn visible(&self, cx: &App) -> bool {
        !AppState::global(cx)
            .invisible_panels
            .read(cx)
            .contains(&self.name)
    }

    fn set_zoomed(&mut self, zoomed: bool, _window: &mut Window, _cx: &mut gpui::Context<Self>) {
        tracing::debug!(panel = %self.name, zoomed, "Storybook panel zoom changed");
    }

    fn set_active(&mut self, active: bool, window: &mut Window, cx: &mut gpui::Context<Self>) {
        tracing::debug!(panel = %self.name, active, "Storybook panel activation changed");
        self.is_active = active;
        if active
            && let Some(state) = self
                .workbench_state
                .as_ref()
                .and_then(gpui::WeakEntity::upgrade)
        {
            let story = cx.entity();
            // Panel activation updates this entity while the dock group is
            // synchronizing its selection. Defer the workbench update so it
            // can inspect the story after the current entity lease is released.
            window.defer(cx, move |_, cx| {
                state.update(cx, |state, cx| state.set_active_story(Some(story), cx));
            });
        }
        if let Some(on_active) = self.on_active
            && let Some(story) = self.story.clone()
        {
            on_active(story, active, window, cx);
        }
    }

    fn on_added_to(
        &mut self,
        tab_group: gpui::WeakEntity<TabGroup>,
        _window: &mut Window,
        _cx: &mut gpui::Context<Self>,
    ) {
        self.tab_group = Some(tab_group);
    }

    fn on_removed(&mut self, _window: &mut Window, _cx: &mut gpui::Context<Self>) {
        self.tab_group = None;
        self.is_active = false;
    }

    fn dump(&self, _cx: &App) -> PanelState {
        let mut state = PanelState::new(self.panel_name());
        if let Some(story_klass) = self.story_klass.clone() {
            let story_state = StoryState { story_klass };
            state.info = PanelInfo::panel(story_state.to_value());
        }
        state
    }
}

impl Panel for StoryContainer {
    fn title(&mut self, _window: &mut Window, _cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let tab_group = self.tab_group.clone();
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
                        move |_: &ClickEvent, _window: &mut Window, cx: &mut App| {
                            cx.stop_propagation();
                            let Some(tab_group) = tab_group.clone().and_then(|tab| tab.upgrade())
                            else {
                                return;
                            };
                            let Some(story_panel) = story_panel.upgrade() else {
                                return;
                            };
                            tab_group.update(cx, |tab_group, cx| {
                                tab_group.close_panel(PanelId::from(story_panel.entity_id()), cx);
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

    fn zoom_control(&self, _cx: &App) -> Option<PanelControl> {
        self.zoomable
    }

    fn dropdown_menu(
        &mut self,
        menu: PopupMenu,
        _window: &mut Window,
        _cx: &mut gpui::Context<Self>,
    ) -> PopupMenu {
        menu.menu("Info", Box::new(ShowPanelInfo))
    }
}

pub fn reveal_story_panel(
    story: &Entity<StoryContainer>,
    window: &mut Window,
    cx: &mut App,
) -> bool {
    let (is_active, tab_group) = {
        let story = story.read(cx);
        (story.is_active, story.tab_group.clone())
    };

    if is_active {
        return true;
    }

    let Some(tab_group) = tab_group.and_then(|tab| tab.upgrade()) else {
        return false;
    };

    let panel = panel_handle(story.clone());
    tab_group.update(cx, |tab_group, cx| {
        let Some(ix) = tab_group
            .panels()
            .iter()
            .position(|candidate| candidate.panel_id(cx) == panel.panel_id(cx))
        else {
            return;
        };
        tab_group.select_tab(ix, window, cx);
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
    fn render(&mut self, _: &mut Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let canvas_scroll_handle = self.scroll_handle.clone();
        let story_scroll_handle = self.story_scroll_handle.clone();
        let story_key = self.story_key_label().map(str::to_owned);
        let presentation = self.presentation;
        let is_responsive =
            presentation.viewport == crate::presentation::StoryViewportPreset::Responsive;
        let viewport_size = self.viewport_size();
        let automation_size = self.automation_size;
        let background = match presentation.background {
            StoryCanvasBackground::Theme => cx.theme().background,
            StoryCanvasBackground::Light => hsla(0.0, 0.0, 0.98, 1.0),
            StoryCanvasBackground::Dark => hsla(0.0, 0.0, 0.08, 1.0),
            StoryCanvasBackground::Transparent => hsla(0.0, 0.0, 0.0, 0.0),
        };
        let border_color = cx.theme().border;
        let frame = || {
            div()
                .absolute()
                .top_0()
                .left_0()
                .size_full()
                .border_1()
                .border_color(border_color)
                .debug_selector(|| "story-canvas-border".to_owned())
        };
        let resize_handles = if is_responsive {
            vec![
                self.render_canvas_resize_handle(StoryCanvasResizeAxis::Horizontal, cx)
                    .into_any_element(),
                self.render_canvas_resize_handle(StoryCanvasResizeAxis::Vertical, cx)
                    .into_any_element(),
                self.render_canvas_resize_handle(StoryCanvasResizeAxis::Both, cx)
                    .into_any_element(),
            ]
        } else {
            Vec::new()
        };
        let story_for_canvas_bounds = cx.entity();
        let canvas = div()
            .relative()
            .flex_none()
            .bg(background)
            .debug_selector(|| "story-canvas".to_owned())
            .map(|this| match viewport_size {
                Some(size) => this.w(size.width).h(size.height),
                None => this.size_full(),
            })
            .when_some(self.story.clone(), |this, story| {
                this.child(
                    div()
                        .relative()
                        .size_full()
                        .child(
                            div()
                                .id("story-content-scroll-region")
                                .debug_selector(|| "story-content-scroll-region".to_owned())
                                .size_full()
                                .overflow_hidden()
                                .track_scroll(&story_scroll_handle)
                                .child(
                                    div()
                                        .flex_none()
                                        .w_auto()
                                        .h_auto()
                                        .min_w_full()
                                        .min_h_full()
                                        .p_4()
                                        .child(story),
                                ),
                        )
                        .child(
                            ScrollableMask::new(Axis::Vertical, &story_scroll_handle)
                                .id("story-content-scroll-region"),
                        )
                        .child(
                            ScrollableMask::new(Axis::Horizontal, &story_scroll_handle)
                                .id("story-content-scroll-region"),
                        )
                        .scrollbar(&story_scroll_handle, ScrollbarAxis::Both),
                )
            })
            .child(frame())
            .children(resize_handles)
            .on_prepaint(move |bounds, _, cx| {
                story_for_canvas_bounds.update(cx, |story, _| {
                    story.canvas_bounds = Some(bounds);
                });
            });
        let story_for_stage_bounds = cx.entity();
        let canvas_stage = div()
            .relative()
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .debug_selector(|| "story-canvas-stage".to_owned())
            .when(is_responsive, |this| this.p(STORY_CANVAS_RESIZE_GUTTER))
            .map(|this| match viewport_size {
                Some(size) if is_responsive => this
                    .min_w_full()
                    .min_h_full()
                    .w(size.width + STORY_CANVAS_RESIZE_GUTTER * 2.)
                    .h(size.height + STORY_CANVAS_RESIZE_GUTTER * 2.),
                Some(size) => this.min_w_full().min_h_full().w(size.width).h(size.height),
                None => this.size_full(),
            })
            .child(canvas)
            .on_prepaint(move |bounds, _, cx| {
                story_for_stage_bounds.update(cx, |story, _| {
                    story.canvas_stage_bounds = Some(bounds);
                });
            });
        let content = v_flex()
            .id("story-container")
            .debug_selector(|| "story-container-scroll-region".to_owned())
            .when(automation_size.is_none(), |this| this.size_full())
            .when_some(automation_size, |this, size| {
                this.flex_none().w(size.width).h(size.height)
            })
            .track_scroll(&canvas_scroll_handle)
            .overflow_scroll()
            .restrict_scroll_to_axis()
            .track_focus(&self.focus_handle)
            .child(canvas_stage)
            .scrollbar(&canvas_scroll_handle, ScrollbarAxis::Both);
        #[cfg(feature = "inspector")]
        let content = crate::story_inspector::inspectable_story(
            crate::story_inspector::StoryInspectorState::from_container(self, cx),
            content,
        );

        if let Some(story_key) = story_key {
            capture_story_view(story_key, story_scroll_handle, content).into_any_element()
        } else {
            capture_scroll_scope(story_scroll_handle, content).into_any_element()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::{StoryKey, StoryName, StorySectionName};
    use crate::workbench::WorkbenchState;
    use gpui::{
        Modifiers, MouseButton, ScrollDelta, ScrollWheelEvent, TestAppContext, VisualTestContext,
        div, point, px,
    };

    enum DemoSubstory {
        WithIcon,
    }

    impl Substory for DemoSubstory {
        fn capture_key(&self) -> &'static str {
            "with-icon"
        }

        fn title(&self) -> SharedString {
            "With Icon".into()
        }
    }

    struct DefaultStoryContract;

    struct TallStoryContent;

    impl StoryControls for DefaultStoryContract {}

    impl Focusable for DefaultStoryContract {
        fn focus_handle(&self, _: &App) -> gpui::FocusHandle {
            unreachable!("the static Story defaults do not require a focus handle")
        }
    }

    impl Render for DefaultStoryContract {
        fn render(&mut self, _: &mut Window, _: &mut gpui::Context<Self>) -> impl IntoElement {
            div()
        }
    }

    impl Story for DefaultStoryContract {
        fn title(_: &App) -> String {
            "Default Story".to_string()
        }

        fn new_view(_: &mut Window, cx: &mut App) -> Entity<Self> {
            cx.new(|_| DefaultStoryContract)
        }
    }

    impl Render for TallStoryContent {
        fn render(&mut self, _: &mut Window, _: &mut gpui::Context<Self>) -> impl IntoElement {
            v_flex().w_full().child(div().h(px(1200.))).child(
                div()
                    .debug_selector(|| "tall-story-bottom".to_owned())
                    .h(px(32.)),
            )
        }
    }

    #[test]
    fn section_titles_support_visible_and_stable_capture_identity() {
        let (title, key) = StorySectionTitle::new("Visible").into_parts();
        assert_eq!(title.as_ref(), "Visible");
        assert_eq!(key, None);

        let (title, key) = StorySectionTitle::with_capture_key("stable", "Visible").into_parts();
        assert_eq!(title.as_ref(), "Visible");
        assert_eq!(key.as_deref(), Some("stable"));

        for descriptor in [
            StorySectionTitle::from("Borrowed"),
            StorySectionTitle::from(String::from("Owned")),
            StorySectionTitle::from(SharedString::from("Shared")),
        ] {
            assert_eq!(descriptor.capture_key, None);
        }

        let descriptor = StorySectionTitle::from(DemoSubstory::WithIcon);
        let base = StorySectionBase::new(descriptor);
        assert_eq!(base.title().as_ref(), "With Icon");
        assert_eq!(base.capture_key().map(AsRef::as_ref), Some("with-icon"));
        let _captured = base.capture(div());

        let visible = StorySectionBase::new("Visible");
        assert_eq!(visible.title().as_ref(), "Visible");
        assert_eq!(visible.capture_key(), None);
        let _captured = visible.capture(div());
    }

    #[test]
    fn styled_section_builder_collects_subtitles_children_and_widths() {
        let mut section = section("Examples")
            .sub_title(div().child("Subtitle"))
            .max_w_md()
            .max_w_lg()
            .max_w_xl()
            .max_w_2xl();
        section.extend([div().child("First").into_any_element()]);
        let _style = section.style();

        assert_eq!(section.capture.title().as_ref(), "Examples");
        assert_eq!(section.sub_title.len(), 1);
        assert_eq!(section.children.len(), 1);
    }

    #[cfg(feature = "dock")]
    #[test]
    fn story_list_class_round_trips_sorted_members() {
        assert_eq!(parse_story_list_klass("ButtonStory"), None);
        assert_eq!(
            parse_story_list_klass(STORY_LIST_KLASS_PREFIX),
            Some(Vec::new())
        );
        assert_eq!(
            parse_story_list_klass("__gpui_storybook_list__:TableStory||ButtonStory"),
            Some(vec!["TableStory".to_string(), "ButtonStory".to_string()])
        );
    }

    #[gpui::test]
    fn story_trait_defaults_are_stable(cx: &mut App) {
        assert_eq!(DefaultStoryContract::klass(), "DefaultStoryContract");
        assert_eq!(DefaultStoryContract::title(cx), "Default Story");
        assert_eq!(DefaultStoryContract::description(cx), "");
        assert!(DefaultStoryContract::closable());
        assert!(DefaultStoryContract::zoomable().is_some());
        assert_eq!(DefaultStoryContract::title_bg(), None);
    }

    #[gpui::test]
    fn story_container_builders_and_metadata_expose_runtime_contract(cx: &mut App) {
        gpui_component::init(cx);
        let window: gpui::WindowHandle<StoryContainer> = cx
            .open_window(Default::default(), |window, cx| {
                cx.new(|cx| StoryContainer::new(window, cx))
            })
            .expect("test window should open");

        window
            .update(cx, |container, window, cx| {
                assert_eq!(container.sidebar_group(), None);
                assert_eq!(container.sidebar_section(), None);
                assert_eq!(container.display_title(cx), "");
                assert_eq!(container.display_description(cx), "");

                *container = StoryContainer::new(window, cx)
                    .group("Examples")
                    .section("Components")
                    .width(px(800.))
                    .height(px(600.));
                container.name = "Button".into();
                container.description = "Button states".into();
                assert_eq!(container.sidebar_group().as_deref(), Some("Examples"));
                assert_eq!(container.sidebar_section().as_deref(), Some("Components"));
                assert_eq!(container.display_title(cx), "Button");
                assert_eq!(container.display_description(cx), "Button states");
                assert_eq!(container.width, Some(px(800.)));
                assert_eq!(container.height, Some(px(600.)));

                let metadata = RegisteredStoryMetadata::new(
                    StoryKey::new("crate-ButtonStory"),
                    StoryName::new("ButtonStory"),
                    Some(StorySectionName::new("Components")),
                    "crate",
                    "/tmp/crate",
                    "src/button.rs",
                    42,
                );
                container.set_registration_metadata(metadata);
                assert_eq!(container.registration_metadata(), Some(metadata));
                assert_eq!(
                    container.story_key(),
                    Some(StoryKey::new("crate-ButtonStory"))
                );
                assert_eq!(container.story_name(), Some(StoryName::new("ButtonStory")));
                assert_eq!(container.story_key_label(), Some("crate-ButtonStory"));
                assert_eq!(container.story_name_label(), Some("ButtonStory"));
                assert_eq!(container.crate_name_label(), Some("crate"));
                assert_eq!(container.source_file_label(), Some("src/button.rs"));
                assert_eq!(container.source_line(), Some(42));

                container.title_fn = Some(Box::new(|_| "Localized Button".to_string()));
                container.description_fn = Some(Box::new(|_| "Localized description".to_string()));
                assert_eq!(container.display_title(cx), "Localized Button");
                assert_eq!(container.display_description(cx), "Localized description");
            })
            .expect("container should update");
    }

    #[test]
    fn tall_stories_scroll_inside_canvas_without_panning_viewport_frame() {
        let mut app = TestAppContext::single();
        app.update(gpui_component::init);
        let (container, cx) = app.add_window_view(|window, cx| -> StoryContainer {
            let story = cx.new(|_| TallStoryContent);
            let mut container =
                StoryContainer::new(window, cx).story(story.into(), "TallStoryContent");
            container.set_presentation(StoryPresentation {
                viewport: crate::presentation::StoryViewportPreset::Responsive,
                background: StoryCanvasBackground::Theme,
            });
            container
        });

        let draw = |cx: &mut VisualTestContext| {
            cx.run_until_parked();
            cx.update(|window, cx| {
                _ = window.draw(cx);
            });
        };

        draw(cx);
        let story_viewport = cx
            .debug_bounds("story-content-scroll-region")
            .expect("story content viewport should render");
        let bottom_before = cx
            .debug_bounds("tall-story-bottom")
            .expect("tall story content should render beyond the viewport");
        cx.read(|cx| {
            let container = container.read(cx);
            assert!(container.story_scroll_handle.max_offset().y > px(0.));
            assert_eq!(container.scroll_handle.max_offset(), point(px(0.), px(0.)));
        });

        cx.simulate_event(ScrollWheelEvent {
            position: story_viewport.center(),
            delta: ScrollDelta::Pixels(point(px(0.), px(-120.))),
            ..Default::default()
        });
        draw(cx);

        let bottom_after = cx
            .debug_bounds("tall-story-bottom")
            .expect("tall story content should remain rendered after scrolling");
        assert_eq!(bottom_after.origin.y, bottom_before.origin.y - px(120.));
        cx.read(|cx| {
            let container = container.read(cx);
            assert_eq!(
                container.story_scroll_handle.offset(),
                point(px(0.), px(-120.))
            );
            assert_eq!(container.scroll_handle.offset(), point(px(0.), px(0.)));
        });
    }

    #[test]
    fn viewport_presets_size_and_center_the_canvas() {
        let mut app = TestAppContext::single();
        app.update(gpui_component::init);
        let (container, cx) = app.add_window_view(|window, cx| -> StoryContainer {
            let mut container = StoryContainer::new(window, cx);
            container.set_presentation(StoryPresentation {
                viewport: crate::presentation::StoryViewportPreset::Responsive,
                background: StoryCanvasBackground::Theme,
            });
            container
        });

        let draw = |cx: &mut VisualTestContext| {
            cx.run_until_parked();
            cx.update(|window, cx| {
                _ = window.draw(cx);
            });
        };

        draw(cx);
        let initial_canvas_bounds = cx
            .debug_bounds("story-canvas")
            .expect("responsive canvas should render");
        let initial_stage_bounds = cx
            .debug_bounds("story-canvas-stage")
            .expect("responsive canvas stage should render");
        let scroll_region_bounds = cx
            .debug_bounds("story-container-scroll-region")
            .expect("story scroll region should render");
        assert_eq!(initial_stage_bounds, scroll_region_bounds);
        assert_eq!(
            initial_canvas_bounds.size,
            gpui::size(
                initial_stage_bounds.size.width - STORY_CANVAS_RESIZE_GUTTER * 2.,
                initial_stage_bounds.size.height - STORY_CANVAS_RESIZE_GUTTER * 2.
            )
        );
        assert_eq!(
            initial_canvas_bounds.center(),
            initial_stage_bounds.center()
        );
        assert_eq!(
            cx.debug_bounds("story-canvas-border"),
            cx.debug_bounds("story-canvas")
        );
        assert!(cx.debug_bounds("story-canvas-resize-width").is_some());
        assert!(cx.debug_bounds("story-canvas-resize-height").is_some());
        assert!(cx.debug_bounds("story-canvas-resize-corner").is_some());

        for (viewport, expected_size) in [
            (
                crate::presentation::StoryViewportPreset::Mobile,
                gpui::size(px(390.), px(844.)),
            ),
            (
                crate::presentation::StoryViewportPreset::Tablet,
                gpui::size(px(768.), px(1024.)),
            ),
            (
                crate::presentation::StoryViewportPreset::Desktop,
                gpui::size(px(1440.), px(900.)),
            ),
        ] {
            container.update(cx, |container, cx| {
                container.set_presentation(StoryPresentation {
                    viewport,
                    background: StoryCanvasBackground::Theme,
                });
                cx.notify();
            });
            draw(cx);

            let canvas_bounds = cx
                .debug_bounds("story-canvas")
                .expect("fixed canvas should render");
            let stage_bounds = cx
                .debug_bounds("story-canvas-stage")
                .expect("centered canvas stage should render");
            assert_eq!(canvas_bounds.size, expected_size);
            assert_eq!(canvas_bounds.center(), stage_bounds.center());
            assert_eq!(
                cx.debug_bounds("story-canvas-border")
                    .expect("canvas border should render"),
                canvas_bounds
            );
            assert_eq!(cx.debug_bounds("story-canvas-resize-corner"), None);
        }

        let workbench = cx.new(|_| WorkbenchState::new(Some(container.clone())));
        workbench.update(cx, |state, cx| {
            state.set_viewport(crate::presentation::StoryViewportPreset::Mobile, cx);
            state.set_viewport(crate::presentation::StoryViewportPreset::Responsive, cx);
        });
        draw(cx);

        let canvas_bounds = cx
            .debug_bounds("story-canvas")
            .expect("inherited responsive canvas should render");
        let stage_bounds = cx
            .debug_bounds("story-canvas-stage")
            .expect("responsive canvas stage should render");
        assert_eq!(canvas_bounds.size, gpui::size(px(390.), px(844.)));
        assert_eq!(canvas_bounds.center(), stage_bounds.center());
        assert!(cx.debug_bounds("story-canvas-resize-corner").is_some());

        let resize_start = cx
            .debug_bounds("story-canvas-resize-corner")
            .expect("responsive corner resize handle should render")
            .center();
        cx.simulate_mouse_move(resize_start, None, Modifiers::none());
        cx.simulate_mouse_down(resize_start, MouseButton::Left, Modifiers::none());
        cx.simulate_mouse_move(
            point(resize_start.x + px(5.), resize_start.y + px(5.)),
            MouseButton::Left,
            Modifiers::none(),
        );
        cx.simulate_mouse_move(
            point(resize_start.x + px(30.), resize_start.y + px(25.)),
            MouseButton::Left,
            Modifiers::none(),
        );
        draw(cx);
        let first_resized_canvas = cx
            .debug_bounds("story-canvas")
            .expect("responsive canvas should resize during the drag");
        assert_eq!(
            first_resized_canvas.size,
            gpui::size(
                canvas_bounds.size.width + px(60.),
                canvas_bounds.size.height + px(50.)
            )
        );

        cx.simulate_mouse_move(
            point(resize_start.x + px(50.), resize_start.y + px(40.)),
            MouseButton::Left,
            Modifiers::none(),
        );
        draw(cx);
        let second_resized_canvas = cx
            .debug_bounds("story-canvas")
            .expect("responsive canvas should keep resizing during the drag");
        assert_eq!(
            second_resized_canvas.size,
            gpui::size(
                canvas_bounds.size.width + px(100.),
                canvas_bounds.size.height + px(80.)
            )
        );

        cx.simulate_mouse_up(
            point(resize_start.x + px(50.), resize_start.y + px(40.)),
            MouseButton::Left,
            Modifiers::none(),
        );
        draw(cx);
        let resized_canvas = cx
            .debug_bounds("story-canvas")
            .expect("resized responsive canvas should render");
        assert_eq!(resized_canvas.size, second_resized_canvas.size);
        assert_eq!(
            cx.read(|cx| workbench.read(cx).responsive_size()),
            Some(resized_canvas.size)
        );

        container.update(cx, |container, cx| {
            container.set_responsive_size(Some(gpui::size(px(2000.), px(1500.))));
            cx.notify();
        });
        draw(cx);
        let oversized_canvas = cx
            .debug_bounds("story-canvas")
            .expect("oversized responsive canvas should render");
        let oversized_stage = cx
            .debug_bounds("story-canvas-stage")
            .expect("oversized responsive canvas stage should render");
        assert_eq!(
            oversized_canvas.origin.x - oversized_stage.origin.x,
            STORY_CANVAS_RESIZE_GUTTER
        );
        assert_eq!(
            oversized_canvas.origin.y - oversized_stage.origin.y,
            STORY_CANVAS_RESIZE_GUTTER
        );
        assert_eq!(
            oversized_stage.right() - oversized_canvas.right(),
            STORY_CANVAS_RESIZE_GUTTER
        );
        assert_eq!(
            oversized_stage.bottom() - oversized_canvas.bottom(),
            STORY_CANVAS_RESIZE_GUTTER
        );
        for selector in [
            "story-canvas-resize-width",
            "story-canvas-resize-height",
            "story-canvas-resize-corner",
        ] {
            let handle = cx
                .debug_bounds(selector)
                .unwrap_or_else(|| panic!("{selector} should render"));
            assert!(
                handle.left() >= oversized_stage.left()
                    && handle.top() >= oversized_stage.top()
                    && handle.right() <= oversized_stage.right()
                    && handle.bottom() <= oversized_stage.bottom(),
                "resize handle {handle:?} must remain inside stage {oversized_stage:?}"
            );
        }

        let scroll_region =
            cx.update(|window, _| gpui::Bounds::new(point(px(0.), px(0.)), window.viewport_size()));
        cx.simulate_event(ScrollWheelEvent {
            position: scroll_region.center(),
            delta: ScrollDelta::Pixels(point(px(-5000.), px(0.))),
            ..Default::default()
        });
        cx.simulate_event(ScrollWheelEvent {
            position: scroll_region.center(),
            delta: ScrollDelta::Pixels(point(px(0.), px(-5000.))),
            ..Default::default()
        });
        draw(cx);
        let scrolled_canvas = cx
            .debug_bounds("story-canvas")
            .expect("scrolled responsive canvas should render");
        let scrolled_stage = cx
            .debug_bounds("story-canvas-stage")
            .expect("scrolled responsive stage should render");
        assert_eq!(
            scroll_region.right() - scrolled_canvas.right(),
            STORY_CANVAS_RESIZE_GUTTER,
            "scroll region {scroll_region:?}, canvas {scrolled_canvas:?}, stage {scrolled_stage:?}"
        );
        assert_eq!(
            scroll_region.bottom() - scrolled_canvas.bottom(),
            STORY_CANVAS_RESIZE_GUTTER,
            "scroll viewport {scroll_region:?}, canvas {scrolled_canvas:?}, stage {scrolled_stage:?}"
        );
    }

    #[gpui::test]
    fn panel_activation_defers_workbench_story_reads(cx: &mut App) {
        gpui_component::init(cx);
        let state = cx.new(|_| WorkbenchState::new(None));
        let state_for_window = state.clone();
        let window: gpui::WindowHandle<StoryContainer> = cx
            .open_window(Default::default(), move |window, cx| {
                cx.new(|cx| {
                    let mut story = StoryContainer::new(window, cx);
                    story.set_workbench_state(state_for_window.downgrade());
                    story
                })
            })
            .expect("test window should open");

        window
            .update(cx, |story, window, cx| {
                BasePanel::set_active(story, true, window, cx);
                assert!(state.read(cx).active_story().is_none());
            })
            .expect("panel activation should not reenter the story entity");
    }

    #[gpui::test]
    fn story_list_class_sorts_entities_and_ignores_missing_classes(cx: &mut App) {
        gpui_component::init(cx);
        let window: gpui::WindowHandle<StoryContainer> = cx
            .open_window(Default::default(), |window, cx| {
                cx.new(|cx| StoryContainer::new(window, cx))
            })
            .expect("test window should open");

        window
            .update(cx, |_, window, cx| {
                let table = cx.new(|cx| {
                    let mut story = StoryContainer::new(window, cx);
                    story.story_klass = Some("TableStory".into());
                    story
                });
                let button = cx.new(|cx| {
                    let mut story = StoryContainer::new(window, cx);
                    story.story_klass = Some("ButtonStory".into());
                    story
                });
                let missing = cx.new(|cx| StoryContainer::new(window, cx));

                assert_eq!(
                    story_list_klass(&[table, missing, button], cx).as_ref(),
                    "__gpui_storybook_list__:ButtonStory|TableStory"
                );
            })
            .expect("story classes should be computed");
    }

    #[test]
    fn story_state_serializes_panel_identity() {
        let state = StoryState {
            story_klass: "ButtonStory".into(),
        };
        assert_eq!(
            state.to_value(),
            serde_json::json!({ "story_klass": "ButtonStory" })
        );
    }
}
