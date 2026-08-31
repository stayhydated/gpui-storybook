//! Typed MCP inputs and structured output envelopes.

use std::{collections::BTreeMap, path::PathBuf};

use component_shape_mcp::{McpJsonSchema, McpSchema, McpToolError, McpToolInput};
use gpui_storybook_core::{
    automation::{
        StoryActionSnapshot, StoryInteractionCaptureRequest, StoryInteractionStep,
        StoryScenarioSnapshot, StorySemanticValueSnapshot, StorySnapshot,
    },
    controls::ControlValue,
    presentation::StoryViewportPreset,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::schema::deserialize_schema;

#[derive(Clone, Debug, Deserialize)]
#[serde(transparent)]
pub(crate) struct SchemarsValue<T>(pub(crate) T);

impl<T> SchemarsValue<T> {
    pub(crate) fn into_inner(self) -> T {
        self.0
    }
}

impl<T: JsonSchema> McpJsonSchema for SchemarsValue<T> {
    fn json_schema() -> McpSchema {
        deserialize_schema::<T>()
    }
}

#[derive(JsonSchema, Serialize)]
#[schemars(deny_unknown_fields)]
pub(crate) struct ListStoriesOutput {
    pub(crate) stories: Vec<StorySnapshot>,
}

#[derive(JsonSchema, Serialize)]
#[schemars(deny_unknown_fields)]
pub(crate) struct ListScenariosOutput {
    pub(crate) story: StorySnapshot,
    pub(crate) scenarios: Vec<StoryScenarioSnapshot>,
}

#[derive(JsonSchema, Serialize)]
#[schemars(deny_unknown_fields)]
pub(crate) struct StoryOutput {
    pub(crate) story: StorySnapshot,
}

#[derive(Clone, Debug, Default, component_shape_mcp::McpToolInput)]
pub(crate) struct ListScenariosInput {
    /// Stable story or sub-story route. Omit to inspect the current story.
    pub(crate) story_key: Option<String>,
}

/// Run one declared story-owned interaction scenario from a fresh instance.
#[derive(Clone, Debug, component_shape_mcp::McpToolInput)]
pub(crate) struct RunScenarioInput {
    /// Stable story or sub-story route. Omit to use the current story.
    pub(crate) story_key: Option<String>,
    /// Stable scenario key from `storybook_list_scenarios`.
    pub(crate) scenario_key: String,
}

#[derive(Debug, JsonSchema, PartialEq, Serialize)]
#[schemars(deny_unknown_fields)]
pub(crate) struct SemanticValueOutput {
    pub(crate) story: StorySnapshot,
    pub(crate) semantic_value: StorySemanticValueSnapshot,
}

#[derive(Debug, JsonSchema, PartialEq, Serialize)]
#[schemars(deny_unknown_fields)]
pub(crate) struct WaitForValueOutput {
    pub(crate) story: StorySnapshot,
    pub(crate) semantic_value: StorySemanticValueSnapshot,
    pub(crate) frames_waited: u16,
}

#[derive(JsonSchema, Serialize)]
#[schemars(deny_unknown_fields)]
pub(crate) struct ListActionsOutput {
    pub(crate) actions: Vec<StoryActionSnapshot>,
}

/// No arguments.
#[derive(Clone, Debug, Default)]
pub(crate) struct EmptyInput {}

impl McpToolInput for EmptyInput {
    fn input_schema() -> McpSchema {
        McpSchema::object().with_additional_properties(false)
    }

    fn from_tool_call(call: component_shape_mcp::McpToolCall) -> Result<Self, McpToolError> {
        call.into_arguments().finish()?;
        Ok(Self {})
    }
}

/// Select one registered story or sub-story route.
#[derive(Clone, Debug, component_shape_mcp::McpToolInput)]
pub(crate) struct StoryKeyInput {
    /// Stable story key or `story-key/substory-key` route.
    pub(crate) story_key: String,
}

/// Capture the story currently displayed by the live storybook window.
#[derive(Clone, Debug, Default, component_shape_mcp::McpToolInput)]
pub(crate) struct CaptureCurrentStoryInput {
    /// PNG output path. The capture runtime chooses its default when omitted.
    pub(crate) output_path: Option<PathBuf>,
    /// Requested captured story-region width in pixels. Set together with `height`.
    pub(crate) width: Option<u32>,
    /// Requested captured story-region height in pixels. Set together with `width`.
    pub(crate) height: Option<u32>,
    /// Named viewport (`responsive`, `mobile`, `tablet`, or `desktop`).
    pub(crate) viewport: Option<SchemarsValue<StoryViewportPreset>>,
    /// Tagged `ControlValue` objects keyed by control name, applied before capture.
    pub(crate) controls: Option<BTreeMap<String, SchemarsValue<ControlValue>>>,
}

/// Run an ordered in-process interaction batch.
#[derive(Clone, Debug, component_shape_mcp::McpToolInput)]
pub(crate) struct RunStepsInput {
    /// Stable story or substory route to open before interaction.
    pub(crate) story_key: Option<String>,
    /// Tagged `ControlValue` objects applied before interaction.
    pub(crate) controls: Option<BTreeMap<String, SchemarsValue<ControlValue>>>,
    /// Requested story-region width in physical pixels. Set together with `height`.
    pub(crate) width: Option<u32>,
    /// Requested story-region height in physical pixels. Set together with `width`.
    pub(crate) height: Option<u32>,
    /// Named viewport used when explicit dimensions are omitted.
    pub(crate) viewport: Option<SchemarsValue<StoryViewportPreset>>,
    /// Ordered, closed, tagged interaction step objects.
    pub(crate) steps: Vec<SchemarsValue<StoryInteractionStep>>,
    /// Optional `{ "output_path": "..." }` next-frame PNG capture request.
    pub(crate) capture: Option<SchemarsValue<StoryInteractionCaptureRequest>>,
}

/// Set one control on the active concrete story instance.
#[derive(Clone, Debug, component_shape_mcp::McpToolInput)]
pub(crate) struct SetControlInput {
    /// Control key from `storybook_read_controls`.
    pub(crate) control_key: String,
    /// Tagged `ControlValue`, for example `{ "type": "boolean", "value": true }`.
    pub(crate) value: SchemarsValue<ControlValue>,
}

/// Reset one active-story control, or all controls when `key` is omitted.
#[derive(Clone, Debug, Default, component_shape_mcp::McpToolInput)]
pub(crate) struct ResetControlInput {
    /// Control key to reset. Omit to reset all controls.
    pub(crate) control_key: Option<String>,
}

/// Click one semantic target, optionally opening its story route first.
#[derive(Clone, Debug, component_shape_mcp::McpToolInput)]
pub(crate) struct ClickTargetInput {
    /// Stable story or substory route to open before the click.
    pub(crate) story_key: Option<String>,
    /// Stable key from `storybook_list_interaction_targets`.
    pub(crate) target_key: String,
}

/// Read one semantic value from the active story route.
#[derive(Clone, Debug, component_shape_mcp::McpToolInput)]
pub(crate) struct ReadValueInput {
    /// Stable key from `storybook_read_semantic_values`.
    pub(crate) value_key: String,
}

/// Wait for one semantic value, or one JSON Pointer inside it, to equal a value.
#[derive(Clone, Debug, component_shape_mcp::McpToolInput)]
pub(crate) struct WaitForValueInput {
    /// Stable key from `storybook_read_semantic_values`.
    pub(crate) value_key: String,
    /// RFC 6901 JSON Pointer inside the semantic value. Omit to compare the complete value.
    pub(crate) json_pointer: Option<String>,
    /// Exact JSON value expected at `json_pointer` or at the semantic value root.
    pub(crate) expected: SchemarsValue<Value>,
    /// Positive number of freshly rendered frames to inspect, defaulting to 120.
    pub(crate) max_frames: Option<u16>,
}

/// Build the environment and platform launch command for a capture-enabled
/// Storybook.
#[derive(Clone, Debug, component_shape_mcp::McpToolInput)]
pub(crate) struct CaptureLaunchEnvInput {
    /// Stable story key or `story-key/substory-key` route.
    pub(crate) story_key: String,
    /// PNG output path. Omit it to open the route without taking a capture.
    pub(crate) output_path: Option<PathBuf>,
    /// One-based frame number to capture.
    pub(crate) frame: Option<u32>,
    /// Requested captured story-region width in pixels. Set together with `height`.
    pub(crate) width: Option<u32>,
    /// Requested captured story-region height in pixels. Set together with `width`.
    pub(crate) height: Option<u32>,
    /// Named viewport used when explicit width and height are omitted.
    pub(crate) viewport: Option<SchemarsValue<StoryViewportPreset>>,
    /// Optional Cargo package passed with `-p`.
    pub(crate) package: Option<String>,
    /// Optional Cargo binary passed with `--bin`.
    pub(crate) bin: Option<String>,
    /// Cargo features passed with `--features`.
    pub(crate) features: Option<Vec<String>>,
    /// Whether to include `GPUI_STORYBOOK_MCP_STDIO=1`; defaults to `true`.
    pub(crate) stdio: Option<bool>,
}
