//! MCP tool definitions, request decoding, and automation error mapping.

mod input;
mod registry;
mod schema;

use std::{collections::BTreeMap, time::Duration};

use component_shape_mcp::{
    McpSchema, McpToolError, McpToolInput, McpToolMetadata, McpToolRegistry, McpTypedTool,
    nullable_schema, tool_definition_for_input_with_metadata, tool_error_result_for,
    tool_structured_result,
};
use frame_capture::CaptureLaunchEnv as FrameCaptureLaunchEnv;
use gpui_storybook_core::automation::{
    MAX_INTERACTION_WAITED_FRAMES, SharedStorybookAutomation, StoryInteractionRequest,
    StoryInteractionStep, StoryModifiers, StoryMouseButton, StoryScreenshotRequest,
    StorySemanticValueSnapshot, StorySemanticValuesSnapshot, StorybookAutomation,
    StorybookAutomationError,
};
use rmcp::model::CallToolResult as ToolCallResult;
use serde_json::{Value, json};

use crate::{
    CaptureLaunchEnv, ControlValue, STDIO_ENV_VAR, StoryViewportPreset, StorybookMcpError,
    StorybookMcpServerOptions, capture::storybook_capture_env,
};

pub(crate) use input::*;
pub use registry::{
    register_tools, register_tools_with_options, tool_registry, tool_registry_with_options,
};
pub(crate) use schema::*;

pub const TOOL_LIST_STORIES: &str = "storybook_list_stories";
pub const TOOL_LIST_SCENARIOS: &str = "storybook_list_scenarios";
pub const TOOL_RUN_SCENARIO: &str = "storybook_run_scenario";
pub const TOOL_GET_STORY: &str = "storybook_get_story";
pub const TOOL_CURRENT_STORY: &str = "storybook_current_story";
pub const TOOL_OPEN_STORY: &str = "storybook_open_story";
pub const TOOL_READ_CONTROLS: &str = "storybook_read_controls";
pub const TOOL_SET_CONTROL: &str = "storybook_set_control";
pub const TOOL_RESET_CONTROL: &str = "storybook_reset_control";
pub const TOOL_CAPTURE_CURRENT_STORY: &str = "storybook_capture_current_story";
pub const TOOL_CAPTURE_LAUNCH_ENV: &str = "storybook_capture_launch_env";
pub const TOOL_LIST_ACTIONS: &str = "storybook_list_actions";
pub const TOOL_LIST_INTERACTION_TARGETS: &str = "storybook_list_interaction_targets";
pub const TOOL_READ_SEMANTIC_VALUES: &str = "storybook_read_semantic_values";
pub const TOOL_READ_VALUE: &str = "storybook_read_value";
pub const TOOL_WAIT_FOR_VALUE: &str = "storybook_wait_for_value";
pub const TOOL_CLICK_TARGET: &str = "storybook_click_target";
pub const TOOL_RUN_STEPS: &str = "storybook_run_steps";

const AUTOMATION_STARTUP_TIMEOUT_SECS: u64 = 30;
const SEMANTIC_VALUE_WAIT_TIMEOUT_SECS: u64 = 30;

#[derive(Clone, Copy)]
struct ToolHints {
    read_only: bool,
    destructive: bool,
    idempotent: bool,
    open_world: bool,
}

impl ToolHints {
    const fn read_only() -> Self {
        Self {
            read_only: true,
            destructive: false,
            idempotent: true,
            open_world: false,
        }
    }

    const fn mutation(idempotent: bool, destructive: bool) -> Self {
        Self {
            read_only: false,
            destructive,
            idempotent,
            open_world: false,
        }
    }

    const fn interaction() -> Self {
        Self {
            read_only: false,
            destructive: true,
            idempotent: false,
            open_world: true,
        }
    }
}

fn tool<Input>(
    name: &'static str,
    title: &'static str,
    description: &'static str,
    output_schema: McpSchema,
    hints: ToolHints,
) -> Result<McpTypedTool<Input>, McpToolError>
where
    Input: McpToolInput,
{
    tool_definition_for_input_with_metadata(
        name,
        McpToolMetadata::new()
            .with_title(title)
            .with_description(description)
            .with_read_only_hint(hints.read_only)
            .with_destructive_hint(hints.destructive)
            .with_idempotent_hint(hints.idempotent)
            .with_open_world_hint(hints.open_world),
        Some(output_schema),
    )
}

fn capture_tool<Input>(
    name: &'static str,
    title: &'static str,
    description: &'static str,
    output_schema: McpSchema,
    hints: ToolHints,
    has_frame: bool,
) -> Result<McpTypedTool<Input>, McpToolError>
where
    Input: McpToolInput,
{
    let mut tool = tool(name, title, description, output_schema, hints)?;
    let input_schema = std::sync::Arc::make_mut(&mut tool.definition_mut().input_schema);
    input_schema.insert(
        "dependentRequired".to_string(),
        json!({
            "width": ["height"],
            "height": ["width"],
        }),
    );
    if let Some(properties) = input_schema
        .get_mut("properties")
        .and_then(Value::as_object_mut)
    {
        set_optional_positive_integer(properties, "width");
        set_optional_positive_integer(properties, "height");
        if has_frame {
            set_optional_positive_integer(properties, "frame");
        }
    }
    Ok(tool)
}

fn interaction_tool() -> Result<McpTypedTool<RunStepsInput>, McpToolError> {
    let mut tool = tool::<RunStepsInput>(
        TOOL_RUN_STEPS,
        "Run Steps",
        "Run one exclusive, ordered in-process interaction batch against the active story region, with optional next-frame capture. Clicks and actions can trigger arbitrary application effects.",
        interaction_output_schema(),
        ToolHints::interaction(),
    )?;
    let input_schema = std::sync::Arc::make_mut(&mut tool.definition_mut().input_schema);
    input_schema.insert(
        "dependentRequired".to_string(),
        json!({
            "width": ["height"],
            "height": ["width"],
        }),
    );
    if let Some(properties) = input_schema
        .get_mut("properties")
        .and_then(Value::as_object_mut)
    {
        properties.insert(
            "controls".to_owned(),
            nullable_schema(McpSchema::object().with_additional_properties(control_value_schema()))
                .into_value(),
        );
        properties.insert("steps".to_owned(), interaction_steps_schema().into_value());
        properties.insert(
            "capture".to_owned(),
            nullable_schema(interaction_capture_request_schema()).into_value(),
        );
        set_optional_positive_integer(properties, "width");
        set_optional_positive_integer(properties, "height");
    }
    Ok(tool)
}

fn wait_for_value_tool() -> Result<McpTypedTool<WaitForValueInput>, McpToolError> {
    let mut tool = tool::<WaitForValueInput>(
        TOOL_WAIT_FOR_VALUE,
        "Wait for Value",
        "Refresh the active story for a bounded number of frames until one semantic value, or one JSON Pointer inside it, exactly matches the expected JSON value.",
        wait_for_value_output_schema(),
        ToolHints::read_only(),
    )?;
    if let Some(properties) = std::sync::Arc::make_mut(&mut tool.definition_mut().input_schema)
        .get_mut("properties")
        .and_then(Value::as_object_mut)
    {
        set_optional_bounded_integer(
            properties,
            "max_frames",
            u64::from(MAX_INTERACTION_WAITED_FRAMES),
        );
    }
    Ok(tool)
}

fn automation_tool_error(error: StorybookAutomationError) -> ToolCallResult {
    let error = match error {
        StorybookAutomationError::StoryNotFound { key } => {
            McpToolError::invalid_field_value("story_key", key)
        },
        error => structured_automation_error(error),
    };
    tool_error_result_for(error)
}

pub(crate) fn interaction_automation_tool_error(error: StorybookAutomationError) -> ToolCallResult {
    let error = match error {
        StorybookAutomationError::StoryNotFound { key } => {
            McpToolError::invalid_field_value("story_key", key)
        },
        error => structured_automation_error(error),
    };
    tool_error_result_for(error)
}

pub(crate) fn structured_automation_error(error: StorybookAutomationError) -> McpToolError {
    let detail = match &error {
        StorybookAutomationError::StartupTimedOut { seconds } => json!({
            "code": "startup_timed_out",
            "seconds": seconds,
        }),
        StorybookAutomationError::NoLiveHost => json!({ "code": "no_live_host" }),
        StorybookAutomationError::HostDisconnected {
            steps_dispatched, ..
        } => json!({
            "code": "host_disconnected",
            "steps_dispatched": steps_dispatched,
        }),
        StorybookAutomationError::AutomationBusy => json!({ "code": "automation_busy" }),
        StorybookAutomationError::CaptureUnavailable { .. } => {
            json!({ "code": "capture_unavailable" })
        },
        StorybookAutomationError::InvalidCaptureRequest { .. } => {
            json!({ "code": "invalid_capture_request" })
        },
        StorybookAutomationError::InvalidInteractionRequest { .. } => {
            json!({ "code": "invalid_interaction_request" })
        },
        StorybookAutomationError::InvalidInteractionStep { step_index, .. } => json!({
            "code": "invalid_interaction_step",
            "step_index": step_index,
            "steps_dispatched": 0,
        }),
        StorybookAutomationError::InvalidInteractionPostcondition {
            postcondition_index,
            ..
        } => json!({
            "code": "invalid_interaction_postcondition",
            "postcondition_index": postcondition_index,
            "steps_dispatched": 0,
        }),
        StorybookAutomationError::InteractionFailed {
            request_id,
            steps_dispatched,
            ..
        } => json!({
            "code": "interaction_failed",
            "request_id": request_id,
            "steps_dispatched": steps_dispatched,
        }),
        StorybookAutomationError::NoActiveStory => json!({ "code": "no_active_story" }),
        StorybookAutomationError::ControlsUnavailable { key } => json!({
            "code": "controls_unavailable",
            "story_key": key,
        }),
        StorybookAutomationError::ControlOperationFailed { .. } => {
            json!({ "code": "control_operation_failed" })
        },
        StorybookAutomationError::InteractionTargetsUnavailable { route } => json!({
            "code": "interaction_targets_unavailable",
            "story_key": route,
        }),
        StorybookAutomationError::InteractionTargetNotFound { route, key } => json!({
            "code": "interaction_target_not_found",
            "story_key": route,
            "target_key": key,
            "steps_dispatched": 0,
        }),
        StorybookAutomationError::DuplicateInteractionTarget { route, key } => json!({
            "code": "duplicate_interaction_target",
            "story_key": route,
            "target_key": key,
            "steps_dispatched": 0,
        }),
        StorybookAutomationError::SemanticValuesUnavailable { route } => json!({
            "code": "semantic_values_unavailable",
            "story_key": route,
        }),
        StorybookAutomationError::SemanticValueNotFound { route, key } => json!({
            "code": "semantic_value_not_found",
            "story_key": route,
            "value_key": key,
        }),
        StorybookAutomationError::SemanticValueWaitTimedOut {
            route,
            key,
            max_frames,
        } => json!({
            "code": "semantic_value_wait_timed_out",
            "story_key": route,
            "value_key": key,
            "max_frames": max_frames,
        }),
        StorybookAutomationError::DuplicateSemanticValue { route, key } => json!({
            "code": "duplicate_semantic_value",
            "story_key": route,
            "value_key": key,
        }),
        StorybookAutomationError::ScenarioNotFound {
            story_key,
            scenario_key,
        } => json!({
            "code": "scenario_not_found",
            "story_key": story_key,
            "scenario_key": scenario_key,
        }),
        StorybookAutomationError::DuplicateScenarioKey {
            story_key,
            scenario_key,
        } => json!({
            "code": "duplicate_scenario_key",
            "story_key": story_key,
            "scenario_key": scenario_key,
        }),
        StorybookAutomationError::StoryNotFound { key } => json!({
            "code": "story_not_found",
            "story_key": key,
        }),
    };
    McpToolError::validation_structured_details(error.to_string(), [detail])
}

fn decode_control_map(
    controls: Option<BTreeMap<String, SchemarsValue<ControlValue>>>,
) -> BTreeMap<String, ControlValue> {
    controls
        .unwrap_or_default()
        .into_iter()
        .map(|(key, value)| (key, value.into_inner()))
        .collect()
}

fn decode_interaction_request(input: RunStepsInput) -> StoryInteractionRequest {
    StoryInteractionRequest {
        story_key: input.story_key,
        controls: decode_control_map(input.controls),
        width: input.width,
        height: input.height,
        viewport: input.viewport.map(SchemarsValue::into_inner),
        presentation: None,
        steps: input
            .steps
            .into_iter()
            .map(SchemarsValue::into_inner)
            .collect(),
        postconditions: Vec::new(),
        capture: input.capture.map(SchemarsValue::into_inner),
    }
}

async fn await_automation_startup(
    automation: &StorybookAutomation,
) -> Result<(), StorybookAutomationError> {
    tokio::time::timeout(
        Duration::from_secs(AUTOMATION_STARTUP_TIMEOUT_SECS),
        automation.wait_until_ready(),
    )
    .await
    .map_err(|_| StorybookAutomationError::StartupTimedOut {
        seconds: AUTOMATION_STARTUP_TIMEOUT_SECS,
    })
}

async fn read_semantic_value(
    automation: &StorybookAutomation,
    value_key: &str,
) -> Result<SemanticValueOutput, StorybookAutomationError> {
    let snapshot = automation.read_semantic_values().await?;
    semantic_value_output(snapshot, value_key)
}

pub(crate) fn semantic_value_output(
    snapshot: StorySemanticValuesSnapshot,
    value_key: &str,
) -> Result<SemanticValueOutput, StorybookAutomationError> {
    let route = snapshot.story.capture_route_id.clone();
    let semantic_value = snapshot
        .values
        .into_iter()
        .find(|value| value.key == value_key)
        .ok_or_else(|| StorybookAutomationError::SemanticValueNotFound {
            route,
            key: value_key.to_owned(),
        })?;
    Ok(SemanticValueOutput {
        story: snapshot.story,
        semantic_value,
    })
}

async fn wait_for_semantic_value(
    automation: &StorybookAutomation,
    input: WaitForValueInput,
) -> Result<WaitForValueOutput, StorybookAutomationError> {
    let max_frames = input.max_frames.unwrap_or(MAX_INTERACTION_WAITED_FRAMES);
    let value_key = input.value_key;
    let json_pointer = input.json_pointer;
    let expected = input.expected.into_inner();
    let wait = async {
        let mut last_route = automation
            .current_story()
            .story
            .map(|story| story.capture_route_id)
            .unwrap_or_else(|| "<active-story>".to_owned());

        for frames_waited in 1..=max_frames {
            let snapshot = automation.read_semantic_values().await?;
            last_route.clone_from(&snapshot.story.capture_route_id);
            if let Some(semantic_value) = snapshot
                .values
                .into_iter()
                .find(|value| value.key == value_key)
                && semantic_value_matches(&semantic_value, json_pointer.as_deref(), &expected)
            {
                return Ok(WaitForValueOutput {
                    story: snapshot.story,
                    semantic_value,
                    frames_waited,
                });
            }
        }

        Err(StorybookAutomationError::SemanticValueWaitTimedOut {
            route: last_route,
            key: value_key.clone(),
            max_frames,
        })
    };

    tokio::time::timeout(Duration::from_secs(SEMANTIC_VALUE_WAIT_TIMEOUT_SECS), wait)
        .await
        .unwrap_or_else(|_| {
            Err(StorybookAutomationError::SemanticValueWaitTimedOut {
                route: automation
                    .current_story()
                    .story
                    .map(|story| story.capture_route_id)
                    .unwrap_or_else(|| "<active-story>".to_owned()),
                key: value_key,
                max_frames,
            })
        })
}

pub(crate) fn semantic_value_matches(
    semantic_value: &StorySemanticValueSnapshot,
    json_pointer: Option<&str>,
    expected: &Value,
) -> bool {
    json_pointer.map_or(Some(&semantic_value.value), |pointer| {
        semantic_value.value.pointer(pointer)
    }) == Some(expected)
}

pub(crate) fn click_target_request(input: ClickTargetInput) -> StoryInteractionRequest {
    StoryInteractionRequest {
        story_key: input.story_key,
        controls: BTreeMap::new(),
        width: None,
        height: None,
        viewport: None,
        presentation: None,
        steps: vec![StoryInteractionStep::ClickTarget {
            target_key: input.target_key,
            button: StoryMouseButton::default(),
            click_count: 1,
            modifiers: StoryModifiers::default(),
        }],
        postconditions: Vec::new(),
        capture: None,
    }
}

pub(crate) fn validate_wait_for_value_input(input: &WaitForValueInput) -> Result<(), McpToolError> {
    if input.value_key.trim().is_empty() {
        return Err(McpToolError::invalid_field_value(
            "value_key",
            "value keys must not be empty",
        ));
    }
    if input
        .json_pointer
        .as_deref()
        .is_some_and(|pointer| !pointer.is_empty() && !pointer.starts_with('/'))
    {
        return Err(McpToolError::invalid_field_value(
            "json_pointer",
            "JSON Pointers must be empty or start with `/`",
        ));
    }
    if input
        .max_frames
        .is_some_and(|frames| frames == 0 || frames > MAX_INTERACTION_WAITED_FRAMES)
    {
        return Err(McpToolError::invalid_field_value(
            "max_frames",
            format!("max_frames must be between 1 and {MAX_INTERACTION_WAITED_FRAMES}"),
        ));
    }
    Ok(())
}

pub(crate) fn build_capture_launch_env(
    input: CaptureLaunchEnvInput,
) -> Result<CaptureLaunchEnv, StorybookMcpError> {
    let viewport = input.viewport.map(SchemarsValue::into_inner);
    let (width, height) = match (input.width, input.height) {
        (None, None) => viewport
            .and_then(StoryViewportPreset::dimensions)
            .map_or((None, None), |(width, height)| (Some(width), Some(height))),
        dimensions => dimensions,
    };
    let size = FrameCaptureLaunchEnv::optional_size(width, height)?;
    let mut env = FrameCaptureLaunchEnv::builder()
        .route_id(input.story_key)?
        .env(storybook_capture_env())
        .maybe_output_path(input.output_path)?
        .maybe_frame(input.frame)?
        .maybe_size(size)?
        .build()
        .env_map_lossy();
    if input.stdio.unwrap_or(true) {
        env.insert(STDIO_ENV_VAR.to_string(), "1".to_string());
    }

    let mut cargo_args = vec!["run".to_string()];
    if let Some(package) = input.package {
        cargo_args.extend(["-p".to_string(), package]);
    }
    if let Some(features) = input.features
        && !features.is_empty()
    {
        cargo_args.extend(["--features".to_string(), features.join(",")]);
    }
    if let Some(bin) = input.bin {
        cargo_args.extend(["--bin".to_string(), bin]);
    }

    let command = cargo_launch_command(&cargo_args);

    Ok(CaptureLaunchEnv {
        env,
        cargo_args,
        command,
    })
}

fn cargo_launch_command(cargo_args: &[String]) -> Vec<String> {
    #[cfg(target_os = "linux")]
    let command_prefix = &["gpui-storybook-launch", "--", "cargo"];

    #[cfg(not(target_os = "linux"))]
    let command_prefix = &["cargo"];

    cargo_launch_command_for(command_prefix, cargo_args)
}

pub(crate) fn cargo_launch_command_for(
    command_prefix: &[&str],
    cargo_args: &[String],
) -> Vec<String> {
    let mut command = command_prefix
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>();

    command.extend(cargo_args.iter().cloned());
    command
}
