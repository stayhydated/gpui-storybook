use crate::{
    automation::{
        SharedStorybookAutomation, StoryCurrentSnapshot, StoryScreenshotRequest, StorySnapshot,
        StorybookAutomationCommand, StorybookAutomationCommandReceiver, StorybookAutomationError,
        default_storybook_automation, schedule_story_capture, set_capture_target_size,
        story_snapshots_from_containers, validate_capture_target_size,
    },
    capture_region::capture_route_story_key,
    dock_layout_store::DockLayoutStore,
    dock_sidebar_index::{SidebarStoryMetadata, group_matching_stories},
    registry::StoryEntry,
    story::{StoryContainer, StoryState, parse_story_group_klass, reveal_story_panel},
    storybook_window_ui::StorybookWindowUi,
    title_bar::{AppTitleBar, sidebar_toggle_button},
    workbench::{StoryWorkbench, WorkbenchEvent, WorkbenchState, WorkbenchTab},
};
use anyhow::{Context as _, Result};
use gpui::{
    Action, AnyElement, App, AppContext as _, ClickEvent, Context, Entity, EntityId, EventEmitter,
    FocusHandle, Focusable, InteractiveElement as _, IntoElement, ParentElement as _, Render,
    SharedString, Styled as _, Subscription, Window, div, px, relative,
};
use gpui_component::{
    ActiveTheme as _, Root, Side, Sizable as _,
    button::{Button, ButtonVariants as _},
    dock::{
        BasePanel, DockArea, DockAreaState, DockEvent, DockLayout, DockPlacement, DockSkin, Panel,
        PanelControl, PanelEvent, PanelInfo, panel_handle, register_panel,
    },
    h_flex,
    input::{Input, InputEvent, InputState},
    sidebar::{Sidebar, SidebarGroup},
    v_flex,
};
use gpui_storybook_components::{StoryDrag, StorySidebarItem};
use std::{
    collections::{BTreeMap, BTreeSet},
    rc::Rc,
    sync::{LazyLock, Mutex},
};

#[derive(Action, Clone, Debug, Default, Eq, PartialEq)]
#[action(namespace = story)]
pub struct ToggleDockToggleButton;

#[derive(Action, Clone, Debug, Default, Eq, PartialEq)]
#[action(namespace = story)]
pub struct ResetLayout;

#[derive(Action, Clone, Debug, Default, Eq, PartialEq)]
#[action(namespace = story)]
pub struct ToggleSidebar;

const MAIN_DOCK_AREA: DockAreaTab = DockAreaTab {
    id: "storybook-main-dock",
    version: 7,
};

#[cfg(debug_assertions)]
const STATE_FILE: &str = "target/storybook-docks.json";
#[cfg(not(debug_assertions))]
const STATE_FILE: &str = "storybook-docks.json";
// Unit tests use fixture-specific story registries and exercise the layout
// store through explicit temporary paths, so they must not share this file.
const PERSIST_DOCK_LAYOUT: bool = !cfg!(test);

struct DockAreaTab {
    id: &'static str,
    version: usize,
}

type StoryPanelMap = BTreeMap<String, gpui::WeakEntity<StoryContainer>>;
type StoryPanelRegistries = BTreeMap<EntityId, StoryPanelMap>;
type StorySeedRegistries = BTreeMap<EntityId, Vec<StorySeed>>;
type WorkbenchStateRegistries = BTreeMap<EntityId, gpui::WeakEntity<WorkbenchState>>;

static STORY_PANELS: LazyLock<Mutex<StoryPanelRegistries>> =
    LazyLock::new(|| Mutex::new(BTreeMap::new()));
static STORY_SEEDS: LazyLock<Mutex<StorySeedRegistries>> =
    LazyLock::new(|| Mutex::new(BTreeMap::new()));
static WORKBENCH_STATES: LazyLock<Mutex<WorkbenchStateRegistries>> =
    LazyLock::new(|| Mutex::new(BTreeMap::new()));

fn register_workbench_state(
    dock_area: &gpui::WeakEntity<DockArea>,
    state: &Entity<WorkbenchState>,
) {
    if let Ok(mut states) = WORKBENCH_STATES.lock() {
        states.insert(dock_area.entity_id(), state.downgrade());
    }
}

fn workbench_state(dock_area: &gpui::WeakEntity<DockArea>) -> Option<Entity<WorkbenchState>> {
    let mut states = WORKBENCH_STATES.lock().ok()?;
    let state = states
        .get(&dock_area.entity_id())
        .and_then(gpui::WeakEntity::upgrade);
    if state.is_none() {
        states.remove(&dock_area.entity_id());
    }
    state
}

#[derive(Clone, Debug)]
struct StorySeed {
    name: String,
    story_key: Option<String>,
    story_klass: String,
    group: Option<String>,
    section: Option<String>,
}

/// Sidebar panel for navigating stories
/// Dock workspace whose collapsible side docks leave the story canvas centered
/// within the available center pane.
pub struct StoryWorkspace {
    title_bar: Entity<AppTitleBar>,
    dock_area: Entity<DockArea>,
    dock_skin: Rc<DockSkin>,
    workbench_state: Entity<WorkbenchState>,
    automation: Option<SharedStorybookAutomation>,
    last_layout_state: Option<DockAreaState>,
    toggle_button_visible: bool,
    _app_quit_subscription: Subscription,
    _preference_subscriptions: Vec<Subscription>,
}

mod actions;
mod automation;
mod layout;
mod register;
mod render;
mod sidebar;
mod workspace;

pub use register::register_story_panels;
pub use sidebar::StorySidebar;

#[cfg(test)]
mod tests;
