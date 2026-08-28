//! Registration and handlers for the Storybook MCP tool catalog.

use super::*;

/// Build the Storybook MCP tool registry.
pub fn tool_registry(
    automation: SharedStorybookAutomation,
) -> Result<McpToolRegistry, McpToolError> {
    tool_registry_with_options(automation, StorybookMcpServerOptions::from_env())
}

/// Build the Storybook tool registry with explicit runtime capabilities.
pub fn tool_registry_with_options(
    automation: SharedStorybookAutomation,
    options: StorybookMcpServerOptions,
) -> Result<McpToolRegistry, McpToolError> {
    let mut tools = McpToolRegistry::new();
    register_tools_with_options(&mut tools, automation, options)?;
    Ok(tools)
}

pub fn register_tools(
    tools: &mut McpToolRegistry,
    automation: SharedStorybookAutomation,
) -> Result<(), McpToolError> {
    register_tools_with_options(tools, automation, StorybookMcpServerOptions::from_env())
}

/// Register Storybook tools with explicit runtime capabilities.
pub fn register_tools_with_options(
    tools: &mut McpToolRegistry,
    automation: SharedStorybookAutomation,
    options: StorybookMcpServerOptions,
) -> Result<(), McpToolError> {
    tools.add_typed_tool_async(
        tool::<EmptyInput>(
            TOOL_LIST_STORIES,
            "List Stories",
            "List the registered stories and their stable capture route metadata.",
            list_stories_output_schema(),
            ToolHints::read_only(),
        )?,
        {
            let automation = automation.clone();
            move |_input| {
                let automation = automation.clone();
                async move {
                    if let Err(error) = await_automation_startup(&automation).await {
                        return automation_tool_error(error);
                    }
                    tool_structured_result(json!(ListStoriesOutput {
                        stories: automation.stories(),
                    }))
                }
            }
        },
    )?;

    tools.add_typed_tool_async(
        tool::<ListScenariosInput>(
            TOOL_LIST_SCENARIOS,
            "List Scenarios",
            "List the stable, story-owned interaction scenarios declared by the current story or a supplied story route.",
            list_scenarios_output_schema(),
            ToolHints::read_only(),
        )?,
        {
            let automation = automation.clone();
            move |input| {
                let automation = automation.clone();
                async move {
                    if let Err(error) = await_automation_startup(&automation).await {
                        return automation_tool_error(error);
                    }
                    let result = input.story_key.as_deref().map_or_else(
                        || automation.list_scenarios(),
                        |story_key| automation.list_scenarios_for(story_key),
                    );
                    match result {
                        Ok(snapshot) => tool_structured_result(json!(ListScenariosOutput {
                            story: snapshot.story,
                            scenarios: snapshot.scenarios,
                        })),
                        Err(error) => automation_tool_error(error),
                    }
                }
            }
        },
    )?;

    tools.add_typed_tool_async(
        tool::<StoryKeyInput>(
            TOOL_GET_STORY,
            "Get Story",
            "Inspect one registered story or sub-story route by its stable key.",
            get_story_output_schema(),
            ToolHints::read_only(),
        )?,
        {
            let automation = automation.clone();
            move |input| {
                let automation = automation.clone();
                async move {
                    if let Err(error) = await_automation_startup(&automation).await {
                        return automation_tool_error(error);
                    }
                    match automation.get_story(&input.story_key) {
                        Ok(story) => tool_structured_result(json!(StoryOutput { story })),
                        Err(error) => automation_tool_error(error),
                    }
                }
            }
        },
    )?;

    tools.add_typed_tool_async(
        tool::<EmptyInput>(
            TOOL_CURRENT_STORY,
            "Current Story",
            "Inspect the story currently displayed by the live storybook window.",
            current_story_output_schema(),
            ToolHints::read_only(),
        )?,
        {
            let automation = automation.clone();
            move |_input| {
                let automation = automation.clone();
                async move {
                    if let Err(error) = await_automation_startup(&automation).await {
                        return automation_tool_error(error);
                    }
                    tool_structured_result(json!(automation.current_story()))
                }
            }
        },
    )?;

    tools.add_typed_tool_async(
        tool::<StoryKeyInput>(
            TOOL_OPEN_STORY,
            "Open Story",
            "Open one registered story or sub-story route in the live storybook window.",
            current_story_output_schema(),
            ToolHints::mutation(true, false),
        )?,
        {
            let automation = automation.clone();
            move |input| {
                let automation = automation.clone();
                async move {
                    if let Err(error) = await_automation_startup(&automation).await {
                        return automation_tool_error(error);
                    }
                    match automation.open_story(input.story_key).await {
                        Ok(snapshot) => tool_structured_result(json!(snapshot)),
                        Err(error) => automation_tool_error(error),
                    }
                }
            }
        },
    )?;

    tools.add_typed_tool_async(
        tool::<EmptyInput>(
            TOOL_READ_SEMANTIC_VALUES,
            "Read Semantic Values",
            "Read stable machine-readable values rendered from application state by the selected story or substory route.",
            semantic_values_output_schema(),
            ToolHints::read_only(),
        )?,
        {
            let automation = automation.clone();
            move |_input| {
                let automation = automation.clone();
                async move {
                    if let Err(error) = await_automation_startup(&automation).await {
                        return automation_tool_error(error);
                    }
                    match automation.read_semantic_values().await {
                        Ok(snapshot) => tool_structured_result(json!(snapshot)),
                        Err(error) => automation_tool_error(error),
                    }
                }
            }
        },
    )?;

    tools.add_typed_tool_async(
        tool::<ReadValueInput>(
            TOOL_READ_VALUE,
            "Read Value",
            "Read one stable machine-readable value rendered by the selected story or substory route.",
            semantic_value_output_schema(),
            ToolHints::read_only(),
        )?,
        {
            let automation = automation.clone();
            move |input| {
                let automation = automation.clone();
                async move {
                    if let Err(error) = await_automation_startup(&automation).await {
                        return automation_tool_error(error);
                    }
                    match read_semantic_value(&automation, &input.value_key).await {
                        Ok(output) => tool_structured_result(json!(output)),
                        Err(error) => automation_tool_error(error),
                    }
                }
            }
        },
    )?;

    tools.add_typed_tool_async(wait_for_value_tool()?, {
        let automation = automation.clone();
        move |input| {
            let automation = automation.clone();
            async move {
                if let Err(error) = validate_wait_for_value_input(&input) {
                    return tool_error_result_for(error);
                }
                if let Err(error) = await_automation_startup(&automation).await {
                    return automation_tool_error(error);
                }
                match wait_for_semantic_value(&automation, input).await {
                    Ok(output) => tool_structured_result(json!(output)),
                    Err(error) => automation_tool_error(error),
                }
            }
        }
    })?;

    if options.interaction_enabled() {
        tools.add_typed_tool_async(
            tool::<EmptyInput>(
                TOOL_LIST_ACTIONS,
                "List Actions",
                "List non-internal GPUI actions registered by this launched application. Rediscover actions after every launch because action names are runtime registrations.",
                list_actions_output_schema(),
                ToolHints::read_only(),
            )?,
            {
                let automation = automation.clone();
                move |_input| {
                    let automation = automation.clone();
                    async move {
                        if let Err(error) = await_automation_startup(&automation).await {
                            return automation_tool_error(error);
                        }
                        match automation.list_actions().await {
                            Ok(actions) => {
                                tool_structured_result(json!(ListActionsOutput { actions }))
                            },
                            Err(error) => automation_tool_error(error),
                        }
                    }
                }
            },
        )?;

        tools.add_typed_tool_async(
            tool::<EmptyInput>(
                TOOL_LIST_INTERACTION_TARGETS,
                "List Interaction Targets",
                "List stable semantic targets and live route-relative bounds rendered by the selected story or substory route.",
                interaction_targets_output_schema(),
                ToolHints::read_only(),
            )?,
            {
                let automation = automation.clone();
                move |_input| {
                    let automation = automation.clone();
                    async move {
                        if let Err(error) = await_automation_startup(&automation).await {
                            return automation_tool_error(error);
                        }
                        match automation.list_interaction_targets().await {
                            Ok(snapshot) => tool_structured_result(json!(snapshot)),
                            Err(error) => interaction_automation_tool_error(error),
                        }
                    }
                }
            },
        )?;

        tools.add_typed_tool_async(
            tool::<ClickTargetInput>(
                TOOL_CLICK_TARGET,
                "Click Target",
                "Click one stable semantic target once, optionally opening its story route first. This operation is destructive, non-idempotent, and is never retried automatically.",
                interaction_output_schema(),
                ToolHints::interaction(),
            )?,
            {
                let automation = automation.clone();
                move |input| {
                    let automation = automation.clone();
                    async move {
                        if let Err(error) = await_automation_startup(&automation).await {
                            return automation_tool_error(error);
                        }
                        let request = click_target_request(input);
                        match automation.run_steps(request).await {
                            Ok(snapshot) => tool_structured_result(json!(snapshot)),
                            Err(error) => interaction_automation_tool_error(error),
                        }
                    }
                }
            },
        )?;

        tools.add_typed_tool_async(
            tool::<RunScenarioInput>(
                TOOL_RUN_SCENARIO,
                "Run Scenario",
                "Run one declared story-owned interaction scenario from a freshly recreated story instance. The ordered batch is exclusive, destructive, and is never resumed or retried after a partial dispatch.",
                scenario_run_output_schema(),
                ToolHints::interaction(),
            )?,
            {
                let automation = automation.clone();
                move |input| {
                    let automation = automation.clone();
                    async move {
                        if let Err(error) = await_automation_startup(&automation).await {
                            return automation_tool_error(error);
                        }
                        match automation
                            .run_scenario(input.story_key, input.scenario_key)
                            .await
                        {
                            Ok(snapshot) => tool_structured_result(json!(snapshot)),
                            Err(error) => interaction_automation_tool_error(error),
                        }
                    }
                }
            },
        )?;

        tools.add_typed_tool_async(interaction_tool()?, {
            let automation = automation.clone();
            move |input| {
                let automation = automation.clone();
                async move {
                    if let Err(error) = await_automation_startup(&automation).await {
                        return automation_tool_error(error);
                    }
                    let request = decode_interaction_request(input);
                    match automation.run_steps(request).await {
                        Ok(snapshot) => tool_structured_result(json!(snapshot)),
                        Err(error) => interaction_automation_tool_error(error),
                    }
                }
            }
        })?;
    }

    tools.add_typed_tool_async(
        tool::<EmptyInput>(
            TOOL_READ_CONTROLS,
            "Read Controls",
            "Read control metadata and values from the active concrete story instance.",
            story_controls_output_schema(),
            ToolHints::read_only(),
        )?,
        {
            let automation = automation.clone();
            move |_input| {
                let automation = automation.clone();
                async move {
                    if let Err(error) = await_automation_startup(&automation).await {
                        return automation_tool_error(error);
                    }
                    match automation.read_controls().await {
                        Ok(snapshot) => tool_structured_result(json!(snapshot)),
                        Err(error) => automation_tool_error(error),
                    }
                }
            }
        },
    )?;

    tools.add_typed_tool_async(
        tool::<SetControlInput>(
            TOOL_SET_CONTROL,
            "Set Control",
            "Set one control on the active concrete story instance and return all current values.",
            story_controls_output_schema(),
            ToolHints::mutation(true, false),
        )?,
        {
            let automation = automation.clone();
            move |input| {
                let automation = automation.clone();
                async move {
                    if let Err(error) = await_automation_startup(&automation).await {
                        return automation_tool_error(error);
                    }
                    match automation
                        .set_control(input.control_key, input.value.into_inner())
                        .await
                    {
                        Ok(snapshot) => tool_structured_result(json!(snapshot)),
                        Err(error) => automation_tool_error(error),
                    }
                }
            }
        },
    )?;

    tools.add_typed_tool_async(
        tool::<ResetControlInput>(
            TOOL_RESET_CONTROL,
            "Reset Control",
            "Reset one active-story control, or every control when no control_key is supplied.",
            story_controls_output_schema(),
            ToolHints::mutation(true, false),
        )?,
        {
            let automation = automation.clone();
            move |input| {
                let automation = automation.clone();
                async move {
                    if let Err(error) = await_automation_startup(&automation).await {
                        return automation_tool_error(error);
                    }
                    match automation.reset_control(input.control_key).await {
                        Ok(snapshot) => tool_structured_result(json!(snapshot)),
                        Err(error) => automation_tool_error(error),
                    }
                }
            }
        },
    )?;

    tools.add_typed_tool_async(
        capture_tool::<CaptureCurrentStoryInput>(
            TOOL_CAPTURE_CURRENT_STORY,
            "Capture Current Story",
            "Capture the current story view to a PNG, excluding storybook chrome.",
            capture_story_output_schema(),
            ToolHints::mutation(false, true),
            false,
        )?,
        move |input| {
            let automation = automation.clone();
            async move {
                if let Err(error) = await_automation_startup(&automation).await {
                    return automation_tool_error(error);
                }
                let request = StoryScreenshotRequest {
                    output_path: input.output_path,
                    width: input.width,
                    height: input.height,
                    viewport: input.viewport.map(SchemarsValue::into_inner),
                    controls: decode_control_map(input.controls),
                    quit_after_capture: false,
                };

                match automation.capture_current_story(request).await {
                    Ok(snapshot) => tool_structured_result(json!(snapshot)),
                    Err(error) => automation_tool_error(error),
                }
            }
        },
    )?;

    tools.add_typed_tool(
        capture_tool::<CaptureLaunchEnvInput>(
            TOOL_CAPTURE_LAUNCH_ENV,
            "Capture Launch Env",
            "Build frame-capture environment variables and a platform launch command for a story route.",
            capture_launch_env_output_schema(),
            ToolHints::read_only(),
            true,
        )?,
        move |input| match build_capture_launch_env(input) {
            Ok(env) => tool_structured_result(json!(env)),
            Err(error) => tool_error_result_for(McpToolError::invalid_field_value(
                "capture",
                error.to_string(),
            )),
        },
    )?;

    Ok(())
}
