//! Live-window automation shared by gallery, dock, and MCP integrations.
//!
//! [`StorybookAutomation`] serializes navigation, control mutation, capture,
//! and [`StoryInteractionRequest`] batches through one exclusive operation
//! guard. Story and current-route reads, control reads, and action discovery do
//! not acquire the guard; callers may use them while a mutation is active, but
//! they can observe an intermediate rendered state.
//!
//! Interaction requests are completely validated and their keystrokes and
//! registered actions are constructed before input dispatch. The shared
//! frame-aware executor resolves fresh story or substory capture bounds after
//! route preparation and story-region sizing, constrains pointer input to those bounds,
//! honors explicit rendered-frame waits, and performs an optional capture in
//! the same operation. Runtime failures after dispatch report partial progress
//! and must not be retried automatically.
//!
//! This controller uses the application's normal platform window. On Linux,
//! the MCP integration provides a Wayland compositor with Sway's wlroots
//! headless backend. macOS capture uses GPUI's native image renderer directly.

#[cfg(feature = "capture")]
use crate::capture_output::CaptureOutputStore;
pub(crate) mod interaction;

pub use crate::capture_region::{
    StoryInteractionTargetBounds, StoryInteractionTargetSnapshot, StorySemanticValueSnapshot,
};
pub use crate::story::{StoryScenario, StoryScenarioSnapshot, StoryScenarioStep};
use crate::{
    capture_region::{
        InteractionTargetLookupError, SemanticValueLookupError, capture_region_bounds,
        capture_route_story_key, interaction_targets, scroll_capture_region_into_view,
        semantic_values,
    },
    controls::{ControlSnapshot, ControlValue},
    presentation::StoryViewportPreset,
    story::StoryContainer,
};
use gpui_kit::{App, Entity, Global, Window, px};
#[cfg(feature = "capture")]
use gpui_kit::{Bounds, Pixels, point};
pub use interaction::{
    DEFAULT_INTERACTION_POSTCONDITION_FRAMES, MAX_INTERACTION_POSTCONDITIONS,
    MAX_INTERACTION_STEPS, MAX_INTERACTION_TEXT_BYTES, MAX_INTERACTION_WAITED_FRAMES,
    StoryActionSnapshot, StoryInteractionCaptureRequest, StoryInteractionDispatch,
    StoryInteractionObservation, StoryInteractionPostcondition,
    StoryInteractionPostconditionSnapshot, StoryInteractionRequest, StoryInteractionSnapshot,
    StoryInteractionStep, StoryModifier, StoryModifiers, StoryMouseButton, StoryPoint,
    StoryPointSpace,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::{
    borrow::Borrow,
    collections::BTreeMap,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
};
use thiserror::Error;
use tokio::sync::{mpsc, oneshot, watch};

pub const DEFAULT_STORY_CAPTURE_WIDTH: u32 = 1280;
pub const DEFAULT_STORY_CAPTURE_HEIGHT: u32 = 720;

/// Shared automation handle used by live storybook views and MCP integrations.
pub type SharedStorybookAutomation = Arc<StorybookAutomation>;

/// Shared story navigation controller.
pub type SharedStoryController = SharedStorybookAutomation;

/// Shared story screenshot controller.
pub type SharedStoryCaptureController = SharedStorybookAutomation;

/// App-wide automation controller used by base storybook constructors.
///
/// When this global is installed, [`Gallery`](crate::gallery::Gallery) and
/// the dock workspace attach it from their base `view(...)` constructors.
#[derive(Clone)]
pub struct DefaultStorybookAutomation {
    automation: SharedStorybookAutomation,
}

impl Global for DefaultStorybookAutomation {}

impl DefaultStorybookAutomation {
    pub fn new(automation: SharedStorybookAutomation) -> Self {
        Self { automation }
    }

    pub fn automation(&self) -> SharedStorybookAutomation {
        self.automation.clone()
    }
}

pub fn set_default_storybook_automation(
    cx: &mut App,
    automation: SharedStorybookAutomation,
) -> SharedStorybookAutomation {
    cx.set_global(DefaultStorybookAutomation::new(automation.clone()));
    automation
}

pub fn default_storybook_automation(cx: &App) -> Option<SharedStorybookAutomation> {
    cx.try_global::<DefaultStorybookAutomation>()
        .map(DefaultStorybookAutomation::automation)
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[schemars(deny_unknown_fields)]
pub struct StoryDefaultSize {
    pub width: u32,
    pub height: u32,
}

impl Default for StoryDefaultSize {
    fn default() -> Self {
        Self {
            width: DEFAULT_STORY_CAPTURE_WIDTH,
            height: DEFAULT_STORY_CAPTURE_HEIGHT,
        }
    }
}

/// Machine-readable story metadata used by automation and capture tools.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[schemars(deny_unknown_fields)]
pub struct StorySnapshot {
    pub key: String,
    pub crate_name: String,
    pub story_name: String,
    pub title: String,
    pub description: String,
    pub group: Option<String>,
    pub section: Option<String>,
    pub source_file: String,
    pub source_line: u32,
    pub capture_route_id: String,
    pub default_size: StoryDefaultSize,
    /// Reusable interaction scenarios declared by this story.
    #[serde(default)]
    pub scenarios: Vec<StoryScenarioSnapshot>,
}

/// Scenario descriptors available for one selected story.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[schemars(deny_unknown_fields)]
pub struct StoryScenariosSnapshot {
    /// Story that owns the listed scenarios.
    pub story: StorySnapshot,
    /// Stable scenario descriptors in declaration order.
    pub scenarios: Vec<StoryScenarioSnapshot>,
}

/// Completed result for one story-owned interaction scenario.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[schemars(deny_unknown_fields)]
pub struct StoryScenarioRunSnapshot {
    /// Scenario descriptor used to create the fresh interaction request.
    pub scenario: StoryScenarioSnapshot,
    /// Shared interaction executor result, including observations and capture.
    pub interaction: StoryInteractionSnapshot,
}

#[derive(Clone, Debug, Default, Deserialize, JsonSchema, PartialEq, Serialize)]
#[schemars(deny_unknown_fields)]
pub struct StoryCurrentSnapshot {
    pub story: Option<StorySnapshot>,
    pub revision: u64,
}

#[derive(Clone, Debug, Default, Deserialize, JsonSchema, PartialEq, Serialize)]
#[schemars(deny_unknown_fields)]
pub struct StoryScreenshotRequest {
    /// PNG destination, or the route-derived default when omitted.
    pub output_path: Option<PathBuf>,
    /// Requested captured story-region width in physical pixels.
    pub width: Option<u32>,
    /// Requested captured story-region height in physical pixels.
    pub height: Option<u32>,
    /// Named viewport used when explicit dimensions are omitted.
    pub viewport: Option<StoryViewportPreset>,
    /// Serialized controls to apply to the current story before capture.
    #[serde(default)]
    pub controls: BTreeMap<String, ControlValue>,
    #[serde(default)]
    pub quit_after_capture: bool,
}

/// Current values and metadata for the controls on the selected story instance.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[schemars(deny_unknown_fields)]
pub struct StoryControlsSnapshot {
    pub story: StorySnapshot,
    pub controls: Vec<ControlSnapshot>,
}

/// Semantic interaction targets currently rendered by the selected route.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[schemars(deny_unknown_fields)]
pub struct StoryInteractionTargetsSnapshot {
    /// Story or substory route whose rendered targets were inspected.
    pub story: StorySnapshot,
    /// Stable targets in deterministic key order.
    pub targets: Vec<StoryInteractionTargetSnapshot>,
}

/// Machine-readable values currently rendered by the selected story route.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[schemars(deny_unknown_fields)]
pub struct StorySemanticValuesSnapshot {
    /// Story or substory route whose values were read.
    pub story: StorySnapshot,
    /// Stable values in deterministic key order.
    pub values: Vec<StorySemanticValueSnapshot>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[schemars(deny_unknown_fields)]
pub struct StoryCaptureSnapshot {
    pub request_id: u64,
    pub path: PathBuf,
    pub pixel_width: u32,
    pub pixel_height: u32,
    pub story: StorySnapshot,
}

/// Structured live-host, validation, control, interaction, and capture errors.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum StorybookAutomationError {
    /// The standard gallery or dock did not finish publishing and attaching its
    /// live automation host within the MCP startup deadline.
    #[error("GPUI storybook automation did not become ready within {seconds} seconds")]
    StartupTimedOut {
        /// Bounded startup wait in seconds.
        seconds: u64,
    },
    /// No gallery or dock window has attached the automation command receiver.
    #[error("no live GPUI storybook host is attached")]
    NoLiveHost,
    /// The live host disappeared while a request was awaiting completion.
    #[error(
        "live GPUI storybook host disconnected after {steps_dispatched} dispatched step(s): {message}"
    )]
    HostDisconnected {
        /// Oneshot or host failure detail.
        message: String,
        /// Interaction steps completed before disconnection.
        steps_dispatched: usize,
    },
    /// A requested stable story or substory route is unknown.
    #[error("story route `{key}` was not found")]
    StoryNotFound {
        /// Requested route.
        key: String,
    },
    /// Another navigation, control mutation, capture, or batch owns the guard.
    #[error("another storybook automation mutation is already active")]
    AutomationBusy,
    /// The active route could not be rendered or captured.
    #[error("{message}")]
    CaptureUnavailable {
        /// Capture failure detail.
        message: String,
    },
    /// Capture dimensions or viewport input is invalid.
    #[error("{message}")]
    InvalidCaptureRequest {
        /// Validation detail.
        message: String,
    },
    /// Batch-level interaction input is invalid.
    #[error("{message}")]
    InvalidInteractionRequest {
        /// Validation detail.
        message: String,
    },
    /// One indexed interaction step is invalid.
    #[error("interaction step {step_index} is invalid: {message}")]
    InvalidInteractionStep {
        /// Zero-based request step index.
        step_index: usize,
        /// Validation detail.
        message: String,
    },
    /// One indexed semantic postcondition is invalid before dispatch.
    #[error("interaction postcondition {postcondition_index} is invalid: {message}")]
    InvalidInteractionPostcondition {
        /// Zero-based postcondition index.
        postcondition_index: usize,
        /// Validation detail.
        message: String,
    },
    /// A runtime failure occurred after the batch runner started.
    #[error(
        "interaction request {request_id} failed after {steps_dispatched} dispatched step(s): {message}"
    )]
    InteractionFailed {
        /// Controller-assigned interaction request ID.
        request_id: u64,
        /// Steps completed before the runtime failure.
        steps_dispatched: usize,
        /// Runtime failure detail.
        message: String,
    },
    /// The live host has no selected story instance.
    #[error("no story is selected in the live host")]
    NoActiveStory,
    /// The selected story instance has no typed control target.
    #[error("story `{key}` does not expose controls")]
    ControlsUnavailable {
        /// Active story route.
        key: String,
    },
    /// A typed control target rejected a read or mutation.
    #[error("{message}")]
    ControlOperationFailed {
        /// Control failure detail.
        message: String,
    },
    /// The active story route has not rendered semantic target bounds.
    #[error("interaction targets are unavailable because route `{route}` is not rendered")]
    InteractionTargetsUnavailable {
        /// Active story or substory route.
        route: String,
    },
    /// A semantic target key is not present in the active route.
    #[error("interaction target `{key}` was not found in route `{route}`")]
    InteractionTargetNotFound {
        /// Active story or substory route.
        route: String,
        /// Requested stable target key.
        key: String,
    },
    /// A story rendered the same semantic target key more than once.
    #[error("interaction target `{key}` is duplicated in route `{route}`")]
    DuplicateInteractionTarget {
        /// Active story or substory route.
        route: String,
        /// Duplicated stable target key.
        key: String,
    },
    /// The active story route has not rendered semantic values.
    #[error("semantic values are unavailable because route `{route}` is not rendered")]
    SemanticValuesUnavailable {
        /// Active story or substory route.
        route: String,
    },
    /// A semantic value key is not present in the active route.
    #[error("semantic value `{key}` was not found in route `{route}`")]
    SemanticValueNotFound {
        /// Active story or substory route.
        route: String,
        /// Requested stable value key.
        key: String,
    },
    /// A semantic value did not match the requested JSON value within the
    /// bounded number of refreshed frames.
    #[error("semantic value `{key}` in route `{route}` did not match within {max_frames} frame(s)")]
    SemanticValueWaitTimedOut {
        /// Active story or substory route.
        route: String,
        /// Requested stable value key.
        key: String,
        /// Maximum refreshed frames requested by the caller.
        max_frames: u16,
    },
    /// A story rendered the same semantic value key more than once.
    #[error("semantic value `{key}` is duplicated in route `{route}`")]
    DuplicateSemanticValue {
        /// Active story or substory route.
        route: String,
        /// Duplicated stable value key.
        key: String,
    },
    /// A requested story scenario is not declared by that story.
    #[error("scenario `{scenario_key}` was not found in story `{story_key}`")]
    ScenarioNotFound {
        /// Story owning the requested scenario.
        story_key: String,
        /// Requested stable scenario key.
        scenario_key: String,
    },
    /// A story declared the same scenario key more than once.
    #[error("scenario `{scenario_key}` is duplicated in story `{story_key}`")]
    DuplicateScenarioKey {
        /// Story owning the duplicate key.
        story_key: String,
        /// Duplicated stable scenario key.
        scenario_key: String,
    },
}

mod capture;
mod controller;
mod host;

#[cfg(test)]
pub(crate) use capture::capture_exit_code;
pub use capture::{default_capture_output_path, story_snapshots_from_containers};
pub(crate) use capture::{
    ensure_capture_target_visible, render_story_capture, schedule_story_capture,
    set_capture_target_size, validate_capture_target_size,
};
pub use controller::StorybookAutomation;
pub(crate) use controller::{
    AutomationOperationGuard, StorybookAutomationCommand, StorybookAutomationCommandReceiver,
};
use host::{find_scenario, receive_host_response, resolve_story_route};
pub(crate) use host::{
    rendered_interaction_targets, rendered_semantic_values, schedule_semantic_value_read,
};

#[cfg(test)]
mod tests;
