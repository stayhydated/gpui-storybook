use super::{
    AutomationOperationGuard, StoryCaptureSnapshot, StoryInteractionTargetsSnapshot,
    StoryScreenshotRequest, StorySemanticValueSnapshot, StorySnapshot, StorybookAutomationError,
    ensure_capture_target_visible, render_story_capture, rendered_interaction_targets,
    rendered_semantic_values, validate_capture_target_size,
};
use crate::{
    capture_region::{capture_region_bounds, scroll_capture_region_into_view},
    controls::ControlValue,
    presentation::StoryViewportPreset,
};
use gpui::{
    Action, App, Keystroke, Modifiers, MouseDownEvent, MouseMoveEvent, MouseUpEvent, PlatformInput,
    ScrollDelta, ScrollWheelEvent, TouchPhase, Window, point, px,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::{BTreeMap, VecDeque},
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};
use tokio::sync::oneshot;

/// Maximum number of steps accepted by one interaction batch.
pub const MAX_INTERACTION_STEPS: usize = 64;
/// Maximum UTF-8 byte length accepted across text values and keystroke strings.
pub const MAX_INTERACTION_TEXT_BYTES: usize = 4 * 1024;
/// Maximum number of rendered frames one batch may explicitly wait for.
pub const MAX_INTERACTION_WAITED_FRAMES: u16 = 120;

/// Runtime-visible GPUI action metadata.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StoryActionSnapshot {
    /// Runtime action name accepted by a `dispatch_action` step.
    pub name: String,
    /// Documentation registered with GPUI, when supplied by the action.
    pub documentation: Option<String>,
    /// JSON argument schema, or `None` for an action without a public schema.
    pub argument_schema: Option<Value>,
}

/// Coordinate space for a point inside the active story route.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StoryPointSpace {
    /// Fractions across the active route bounds in the inclusive range
    /// `0.0..=1.0`.
    #[default]
    Normalized,
    /// GPUI logical pixels measured from the active route origin.
    LogicalPixels,
}

/// A point relative to the active story or substory capture region.
#[derive(Clone, Copy, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StoryPoint {
    /// Coordinate space. Omission defaults to [`StoryPointSpace::Normalized`].
    #[serde(default)]
    pub space: StoryPointSpace,
    /// Horizontal coordinate in `space`.
    pub x: f32,
    /// Vertical coordinate in `space`.
    pub y: f32,
}

/// Mouse buttons supported by in-process story interaction.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StoryMouseButton {
    /// Primary mouse button.
    #[default]
    Left,
    /// Secondary mouse button.
    Right,
    /// Middle mouse button.
    Middle,
}

/// Keyboard modifiers supported by pointer interaction.
#[derive(
    Clone, Copy, Debug, Deserialize, Eq, JsonSchema, Ord, PartialEq, PartialOrd, Serialize,
)]
#[serde(rename_all = "snake_case")]
pub enum StoryModifier {
    /// Control modifier.
    Control,
    /// Alt or Option modifier.
    Alt,
    /// Shift modifier.
    Shift,
    /// Platform command modifier: Command on macOS or Control elsewhere.
    Platform,
    /// Function modifier.
    Function,
}

/// Modifier keys held while dispatching a pointer step.
#[derive(Clone, Debug, Default, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(transparent)]
pub struct StoryModifiers(pub Vec<StoryModifier>);

/// One ordered interaction operation.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum StoryInteractionStep {
    /// Move focus to the next focusable element.
    FocusNext,
    /// Move focus to the previous focusable element.
    FocusPrevious,
    /// Clear window focus.
    Blur,
    /// Parse and dispatch GPUI key-binding strings in order.
    Keystrokes {
        /// GPUI keystroke strings such as `enter` or `shift-tab`.
        keys: Vec<String>,
    },
    /// Insert one UTF-8 value into the focused basic text input.
    Text {
        /// Text value. This is not IME, clipboard, or dead-key simulation.
        value: String,
    },
    /// Build and dispatch one runtime-registered GPUI action.
    DispatchAction {
        /// Runtime name returned by action discovery.
        name: String,
        /// JSON arguments validated by the action builder.
        args: Option<Value>,
    },
    /// Move the pointer inside the active route bounds.
    PointerMove {
        /// Route-relative destination.
        point: StoryPoint,
    },
    /// Dispatch pointer move, button down, and button up at one point.
    PointerClick {
        /// Route-relative click destination.
        point: StoryPoint,
        /// Button, defaulting to left.
        #[serde(default)]
        button: StoryMouseButton,
        /// Positive click count, defaulting to one.
        #[serde(default = "default_click_count")]
        click_count: u8,
        /// Modifiers held for move, down, and up.
        #[serde(default)]
        modifiers: StoryModifiers,
    },
    /// Click the center of one stable semantic target in the active route.
    ClickTarget {
        /// Target key returned by semantic target discovery.
        target_key: String,
        /// Button, defaulting to left.
        #[serde(default)]
        button: StoryMouseButton,
        /// Positive click count, defaulting to one.
        #[serde(default = "default_click_count")]
        click_count: u8,
        /// Modifiers held for move, down, and up.
        #[serde(default)]
        modifiers: StoryModifiers,
    },
    /// Dispatch one pixel scroll event inside the active route bounds.
    Scroll {
        /// Route-relative event destination.
        point: StoryPoint,
        /// Horizontal logical-pixel delta.
        delta_x: f32,
        /// Vertical logical-pixel delta.
        delta_y: f32,
    },
    /// Refresh and continue after a positive number of rendered frames.
    WaitFrames {
        /// Rendered frames to wait.
        count: u16,
    },
}

const fn default_click_count() -> u8 {
    1
}

/// Optional PNG capture performed after the final interaction step.
#[derive(Clone, Debug, Default, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StoryInteractionCaptureRequest {
    /// PNG destination. The normal capture default is used when omitted.
    pub output_path: Option<PathBuf>,
}

/// Maximum number of refreshed frames used for one semantic postcondition when
/// the caller does not provide an explicit bound.
pub const DEFAULT_INTERACTION_POSTCONDITION_FRAMES: u16 = MAX_INTERACTION_WAITED_FRAMES;

/// Maximum number of exact semantic-value postconditions accepted by one
/// interaction request.
pub const MAX_INTERACTION_POSTCONDITIONS: usize = MAX_INTERACTION_STEPS;

/// Exact semantic-value assertion evaluated after all interaction steps.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StoryInteractionPostcondition {
    /// Stable semantic value key rendered by the active story route.
    pub value_key: String,
    /// Optional RFC 6901 JSON Pointer inside the semantic value.
    #[serde(default)]
    pub json_pointer: Option<String>,
    /// Expected JSON value. Matching uses exact [`serde_json::Value`] equality.
    pub expected: Value,
    /// Maximum freshly rendered frames to inspect. Omission uses the bounded
    /// default [`DEFAULT_INTERACTION_POSTCONDITION_FRAMES`].
    #[serde(default)]
    pub max_frames: Option<u16>,
}

impl StoryInteractionPostcondition {
    /// Creates an exact assertion against the complete semantic value.
    pub fn new(value_key: impl Into<String>, expected: Value) -> Self {
        Self {
            value_key: value_key.into(),
            json_pointer: None,
            expected,
            max_frames: None,
        }
    }

    /// Restricts the assertion to one RFC 6901 JSON Pointer.
    pub fn json_pointer(mut self, json_pointer: impl Into<String>) -> Self {
        self.json_pointer = Some(json_pointer.into());
        self
    }

    /// Sets the bounded number of rendered frames used for matching.
    pub fn max_frames(mut self, max_frames: u16) -> Self {
        self.max_frames = Some(max_frames);
        self
    }

    pub(crate) fn frame_limit(&self) -> u16 {
        self.max_frames
            .unwrap_or(DEFAULT_INTERACTION_POSTCONDITION_FRAMES)
    }
}

/// Successful result for one exact semantic-value postcondition.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StoryInteractionPostconditionSnapshot {
    /// Stable semantic value key checked by the executor.
    pub value_key: String,
    /// JSON Pointer used for the comparison, if any.
    pub json_pointer: Option<String>,
    /// Exact expected JSON value.
    pub expected: Value,
    /// Actual semantic value snapshot containing the matching value.
    pub actual: StorySemanticValueSnapshot,
    /// Freshly rendered frames inspected before this assertion matched.
    pub frames_waited: u16,
}

/// One exclusive interaction batch.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StoryInteractionRequest {
    /// Stable story or substory route opened before controls and input.
    pub story_key: Option<String>,
    /// Typed control values applied before input.
    #[serde(default)]
    pub controls: BTreeMap<String, ControlValue>,
    /// Requested story-region width in physical pixels, supplied with `height`.
    pub width: Option<u32>,
    /// Requested story-region height in physical pixels, supplied with `width`.
    pub height: Option<u32>,
    /// Named viewport used when explicit dimensions are omitted.
    pub viewport: Option<StoryViewportPreset>,
    /// Optional presentation applied before controls and input.
    #[serde(default)]
    pub presentation: Option<crate::presentation::StoryPresentation>,
    /// Ordered non-empty interaction steps.
    pub steps: Vec<StoryInteractionStep>,
    /// Exact semantic-value checks evaluated after all interaction steps.
    #[serde(default)]
    pub postconditions: Vec<StoryInteractionPostcondition>,
    /// Optional first-frame capture after the final step or explicit waits.
    pub capture: Option<StoryInteractionCaptureRequest>,
}

/// Dispatch information reported directly by GPUI.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum StoryInteractionDispatch {
    /// The API reports dispatch but no handler outcome.
    Dispatched,
    /// A keystroke or text input dispatch result.
    Input {
        /// Whether GPUI handled the input.
        handled: bool,
    },
    /// A pointer or scroll platform-event result.
    PlatformEvent {
        /// Whether the event propagated through GPUI.
        propagated: bool,
        /// Whether a handler prevented default behavior.
        default_prevented: bool,
    },
}

/// Dispatch observations for one completed interaction step.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StoryInteractionObservation {
    /// Zero-based request step index.
    pub step_index: usize,
    /// One entry per GPUI dispatch made by the step.
    pub dispatches: Vec<StoryInteractionDispatch>,
}

/// Result of an interaction batch executed by the live GPUI host.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StoryInteractionSnapshot {
    /// Monotonic request ID assigned by this automation controller.
    pub request_id: u64,
    /// Story and route used by the executor.
    pub story: StorySnapshot,
    /// Count completed through a safe executor boundary.
    pub steps_dispatched: usize,
    /// Dispatch information for completed steps.
    pub observations: Vec<StoryInteractionObservation>,
    /// Whether any focus handle exists in the window after the batch.
    pub focused: bool,
    /// Exact semantic-value postconditions that matched after the batch.
    pub postconditions: Vec<StoryInteractionPostconditionSnapshot>,
    /// Optional capture produced within this batch.
    pub capture: Option<StoryCaptureSnapshot>,
}

mod request;
mod runner;

pub(crate) use request::{
    PreparedInteractionStep, list_registered_actions, prepare_interaction_steps,
    validate_interaction_request,
};
pub(crate) use runner::{
    PreparedStoryInteraction, interaction_target_size, schedule_interaction_target_listing,
    schedule_story_interaction,
};

#[cfg(test)]
mod tests;
