use crate::*;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::PathBuf};
use thiserror::Error;

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
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
#[serde(deny_unknown_fields)]
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
#[serde(deny_unknown_fields)]
pub struct StoryScenariosSnapshot {
    /// Story that owns the listed scenarios.
    pub story: StorySnapshot,
    /// Stable scenario descriptors in declaration order.
    pub scenarios: Vec<StoryScenarioSnapshot>,
}

/// Completed result for one story-owned interaction scenario.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StoryScenarioRunSnapshot {
    /// Scenario descriptor used to create the fresh interaction request.
    pub scenario: StoryScenarioSnapshot,
    /// Shared interaction executor result, including observations and capture.
    pub interaction: StoryInteractionSnapshot,
}

#[derive(Clone, Debug, Default, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StoryCurrentSnapshot {
    pub story: Option<StorySnapshot>,
    pub revision: u64,
}

/// Capture the story canvas or a registered substory section, excluding gallery chrome.
///
/// Backends advertise capture, capture-control mutation, sizing, and launcher
/// capabilities independently. Device providers use observed geometry; explicit
/// dimensions require sizing support before dispatch.
///
/// Gallery paired physical-pixel dimensions override the named viewport and current preview.
/// Gallery live and startup captures wait up to five seconds for the canvas to fit its
/// visible story pane, returning [`StorybookAutomationError::CaptureUnavailable`]
/// if layout cannot settle. Substory PNGs use the section's cropped dimensions.
#[derive(Clone, Debug, Default, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StoryScreenshotRequest {
    /// PNG destination, or the route-derived default when omitted.
    pub output_path: Option<PathBuf>,
    /// Requested story-canvas width in physical pixels; provide with `height`.
    pub width: Option<u32>,
    /// Requested story-canvas height in physical pixels; provide with `width`.
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
#[serde(deny_unknown_fields)]
pub struct StoryControlsSnapshot {
    pub story: StorySnapshot,
    pub controls: Vec<ControlSnapshot>,
}

/// Semantic interaction targets currently rendered by the selected route.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StoryInteractionTargetsSnapshot {
    /// Story or substory route whose rendered targets were inspected.
    pub story: StorySnapshot,
    /// Stable targets in deterministic key order.
    pub targets: Vec<StoryInteractionTargetSnapshot>,
}

/// Machine-readable values currently rendered by the selected story route.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StorySemanticValuesSnapshot {
    /// Story or substory route whose values were read.
    pub story: StorySnapshot,
    /// Stable values in deterministic key order.
    pub values: Vec<StorySemanticValueSnapshot>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StoryCaptureSnapshot {
    pub request_id: u64,
    pub path: PathBuf,
    pub pixel_width: u32,
    pub pixel_height: u32,
    pub story: StorySnapshot,
    /// Device compositor provenance when supplied by a remote capture backend.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observation: Option<wire::HostCaptureSnapshot>,
}

/// Structured live-host, validation, control, interaction, and capture errors.
#[derive(Clone, Debug, Deserialize, Eq, Error, JsonSchema, PartialEq, Serialize)]
#[serde(tag = "code", rename_all = "snake_case", deny_unknown_fields)]
pub enum StorybookAutomationError {
    #[error("automation protocol mismatch: expected {expected}, received {actual}")]
    ProtocolMismatch { expected: u32, actual: u32 },
    #[error("automation host identity changed: {message}")]
    StaleHost { message: String },
    #[error(
        "request {request_id} outcome is unknown after transport failure: {message}; rediscover state without replaying the request"
    )]
    OutcomeUnknown { request_id: u64, message: String },
    /// The negotiated host does not support this operation; no dispatch occurs.
    #[error("automation capability `{capability:?}` is unsupported")]
    UnsupportedCapability { capability: AutomationCapability },
    /// The selected automation host did not become ready within the startup deadline.
    #[error("GPUI storybook automation did not become ready within {seconds} seconds")]
    StartupTimedOut {
        /// Bounded startup wait in seconds.
        seconds: u64,
    },
    /// No live application host has attached automation execution.
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
