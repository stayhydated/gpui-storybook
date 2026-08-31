//! MCP tools for driving a live `gpui-storybook` window.
//!
//! This crate supports Linux and macOS. Windows and other targets produce a
//! compile-time error. Linux launch commands use the headless Sway wrapper;
//! macOS launch commands use Cargo directly and capture through GPUI's native
//! image renderer.
//!
//! Tools can navigate stable routes, read/set/reset the selected story's typed
//! controls, read or wait for route-local structured application values, apply
//! a serialized control map before capture, and use named or explicit viewport
//! dimensions. Facade-created controllers wait for the standard gallery or dock
//! to publish and attach before handling the first tool call.
//! These operations reuse the core `ControlSpec` and `ControlValue` contracts.
//! Generic actions, focused semantic target clicks, keyboard, pointer, frame
//! waits, and atomic post-interaction capture are registered only when
//! interaction automation is explicitly enabled with
//! [`StorybookMcpServerOptions`] or [`ALLOW_INTERACTION_ENV_VAR`]. The
//! environment value must equal `1`.
//! Interaction is destructive, non-idempotent, and open-world: the capability
//! authorizes event dispatch but does not constrain downstream story behavior.
//!
//! The facade recognizes the same frame-capture route/path variables emitted by
//! this crate. Capture launches use disabled storage plus deterministic light,
//! `Default Light`, and fallback-language overrides. Stdio-only launches use
//! the same presentation with temporary storage, preventing automation from
//! overwriting persistent interactive choices. On Linux, generated launch
//! commands run the Wayland application through Sway's wlroots headless backend
//! so GPUI receives compositor-driven frame callbacks without a physical display.
//! MCP servers retain the shared automation registry for the complete live-host
//! lifetime; completing an automation call never requests application
//! shutdown.

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
compile_error!(
    "gpui-storybook-mcp supports Linux and macOS; Windows and other targets are unsupported"
);

pub use gpui_storybook_core::automation::{
    DEFAULT_INTERACTION_POSTCONDITION_FRAMES, DEFAULT_STORY_CAPTURE_HEIGHT,
    DEFAULT_STORY_CAPTURE_WIDTH, MAX_INTERACTION_POSTCONDITIONS, MAX_INTERACTION_STEPS,
    MAX_INTERACTION_TEXT_BYTES, MAX_INTERACTION_WAITED_FRAMES, SharedStoryCaptureController,
    SharedStoryController, SharedStorybookAutomation, StoryActionSnapshot, StoryCaptureSnapshot,
    StoryControlsSnapshot, StoryCurrentSnapshot, StoryDefaultSize, StoryInteractionCaptureRequest,
    StoryInteractionDispatch, StoryInteractionObservation, StoryInteractionPostcondition,
    StoryInteractionPostconditionSnapshot, StoryInteractionRequest, StoryInteractionSnapshot,
    StoryInteractionStep, StoryInteractionTargetBounds, StoryInteractionTargetSnapshot,
    StoryInteractionTargetsSnapshot, StoryModifier, StoryModifiers, StoryMouseButton, StoryPoint,
    StoryPointSpace, StoryScenarioRunSnapshot, StoryScenarioSnapshot, StoryScenarioStep,
    StoryScenariosSnapshot, StoryScreenshotRequest, StorySemanticValueSnapshot,
    StorySemanticValuesSnapshot, StorySnapshot, StorybookAutomation, StorybookAutomationError,
};
pub use gpui_storybook_core::controls::{
    ControlBounds, ControlColor, ControlKind, ControlSnapshot, ControlSpec, ControlValue,
};
pub use gpui_storybook_core::presentation::StoryViewportPreset;

pub use gpui_storybook_core::automation;

mod error;
mod server;
mod tools;

pub mod capture;

pub use capture::{
    CaptureLaunchEnv, StorybookCaptureConfig, StorybookCaptureSession, capture_catalog,
    capture_requested, read_capture_session, start_capture_session, start_capture_session_from_env,
};
pub use error::StorybookMcpError;
pub use server::{
    StorybookMcpServerOptions, StorybookStdioCompletion, server, server_with_options, start_stdio,
    stdio_requested,
};
pub use tools::{
    TOOL_CAPTURE_CURRENT_STORY, TOOL_CAPTURE_LAUNCH_ENV, TOOL_CLICK_TARGET, TOOL_CURRENT_STORY,
    TOOL_GET_STORY, TOOL_LIST_ACTIONS, TOOL_LIST_INTERACTION_TARGETS, TOOL_LIST_SCENARIOS,
    TOOL_LIST_STORIES, TOOL_OPEN_STORY, TOOL_READ_CONTROLS, TOOL_READ_SEMANTIC_VALUES,
    TOOL_READ_VALUE, TOOL_RESET_CONTROL, TOOL_RUN_SCENARIO, TOOL_RUN_STEPS, TOOL_SET_CONTROL,
    TOOL_WAIT_FOR_VALUE, register_tools, register_tools_with_options, tool_registry,
    tool_registry_with_options,
};

pub const STDIO_ENV_VAR: &str = "GPUI_STORYBOOK_MCP_STDIO";
pub const ALLOW_INTERACTION_ENV_VAR: &str = "GPUI_STORYBOOK_MCP_ALLOW_INTERACTION";

pub mod prelude {
    pub use super::{
        ALLOW_INTERACTION_ENV_VAR, CaptureLaunchEnv, DEFAULT_INTERACTION_POSTCONDITION_FRAMES,
        MAX_INTERACTION_POSTCONDITIONS, MAX_INTERACTION_STEPS, MAX_INTERACTION_TEXT_BYTES,
        MAX_INTERACTION_WAITED_FRAMES, SharedStoryCaptureController, SharedStoryController,
        SharedStorybookAutomation, StoryActionSnapshot, StoryCaptureSnapshot, StoryCurrentSnapshot,
        StoryDefaultSize, StoryInteractionCaptureRequest, StoryInteractionDispatch,
        StoryInteractionObservation, StoryInteractionPostcondition,
        StoryInteractionPostconditionSnapshot, StoryInteractionRequest, StoryInteractionSnapshot,
        StoryInteractionStep, StoryInteractionTargetBounds, StoryInteractionTargetSnapshot,
        StoryInteractionTargetsSnapshot, StoryModifier, StoryModifiers, StoryMouseButton,
        StoryPoint, StoryPointSpace, StoryScenarioRunSnapshot, StoryScenarioSnapshot,
        StoryScenarioStep, StoryScenariosSnapshot, StoryScreenshotRequest,
        StorySemanticValueSnapshot, StorySemanticValuesSnapshot, StorySnapshot,
        StorybookAutomation, StorybookAutomationError, StorybookCaptureConfig,
        StorybookCaptureSession, StorybookMcpServerOptions, StorybookStdioCompletion,
        capture_catalog, read_capture_session, server, server_with_options, start_capture_session,
        start_capture_session_from_env, start_stdio, stdio_requested,
    };
}

#[cfg(test)]
mod tests;
