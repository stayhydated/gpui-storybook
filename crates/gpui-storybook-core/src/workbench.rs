//! Window-scoped Storybook workbench state and UI.

use crate::{
    automation::{
        SharedStorybookAutomation, StoryControlsSnapshot, StoryScenarioRunSnapshot, StorySnapshot,
        StorybookAutomationError,
    },
    controls::{ControlKind, ControlSpec, ControlTarget, ControlValue},
    presentation::{StoryCanvasBackground, StoryPresentation, StoryViewportPreset},
    story::{ContainerEvent, StoryContainer, StoryScenario},
    theme_workbench::ThemeDraft,
};
use gpui_kit::component::{
    ActiveTheme as _, Disableable as _, Sizable as _,
    button::{Button, ButtonVariants as _},
    checkbox::Checkbox,
    clipboard::Clipboard,
    color_picker::{ColorPicker, ColorPickerEvent, ColorPickerState},
    dock::{BasePanel, Panel, PanelControl, PanelEvent, PanelInfo, PanelState},
    h_flex,
    input::{Input, InputEvent, InputState, NumberInput},
    link::Link,
    menu::DropdownMenu as _,
    scroll::ScrollableElement as _,
    searchable_list::SearchableListItem,
    select::{Select, SelectEvent, SelectState},
    slider::{Slider, SliderEvent, SliderState},
    tab::{Tab, TabBar},
    v_flex,
};
use gpui_kit::{
    Action, AnyElement, App, AppContext as _, ClipboardItem, Context, Entity, EntityId,
    EventEmitter, FocusHandle, Focusable, InteractiveElement as _, IntoElement, KeyBinding,
    ParentElement as _, Pixels, Render, SharedString, Size, Styled as _, Subscription, Window, div,
    prelude::FluentBuilder as _, px, size,
};
use serde::{Deserialize, Serialize};
#[cfg(not(target_family = "wasm"))]
use std::path::Path;
use std::{collections::BTreeMap, rc::Rc};

mod actions;
mod controls;
mod inspect;
mod panel;
mod performance;
mod render;
mod scenarios;
mod state;
mod theme;

#[cfg(test)]
use actions::{format_key_binding, is_story_scoped_action, story_scoped_actions};
use panel::{
    ControlEditor, ScenarioRunState, SelectControlOption, SelectViewport, StoryVariantOption,
    StoryWorkbenchPanelState, story_source_url,
};
pub(crate) use state::WorkbenchEvent;
pub use state::{WorkbenchState, WorkbenchTab};

/// Right-side developer workbench for controls, themes, inspection, actions,
/// fresh story scenarios, and opt-in performance telemetry.
pub struct StoryWorkbench {
    focus_handle: FocusHandle,
    state: Entity<WorkbenchState>,
    variant_select: Entity<SelectState<Vec<StoryVariantOption>>>,
    variant_options: Vec<StoryVariantOption>,
    selected_tab: WorkbenchTab,
    editor_story: Option<(EntityId, u64)>,
    editors: BTreeMap<String, ControlEditor>,
    editor_subscriptions: Vec<Subscription>,
    story_subscription: Option<Subscription>,
    _variant_subscription: Subscription,
    _state_subscription: Subscription,
    theme_draft: ThemeDraft,
    theme_search: Entity<InputState>,
    theme_editors: BTreeMap<String, Entity<ColorPickerState>>,
    theme_subscriptions: Vec<Subscription>,
    scenario_run: Option<ScenarioRunState>,
    last_error: Option<SharedString>,
}

#[cfg(test)]
mod tests;
