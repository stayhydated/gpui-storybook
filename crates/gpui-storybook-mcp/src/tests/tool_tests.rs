use super::*;

#[test]
fn tools_advertise_typed_inputs_outputs_and_annotations() {
    let server = server(StorybookAutomation::with_stories(vec![sample_story()]))
        .expect("server should build");
    let server_info = rmcp::ServerHandler::get_info(&server);
    assert_eq!(
        server_info.protocol_version,
        rmcp::model::ProtocolVersion::V_2026_07_28
    );
    assert_eq!(server_info.server_info.name, "gpui-storybook");

    let tools = serde_json::to_value(server.list_tools()).expect("tools should serialize");
    let tools = tools.as_array().expect("tools should be an array");
    let find = |name: &str| {
        tools
            .iter()
            .find(|tool| tool["name"] == name)
            .unwrap_or_else(|| panic!("tool `{name}` should be registered"))
    };

    let list = find(TOOL_LIST_STORIES);
    assert_eq!(list["inputSchema"]["additionalProperties"], false);
    assert_eq!(
        list["outputSchema"]["properties"]["stories"]["type"],
        "array"
    );
    assert_eq!(list["annotations"]["readOnlyHint"], true);
    assert_eq!(list["annotations"]["openWorldHint"], false);

    let get = find(TOOL_GET_STORY);
    assert_eq!(get["inputSchema"]["required"], json!(["story_key"]));
    assert_eq!(
        get["inputSchema"]["properties"]["story_key"]["type"],
        "string"
    );
    assert_eq!(get["outputSchema"]["properties"]["story"]["type"], "object");

    let scenarios = find(TOOL_LIST_SCENARIOS);
    assert_eq!(scenarios["inputSchema"]["additionalProperties"], false);
    assert_eq!(
        scenarios["outputSchema"]["properties"]["scenarios"]["type"],
        "array"
    );
    assert_eq!(scenarios["annotations"]["readOnlyHint"], true);

    let read_controls = find(TOOL_READ_CONTROLS);
    assert_eq!(read_controls["annotations"]["readOnlyHint"], true);
    assert_eq!(
        read_controls["outputSchema"]["properties"]["controls"]["type"],
        "array"
    );

    let semantic_values = find(TOOL_READ_SEMANTIC_VALUES);
    assert_eq!(
        semantic_values["inputSchema"]["additionalProperties"],
        false
    );
    assert_eq!(semantic_values["annotations"]["readOnlyHint"], true);
    assert_eq!(
        semantic_values["outputSchema"]["properties"]["values"]["type"],
        "array"
    );
    assert_eq!(
        semantic_values["outputSchema"]["properties"]["values"]["items"]["properties"]["value"]["description"],
        "Current JSON value captured from application state during rendering."
    );

    let read_value = find(TOOL_READ_VALUE);
    assert_eq!(read_value["inputSchema"]["required"], json!(["value_key"]));
    assert_eq!(read_value["annotations"]["readOnlyHint"], true);
    assert_eq!(
        read_value["outputSchema"]["properties"]["semantic_value"]["type"],
        "object"
    );

    let wait_for_value = find(TOOL_WAIT_FOR_VALUE);
    assert_eq!(
        wait_for_value["inputSchema"]["required"],
        json!(["value_key", "expected"])
    );
    assert_eq!(wait_for_value["annotations"]["readOnlyHint"], true);
    assert_eq!(
        wait_for_value["inputSchema"]["properties"]["max_frames"]["anyOf"][0]["minimum"],
        1
    );
    assert_eq!(
        wait_for_value["inputSchema"]["properties"]["max_frames"]["anyOf"][0]["maximum"],
        MAX_INTERACTION_WAITED_FRAMES
    );
    assert_eq!(
        wait_for_value["inputSchema"]["properties"]["max_frames"]["anyOf"][0]["default"],
        MAX_INTERACTION_WAITED_FRAMES
    );

    let set_control = find(TOOL_SET_CONTROL);
    assert_eq!(
        set_control["inputSchema"]["required"],
        json!(["control_key", "value"])
    );
    assert_eq!(set_control["annotations"]["idempotentHint"], true);

    let reset_control = find(TOOL_RESET_CONTROL);
    assert_eq!(reset_control["inputSchema"]["required"], json!([]));

    let capture = find(TOOL_CAPTURE_CURRENT_STORY);
    assert_eq!(capture["inputSchema"]["required"], json!([]));
    assert_eq!(
        capture["inputSchema"]["properties"]["width"]["anyOf"][0]["type"],
        "integer"
    );
    assert_eq!(
        capture["inputSchema"]["properties"]["width"]["anyOf"][0]["minimum"],
        1
    );
    assert_eq!(
        capture["inputSchema"]["dependentRequired"]["width"],
        json!(["height"])
    );
    assert_eq!(capture["annotations"]["destructiveHint"], true);
    assert_eq!(
        capture["inputSchema"]["properties"]["viewport"]["anyOf"][0],
        deserialize_schema::<StoryViewportPreset>().into_value()
    );

    let launch = find(TOOL_CAPTURE_LAUNCH_ENV);
    assert_eq!(launch["inputSchema"]["required"], json!(["story_key"]));
    assert_eq!(
        launch["inputSchema"]["properties"]["features"]["anyOf"][0]["type"],
        "array"
    );
    assert_eq!(
        launch["inputSchema"]["properties"]["frame"]["anyOf"][0]["minimum"],
        1
    );
    assert_eq!(
        launch["outputSchema"]["properties"]["env"]["type"],
        "object"
    );
}

#[test]
fn interaction_capability_gates_closed_tools_and_annotations() {
    let automation = StorybookAutomation::with_stories(vec![sample_story()]);
    let disabled = server_with_options(automation.clone(), StorybookMcpServerOptions::default())
        .expect("disabled server should build");
    let disabled_tools =
        serde_json::to_value(disabled.list_tools()).expect("disabled tools should serialize");
    let disabled_tools = disabled_tools.as_array().expect("tools should be an array");
    assert!(
        disabled_tools
            .iter()
            .any(|tool| { tool["name"] == TOOL_LIST_SCENARIOS })
    );
    assert!(!disabled_tools.iter().any(|tool| {
        matches!(
            tool["name"].as_str(),
            Some(
                TOOL_LIST_ACTIONS
                    | TOOL_LIST_INTERACTION_TARGETS
                    | TOOL_CLICK_TARGET
                    | TOOL_RUN_STEPS
            )
        )
    }));

    let enabled = server_with_options(
        automation,
        StorybookMcpServerOptions::default().with_interaction(true),
    )
    .expect("enabled server should build");
    let tools = serde_json::to_value(enabled.list_tools()).expect("tools should serialize");
    let tools = tools.as_array().expect("tools should be an array");
    let find = |name: &str| {
        tools
            .iter()
            .find(|tool| tool["name"] == name)
            .unwrap_or_else(|| panic!("tool `{name}` should be registered"))
    };

    let actions = find(TOOL_LIST_ACTIONS);
    assert_eq!(actions["annotations"]["readOnlyHint"], true);
    assert_eq!(actions["annotations"]["openWorldHint"], false);
    assert_eq!(actions["outputSchema"]["additionalProperties"], false);

    let targets = find(TOOL_LIST_INTERACTION_TARGETS);
    assert_eq!(targets["inputSchema"]["additionalProperties"], false);
    assert_eq!(targets["annotations"]["readOnlyHint"], true);
    assert_eq!(
        targets["outputSchema"]["properties"]["targets"]["type"],
        "array"
    );

    let click_target = find(TOOL_CLICK_TARGET);
    assert_eq!(
        click_target["inputSchema"]["required"],
        json!(["target_key"])
    );
    assert_eq!(click_target["annotations"]["destructiveHint"], true);
    assert_eq!(click_target["annotations"]["idempotentHint"], false);
    assert_eq!(click_target["annotations"]["openWorldHint"], true);

    let run_scenario = find(TOOL_RUN_SCENARIO);
    assert_eq!(
        run_scenario["inputSchema"]["required"],
        json!(["scenario_key"])
    );
    assert_eq!(run_scenario["annotations"]["destructiveHint"], true);
    assert_eq!(run_scenario["annotations"]["idempotentHint"], false);
    assert_eq!(run_scenario["annotations"]["openWorldHint"], true);
    assert_eq!(
        run_scenario["outputSchema"]["properties"]["scenario"]["type"],
        "object"
    );

    let run_steps = find(TOOL_RUN_STEPS);
    assert_eq!(run_steps["inputSchema"]["required"], json!(["steps"]));
    assert_eq!(run_steps["inputSchema"]["additionalProperties"], false);
    assert!(
        run_steps["inputSchema"]["properties"]
            .get("route")
            .is_none()
    );
    assert_eq!(
        run_steps["inputSchema"]["properties"]["story_key"]["anyOf"][0]["type"],
        "string"
    );
    assert_eq!(
        run_steps["inputSchema"]["properties"]["steps"]["minItems"],
        1
    );
    assert_eq!(
        run_steps["inputSchema"]["properties"]["steps"]["maxItems"],
        gpui_storybook_core::automation::MAX_INTERACTION_STEPS
    );
    let variants = run_steps["inputSchema"]["properties"]["steps"]["items"]["oneOf"]
        .as_array()
        .expect("steps should use oneOf");
    assert_eq!(variants.len(), 11);
    assert!(
        variants
            .iter()
            .all(|variant| variant["additionalProperties"] == false)
    );
    let variant = |kind: &str| {
        variants
            .iter()
            .find(|variant| variant["properties"]["type"]["const"] == kind)
            .unwrap_or_else(|| panic!("step variant `{kind}` should exist"))
    };
    let pointer = &variant("pointer_move")["properties"]["point"]["oneOf"];
    assert_eq!(pointer[0]["properties"]["x"]["minimum"], 0);
    assert_eq!(pointer[0]["properties"]["x"]["maximum"], 1.0);
    assert_eq!(pointer[1]["required"], json!(["space", "x", "y"]));
    assert_eq!(
        variant("text")["properties"]["value"]["maxLength"],
        gpui_storybook_core::automation::MAX_INTERACTION_TEXT_BYTES
    );
    assert_eq!(
        variant("keystrokes")["properties"]["keys"]["maxItems"],
        gpui_storybook_core::automation::MAX_INTERACTION_STEPS
    );
    assert_eq!(
        variant("keystrokes")["properties"]["keys"]["items"]["maxLength"],
        gpui_storybook_core::automation::MAX_INTERACTION_TEXT_BYTES
    );
    assert_eq!(
        variant("pointer_click")["properties"]["click_count"]["maximum"],
        u8::MAX
    );
    assert_eq!(
        variant("click_target")["required"],
        json!(["type", "target_key"])
    );
    assert_eq!(
        variant("click_target")["properties"]["target_key"]["minLength"],
        1
    );
    assert_eq!(
        variant("click_target")["properties"]["click_count"]["maximum"],
        u8::MAX
    );
    assert_eq!(
        variant("wait_frames")["properties"]["count"]["maximum"],
        gpui_storybook_core::automation::MAX_INTERACTION_WAITED_FRAMES
    );
    assert_eq!(run_steps["annotations"]["readOnlyHint"], false);
    assert_eq!(run_steps["annotations"]["destructiveHint"], true);
    assert_eq!(run_steps["annotations"]["idempotentHint"], false);
    assert_eq!(run_steps["annotations"]["openWorldHint"], true);
    assert_eq!(run_steps["outputSchema"]["additionalProperties"], false);
}

#[test]
fn interaction_tool_rejects_unknown_fields_and_bounds_before_host_dispatch() {
    let server = server_with_options(
        StorybookAutomation::with_stories(vec![sample_story()]),
        StorybookMcpServerOptions::default().with_interaction(true),
    )
    .expect("enabled server should build");

    let unknown = serde_json::to_value(server.call_tool(
        TOOL_RUN_STEPS,
        Some(json!({
            "steps": [{ "type": "focus_next", "unknown": true }]
        })),
    ))
    .expect("unknown-field result should serialize");
    assert_eq!(unknown["isError"], true);

    let invalid_point = serde_json::to_value(server.call_tool(
        TOOL_RUN_STEPS,
        Some(json!({
            "steps": [{
                "type": "pointer_move",
                "point": { "space": "normalized", "x": 2.0, "y": 0.5 }
            }]
        })),
    ))
    .expect("invalid-point result should serialize");
    assert_eq!(invalid_point["isError"], true);
    assert_eq!(
        invalid_point["structuredContent"]["error"]["details"][0]["code"],
        "invalid_interaction_step"
    );
    assert_eq!(
        invalid_point["structuredContent"]["error"]["details"][0]["steps_dispatched"],
        0
    );
}

#[test]
fn interaction_environment_requires_the_explicit_enabled_value() {
    let _lock = ENV_LOCK.lock().expect("env lock should not be poisoned");
    let _unset = EnvGuard::remove(&[ALLOW_INTERACTION_ENV_VAR]);
    assert!(!StorybookMcpServerOptions::from_env().interaction_enabled());

    {
        let _disabled = EnvGuard::set(&[(ALLOW_INTERACTION_ENV_VAR, "0")]);
        assert!(!StorybookMcpServerOptions::from_env().interaction_enabled());
    }
    {
        let _enabled = EnvGuard::set(&[(ALLOW_INTERACTION_ENV_VAR, "1")]);
        assert!(StorybookMcpServerOptions::from_env().interaction_enabled());
    }
}

#[test]
fn semantic_automation_errors_have_stable_structured_codes() {
    let cases = [
        (
            StorybookAutomationError::StartupTimedOut { seconds: 30 },
            "startup_timed_out",
        ),
        (
            StorybookAutomationError::InteractionTargetsUnavailable {
                route: "story/section".to_owned(),
            },
            "interaction_targets_unavailable",
        ),
        (
            StorybookAutomationError::InteractionTargetNotFound {
                route: "story".to_owned(),
                key: "execute".to_owned(),
            },
            "interaction_target_not_found",
        ),
        (
            StorybookAutomationError::DuplicateInteractionTarget {
                route: "story".to_owned(),
                key: "execute".to_owned(),
            },
            "duplicate_interaction_target",
        ),
        (
            StorybookAutomationError::SemanticValuesUnavailable {
                route: "story/section".to_owned(),
            },
            "semantic_values_unavailable",
        ),
        (
            StorybookAutomationError::SemanticValueNotFound {
                route: "story".to_owned(),
                key: "response".to_owned(),
            },
            "semantic_value_not_found",
        ),
        (
            StorybookAutomationError::SemanticValueWaitTimedOut {
                route: "story".to_owned(),
                key: "response".to_owned(),
                max_frames: 12,
            },
            "semantic_value_wait_timed_out",
        ),
        (
            StorybookAutomationError::DuplicateSemanticValue {
                route: "story".to_owned(),
                key: "response".to_owned(),
            },
            "duplicate_semantic_value",
        ),
        (
            StorybookAutomationError::InvalidInteractionPostcondition {
                postcondition_index: 1,
                message: "invalid".to_owned(),
            },
            "invalid_interaction_postcondition",
        ),
        (
            StorybookAutomationError::ScenarioNotFound {
                story_key: "story".to_owned(),
                scenario_key: "scenario".to_owned(),
            },
            "scenario_not_found",
        ),
        (
            StorybookAutomationError::DuplicateScenarioKey {
                story_key: "story".to_owned(),
                scenario_key: "scenario".to_owned(),
            },
            "duplicate_scenario_key",
        ),
    ];

    for (error, expected_code) in cases {
        let result = serde_json::to_value(interaction_automation_tool_error(error))
            .expect("target error should serialize");
        assert_eq!(result["isError"], true);
        assert_eq!(
            result["structuredContent"]["error"]["details"][0]["code"],
            expected_code
        );
    }
}

#[test]
fn focused_click_builds_exactly_one_non_retrying_target_step() {
    let request = click_target_request(ClickTargetInput {
        story_key: Some("example-ButtonStory".to_owned()),
        target_key: "execute-request".to_owned(),
    });

    assert_eq!(request.story_key.as_deref(), Some("example-ButtonStory"));
    assert_eq!(request.steps.len(), 1);
    assert_eq!(
        request.steps[0],
        StoryInteractionStep::ClickTarget {
            target_key: "execute-request".to_owned(),
            button: StoryMouseButton::Left,
            click_count: 1,
            modifiers: StoryModifiers::default(),
        }
    );
    assert!(request.capture.is_none());
}

#[test]
fn focused_value_read_selects_one_key_and_reports_missing_values() {
    let snapshot = StorySemanticValuesSnapshot {
        story: sample_story(),
        values: vec![StorySemanticValueSnapshot {
            key: "response".to_owned(),
            label: "Response".to_owned(),
            value: json!({ "status": "success", "value": { "position": 12.5 } }),
        }],
    };

    let output =
        semantic_value_output(snapshot.clone(), "response").expect("focused value should resolve");
    assert_eq!(output.semantic_value.key, "response");
    assert!(semantic_value_matches(
        &output.semantic_value,
        Some("/status"),
        &json!("success")
    ));
    assert!(!semantic_value_matches(
        &output.semantic_value,
        Some("/status"),
        &json!("loading")
    ));

    assert_eq!(
        semantic_value_output(snapshot, "missing"),
        Err(StorybookAutomationError::SemanticValueNotFound {
            route: "example-ButtonStory".to_owned(),
            key: "missing".to_owned(),
        })
    );
}

#[test]
fn value_wait_validation_is_bounded_and_requires_json_pointer_syntax() {
    let valid = WaitForValueInput {
        value_key: "response".to_owned(),
        json_pointer: Some("/status".to_owned()),
        expected: SchemarsValue(json!("success")),
        max_frames: Some(MAX_INTERACTION_WAITED_FRAMES),
    };
    assert!(validate_wait_for_value_input(&valid).is_ok());

    let invalid_pointer = WaitForValueInput {
        json_pointer: Some("status".to_owned()),
        ..valid.clone()
    };
    assert!(validate_wait_for_value_input(&invalid_pointer).is_err());

    let invalid_frames = WaitForValueInput {
        max_frames: Some(0),
        ..valid
    };
    assert!(validate_wait_for_value_input(&invalid_frames).is_err());
}

#[test]
fn typed_tool_calls_reject_bad_arguments_and_return_structured_results() {
    let story = sample_story();
    let server = server(StorybookAutomation::with_stories(vec![story.clone()]))
        .expect("server should build");

    let listed = serde_json::to_value(server.call_tool(TOOL_LIST_STORIES, Some(json!({}))))
        .expect("result should serialize");
    assert_eq!(
        tool_call_structured_content(&listed).expect("structured list result")["stories"][0]["key"],
        story.key
    );

    let found = serde_json::to_value(
        server.call_tool(TOOL_GET_STORY, Some(json!({ "story_key": story.key }))),
    )
    .expect("result should serialize");
    assert_eq!(found["structuredContent"]["story"]["title"], story.title);

    let current = serde_json::to_value(server.call_tool(TOOL_CURRENT_STORY, Some(json!({}))))
        .expect("result should serialize");
    assert_eq!(current["structuredContent"]["story"]["key"], story.key);
    assert_eq!(current["structuredContent"]["revision"], 0);

    let unexpected = serde_json::to_value(
        server.call_tool(TOOL_LIST_STORIES, Some(json!({ "unexpected": true }))),
    )
    .expect("result should serialize");
    assert_eq!(unexpected["isError"], true);
    assert_eq!(
        unexpected["structuredContent"]["error"]["kind"],
        "unknown_field"
    );

    let missing = serde_json::to_value(server.call_tool(TOOL_GET_STORY, Some(json!({}))))
        .expect("result should serialize");
    assert_eq!(
        missing["structuredContent"]["error"]["kind"],
        "missing_field"
    );

    let unknown = serde_json::to_value(server.call_tool(
        TOOL_GET_STORY,
        Some(json!({ "story_key": "missing-story" })),
    ))
    .expect("result should serialize");
    assert_eq!(
        unknown["structuredContent"]["error"]["kind"],
        "invalid_field_value"
    );
}

#[test]
fn schema_constraint_helper_tolerates_unexpected_shapes() {
    let mut properties = serde_json::Map::new();
    set_optional_positive_integer(&mut properties, "width");
    assert!(properties.is_empty());

    properties.insert("width".to_string(), json!({ "type": "integer" }));
    set_optional_positive_integer(&mut properties, "width");
    assert_eq!(properties["width"], json!({ "type": "integer" }));

    properties.insert(
        "width".to_string(),
        json!({ "anyOf": [{ "type": "string" }] }),
    );
    set_optional_positive_integer(&mut properties, "width");
    assert_eq!(properties["width"]["anyOf"][0]["minimum"], Value::Null);
}

#[test]
fn async_tools_return_structured_live_host_errors() {
    let server = server(StorybookAutomation::with_stories(vec![sample_story()]))
        .expect("server should build");

    let open = serde_json::to_value(server.call_tool(
        TOOL_OPEN_STORY,
        Some(json!({ "story_key": "example-ButtonStory" })),
    ))
    .expect("open result should serialize");
    assert_eq!(open["isError"], true);
    assert_eq!(open["structuredContent"]["error"]["kind"], "validation");
    assert_eq!(
        open["structuredContent"]["error"]["details"][0]["code"],
        "no_live_host"
    );

    let capture = serde_json::to_value(server.call_tool(
        TOOL_CAPTURE_CURRENT_STORY,
        Some(json!({ "output_path": "capture.png", "width": 800, "height": 600 })),
    ))
    .expect("capture result should serialize");
    assert_eq!(capture["isError"], true);
    assert_eq!(capture["structuredContent"]["error"]["kind"], "validation");

    let controls = serde_json::to_value(server.call_tool(TOOL_READ_CONTROLS, Some(json!({}))))
        .expect("controls result should serialize");
    assert_eq!(controls["isError"], true);
    assert_eq!(controls["structuredContent"]["error"]["kind"], "validation");

    let semantic_values =
        serde_json::to_value(server.call_tool(TOOL_READ_SEMANTIC_VALUES, Some(json!({}))))
            .expect("semantic values result should serialize");
    assert_eq!(semantic_values["isError"], true);
    assert_eq!(
        semantic_values["structuredContent"]["error"]["details"][0]["code"],
        "no_live_host"
    );

    let invalid = serde_json::to_value(server.call_tool(
        TOOL_SET_CONTROL,
        Some(json!({ "control_key": "disabled", "value": true })),
    ))
    .expect("invalid control result should serialize");
    assert_eq!(invalid["isError"], true);
    assert_eq!(
        invalid["structuredContent"]["error"]["kind"],
        "decode_field"
    );
}
