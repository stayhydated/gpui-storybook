use gpui_kit::{
    Action, AnyElement, AnyView, App, AppContext as _, Axis, Bounds, ClickEvent, Div,
    DragMoveEvent, Empty, Entity, EntityId, EventEmitter, Focusable, Hsla, InteractiveElement as _,
    IntoElement, ParentElement, Pixels, Point, Render, RenderOnce, ScrollHandle, SharedString,
    Size, StatefulInteractiveElement as _, StyleRefinement, Styled, Window, div, hsla,
    prelude::FluentBuilder as _, px, rems, size,
};

use serde::{Deserialize, Serialize};
use std::{borrow::Borrow, rc::Rc};

type StoryRecreateFn = fn(
    &mut Window,
    &mut App,
) -> (
    AnyView,
    Option<Rc<dyn ControlTarget>>,
    gpui_kit::FocusHandle,
    Option<gpui_kit::FocusHandle>,
);

use gpui_kit::component::{
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
        capture_scroll_scope, capture_story_view, capture_substory, capture_substory_with_key,
    },
    controls::{ControlTarget, EntityControlTarget, StoryControls},
    presentation::{StoryCanvasBackground, StoryPresentation},
    registry::{RegisteredStoryMetadata, StoryKey, StoryName},
    story::StoryScenario,
};

pub const STORY_GROUP_KLASS_PREFIX: &str = "__gpui_storybook_group__:";
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
    fn render(&mut self, _: &mut Window, _: &mut gpui_kit::Context<Self>) -> impl IntoElement {
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

pub struct StoryContainer {
    focus_handle: gpui_kit::FocusHandle,
    action_scope_focus_handle: Option<gpui_kit::FocusHandle>,
    pub name: SharedString,
    pub group: Option<SharedString>,
    pub section: Option<SharedString>,
    pub title_bg: Option<Hsla>,
    pub description: SharedString,
    pub(crate) variants: Vec<Entity<StoryContainer>>,
    pub(crate) variant_group: Option<gpui_kit::WeakEntity<StoryContainer>>,
    scroll_handle: ScrollHandle,
    story_scroll_handle: ScrollHandle,
    width: Option<gpui_kit::Pixels>,
    height: Option<gpui_kit::Pixels>,
    tab_group: Option<gpui_kit::WeakEntity<TabGroup>>,
    story: Option<AnyView>,
    control_target: Option<Rc<dyn ControlTarget>>,
    presentation: StoryPresentation,
    responsive_size: Option<Size<Pixels>>,
    canvas_bounds: Option<Bounds<Pixels>>,
    canvas_stage_bounds: Option<Bounds<Pixels>>,
    canvas_resize_drag: Option<StoryCanvasResizeDrag>,
    automation_size: Option<gpui_kit::Size<gpui_kit::Pixels>>,
    workbench_state: Option<gpui_kit::WeakEntity<crate::workbench::WorkbenchState>>,
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
    scenarios: Vec<StoryScenario>,
    recreate: Option<StoryRecreateFn>,
    recreation_generation: u64,
}

mod container;
mod metadata;
mod panel;
mod render;
mod resize;
mod section;

pub use container::{ContainerEvent, Story, parse_story_group_klass};
pub use metadata::StoryState;
pub use panel::reveal_story_panel;
pub use section::{
    ShowPanelInfo, StorySection, StorySectionBase, StorySectionTitle, Substory, section,
};

#[cfg(test)]
mod tests;
