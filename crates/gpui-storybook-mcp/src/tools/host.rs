//! Capability-gated native host discovery, actions, and display observation.

use super::*;
use gpui_storybook_automation::wire::{
    HostAction, HostActionDescriptor, HostCaptureScope, HostCaptureSnapshot, HostDescriptor,
};

pub const TOOL_GET_HOST: &str = "storybook_get_host";
pub const TOOL_LIST_HOST_ACTIONS: &str = "storybook_list_host_actions";
pub const TOOL_DISPATCH_HOST_ACTION: &str = "storybook_dispatch_host_action";
pub const TOOL_CAPTURE_HOST: &str = "storybook_capture_host";

#[derive(Clone, Debug, component_shape_mcp::McpToolInput)]
struct HostActionInput {
    /// Typed action discovered from the native host catalog.
    action: SchemarsValue<HostAction>,
}
#[derive(Clone, Debug, component_shape_mcp::McpToolInput)]
struct HostCaptureInput {
    /// Full display or observed GPUI surface; includes visible system UI for display.
    scope: SchemarsValue<HostCaptureScope>,
    /// Computer-owned PNG destination.
    output_path: String,
}
#[derive(schemars::JsonSchema, serde::Serialize)]
#[schemars(deny_unknown_fields)]
struct HostActionsOutput {
    actions: Vec<HostActionDescriptor>,
}

pub(super) fn register_host_tools(
    tools: &mut McpToolRegistry,
    backend: SharedAutomationBackend,
    options: StorybookMcpServerOptions,
) -> Result<(), McpToolError> {
    let capabilities = backend.capabilities();
    if capabilities.contains(AutomationCapability::HostDiscovery) {
        let definition = tool::<EmptyInput>(
            TOOL_GET_HOST,
            "Get Host",
            "Read the negotiated session, native and GPUI route agreement, observed surface geometry, and capabilities.",
            serialize_schema::<HostDescriptor>(),
            ToolHints::read_only(),
        )?;
        tools.add_typed_tool_async(definition, {
            let backend = backend.clone();
            move |_| {
                let backend = backend.clone();
                async move {
                    match backend.host().await {
                        Ok(host) => tool_structured_result(json!(host)),
                        Err(error) => automation_tool_error(error),
                    }
                }
            }
        })?;
    }
    if capabilities.contains(AutomationCapability::HostActions) {
        let definition = tool::<EmptyInput>(
            TOOL_LIST_HOST_ACTIONS,
            "List Host Actions",
            "Discover typed public actions owned by the native shell.",
            serialize_schema::<HostActionsOutput>(),
            ToolHints::read_only(),
        )?;
        tools.add_typed_tool_async(definition, {
            let backend = backend.clone();
            move |_| {
                let backend = backend.clone();
                async move {
                    match backend.list_host_actions().await {
                        Ok(actions) => tool_structured_result(json!(HostActionsOutput { actions })),
                        Err(error) => automation_tool_error(error),
                    }
                }
            }
        })?;
        if options.interaction_enabled() {
            let definition = tool::<HostActionInput>(
                TOOL_DISPATCH_HOST_ACTION,
                "Dispatch Host Action",
                "Execute one typed native action under the device mutation lease and await native and GPUI agreement.",
                serialize_schema::<HostDescriptor>(),
                ToolHints::interaction(),
            )?;
            tools.add_typed_tool_async(definition, {
                let backend = backend.clone();
                move |input| {
                    let backend = backend.clone();
                    async move {
                        match backend
                            .dispatch_host_action(input.action.into_inner())
                            .await
                        {
                            Ok(host) => tool_structured_result(json!(host)),
                            Err(error) => automation_tool_error(error),
                        }
                    }
                }
            })?;
        }
    }
    if capabilities.contains(AutomationCapability::DisplayCapture) {
        let definition = tool::<HostCaptureInput>(
            TOOL_CAPTURE_HOST,
            "Capture Host",
            "Observe the device compositor as a full-display or GPUI-surface PNG and validate session, route revision, and geometry across capture. The computer owns the path.",
            serialize_schema::<HostCaptureSnapshot>(),
            ToolHints::mutation(false, false),
        )?;
        tools.add_typed_tool_async(definition, move |input| {
            let backend = backend.clone();
            async move {
                match backend
                    .capture_host(input.scope.into_inner(), input.output_path.into())
                    .await
                {
                    Ok(capture) => tool_structured_result(json!(capture)),
                    Err(error) => automation_tool_error(error),
                }
            }
        })?;
    }
    Ok(())
}
