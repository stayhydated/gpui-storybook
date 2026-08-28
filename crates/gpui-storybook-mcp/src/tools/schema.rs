//! JSON Schema construction for Storybook MCP tools.

use component_shape_mcp::{McpSchema, McpSchemaProperties};
use schemars::{JsonSchema, generate::SchemaSettings};
use serde_json::{Value, json};

use crate::{
    CaptureLaunchEnv, ControlValue, StoryCaptureSnapshot, StoryControlsSnapshot,
    StoryCurrentSnapshot, StoryInteractionCaptureRequest, StoryInteractionSnapshot,
    StoryInteractionTargetsSnapshot, StoryScenarioRunSnapshot, StorySemanticValuesSnapshot,
};

use super::input::{
    ListActionsOutput, ListScenariosOutput, ListStoriesOutput, SemanticValueOutput, StoryOutput,
    WaitForValueOutput,
};

pub(crate) fn set_optional_positive_integer(
    properties: &mut serde_json::Map<String, Value>,
    field: &str,
) {
    let Some(branches) = properties
        .get_mut(field)
        .and_then(Value::as_object_mut)
        .and_then(|schema| schema.get_mut("anyOf"))
        .and_then(Value::as_array_mut)
    else {
        return;
    };
    if let Some(integer) = branches.iter_mut().find_map(|branch| {
        let object = branch.as_object_mut()?;
        (object.get("type").and_then(Value::as_str) == Some("integer")).then_some(object)
    }) {
        integer.insert("minimum".to_string(), json!(1));
    }
}

pub(crate) fn set_optional_bounded_integer(
    properties: &mut serde_json::Map<String, Value>,
    field: &str,
    maximum: u64,
) {
    set_optional_positive_integer(properties, field);
    let Some(branches) = properties
        .get_mut(field)
        .and_then(Value::as_object_mut)
        .and_then(|schema| schema.get_mut("anyOf"))
        .and_then(Value::as_array_mut)
    else {
        return;
    };
    if let Some(integer) = branches.iter_mut().find_map(|branch| {
        let object = branch.as_object_mut()?;
        (object.get("type").and_then(Value::as_str) == Some("integer")).then_some(object)
    }) {
        integer.insert("maximum".to_string(), json!(maximum));
        integer.insert("default".to_string(), json!(maximum));
    }
}

pub(crate) fn schema_with_settings<T: JsonSchema>(settings: SchemaSettings) -> McpSchema {
    let schema = settings
        .with(|settings| {
            settings.meta_schema = None;
            settings.inline_subschemas = true;
        })
        .into_generator()
        .into_root_schema_for::<T>();
    McpSchema::new(schema.to_value())
}

pub(crate) fn deserialize_schema<T: JsonSchema>() -> McpSchema {
    schema_with_settings::<T>(SchemaSettings::draft2020_12().for_deserialize())
}

pub(crate) fn serialize_schema<T: JsonSchema>() -> McpSchema {
    schema_with_settings::<T>(SchemaSettings::draft2020_12().for_serialize())
}

pub(crate) fn object_schema<const N: usize>(
    properties: [(String, McpSchema); N],
    required: impl IntoIterator<Item = &'static str>,
) -> McpSchema {
    McpSchema::object()
        .with_properties(McpSchemaProperties::from(properties))
        .with_required(required)
        .with_additional_properties(false)
}

pub(crate) fn control_value_schema() -> McpSchema {
    deserialize_schema::<ControlValue>()
}

pub(crate) fn story_point_schema() -> McpSchema {
    let normalized_coordinate = || {
        McpSchema::number()
            .with_minimum(0_u64)
            .with_extension("maximum", json!(1.0))
    };
    let logical_coordinate = || McpSchema::number().with_minimum(0_u64);
    let normalized = object_schema(
        [
            (
                "space".to_owned(),
                McpSchema::string()
                    .with_const("normalized")
                    .with_default("normalized"),
            ),
            ("x".to_owned(), normalized_coordinate()),
            ("y".to_owned(), normalized_coordinate()),
        ],
        ["x", "y"],
    );
    let logical = object_schema(
        [
            (
                "space".to_owned(),
                McpSchema::string().with_const("logical_pixels"),
            ),
            ("x".to_owned(), logical_coordinate()),
            ("y".to_owned(), logical_coordinate()),
        ],
        ["space", "x", "y"],
    );
    McpSchema::one_of([normalized, logical])
}

pub(crate) fn story_modifiers_schema() -> McpSchema {
    McpSchema::array(
        McpSchema::string().with_enum_values(["control", "alt", "shift", "platform", "function"]),
    )
    .with_max_items(5)
    .with_unique_items(true)
}

pub(crate) fn tagged_interaction_step_schema<const N: usize>(
    kind: &'static str,
    properties: [(String, McpSchema); N],
    required: impl IntoIterator<Item = &'static str>,
) -> McpSchema {
    let mut all_properties = McpSchemaProperties::new();
    all_properties.insert("type".to_owned(), McpSchema::string().with_const(kind));
    for (name, schema) in properties {
        all_properties.insert(name, schema);
    }
    let mut all_required = vec!["type"];
    all_required.extend(required);
    McpSchema::object()
        .with_properties(all_properties)
        .with_required(all_required)
        .with_additional_properties(false)
}

pub(crate) fn interaction_step_schema() -> McpSchema {
    let empty = |kind| tagged_interaction_step_schema(kind, [], []);
    McpSchema::one_of([
        empty("focus_next"),
        empty("focus_previous"),
        empty("blur"),
        tagged_interaction_step_schema(
            "keystrokes",
            [(
                "keys".to_owned(),
                McpSchema::array(McpSchema::string().with_extension(
                    "maxLength",
                    json!(gpui_storybook_core::automation::MAX_INTERACTION_TEXT_BYTES),
                ))
                .with_min_items(1)
                .with_max_items(gpui_storybook_core::automation::MAX_INTERACTION_STEPS),
            )],
            ["keys"],
        ),
        tagged_interaction_step_schema(
            "text",
            [(
                "value".to_owned(),
                McpSchema::string().with_extension(
                    "maxLength",
                    json!(gpui_storybook_core::automation::MAX_INTERACTION_TEXT_BYTES),
                ),
            )],
            ["value"],
        ),
        tagged_interaction_step_schema(
            "dispatch_action",
            [
                ("name".to_owned(), McpSchema::string()),
                ("args".to_owned(), McpSchema::any()),
            ],
            ["name"],
        ),
        tagged_interaction_step_schema(
            "pointer_move",
            [("point".to_owned(), story_point_schema())],
            ["point"],
        ),
        tagged_interaction_step_schema(
            "pointer_click",
            [
                ("point".to_owned(), story_point_schema()),
                (
                    "button".to_owned(),
                    McpSchema::string()
                        .with_enum_values(["left", "right", "middle"])
                        .with_default("left"),
                ),
                (
                    "click_count".to_owned(),
                    McpSchema::integer()
                        .with_minimum(1_u64)
                        .with_extension("maximum", json!(u8::MAX))
                        .with_default(1),
                ),
                (
                    "modifiers".to_owned(),
                    story_modifiers_schema().with_default(json!([])),
                ),
            ],
            ["point"],
        ),
        tagged_interaction_step_schema(
            "click_target",
            [
                (
                    "target_key".to_owned(),
                    McpSchema::string()
                        .with_extension("minLength", json!(1))
                        .with_extension(
                            "maxLength",
                            json!(gpui_storybook_core::automation::MAX_INTERACTION_TEXT_BYTES),
                        ),
                ),
                (
                    "button".to_owned(),
                    McpSchema::string()
                        .with_enum_values(["left", "right", "middle"])
                        .with_default("left"),
                ),
                (
                    "click_count".to_owned(),
                    McpSchema::integer()
                        .with_minimum(1_u64)
                        .with_extension("maximum", json!(u8::MAX))
                        .with_default(1),
                ),
                (
                    "modifiers".to_owned(),
                    story_modifiers_schema().with_default(json!([])),
                ),
            ],
            ["target_key"],
        ),
        tagged_interaction_step_schema(
            "scroll",
            [
                ("point".to_owned(), story_point_schema()),
                ("delta_x".to_owned(), McpSchema::number()),
                ("delta_y".to_owned(), McpSchema::number()),
            ],
            ["point", "delta_x", "delta_y"],
        ),
        tagged_interaction_step_schema(
            "wait_frames",
            [(
                "count".to_owned(),
                McpSchema::integer().with_minimum(1_u64).with_extension(
                    "maximum",
                    json!(gpui_storybook_core::automation::MAX_INTERACTION_WAITED_FRAMES),
                ),
            )],
            ["count"],
        ),
    ])
}

pub(crate) fn interaction_steps_schema() -> McpSchema {
    McpSchema::array(interaction_step_schema())
        .with_min_items(1)
        .with_max_items(gpui_storybook_core::automation::MAX_INTERACTION_STEPS)
}

pub(crate) fn interaction_capture_request_schema() -> McpSchema {
    deserialize_schema::<StoryInteractionCaptureRequest>()
}

pub(crate) fn story_controls_output_schema() -> McpSchema {
    serialize_schema::<StoryControlsSnapshot>()
}

pub(crate) fn list_stories_output_schema() -> McpSchema {
    serialize_schema::<ListStoriesOutput>()
}

pub(crate) fn list_scenarios_output_schema() -> McpSchema {
    serialize_schema::<ListScenariosOutput>()
}

pub(crate) fn scenario_run_output_schema() -> McpSchema {
    serialize_schema::<StoryScenarioRunSnapshot>()
}

pub(crate) fn get_story_output_schema() -> McpSchema {
    serialize_schema::<StoryOutput>()
}

pub(crate) fn current_story_output_schema() -> McpSchema {
    serialize_schema::<StoryCurrentSnapshot>()
}

pub(crate) fn capture_story_output_schema() -> McpSchema {
    serialize_schema::<StoryCaptureSnapshot>()
}

pub(crate) fn list_actions_output_schema() -> McpSchema {
    serialize_schema::<ListActionsOutput>()
}

pub(crate) fn interaction_output_schema() -> McpSchema {
    serialize_schema::<StoryInteractionSnapshot>()
}

pub(crate) fn interaction_targets_output_schema() -> McpSchema {
    serialize_schema::<StoryInteractionTargetsSnapshot>()
}

pub(crate) fn semantic_values_output_schema() -> McpSchema {
    serialize_schema::<StorySemanticValuesSnapshot>()
}

pub(crate) fn semantic_value_output_schema() -> McpSchema {
    serialize_schema::<SemanticValueOutput>()
}

pub(crate) fn wait_for_value_output_schema() -> McpSchema {
    serialize_schema::<WaitForValueOutput>()
}

pub(crate) fn capture_launch_env_output_schema() -> McpSchema {
    serialize_schema::<CaptureLaunchEnv>()
}
