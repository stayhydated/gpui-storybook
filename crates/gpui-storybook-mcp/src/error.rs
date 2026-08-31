//! Errors returned while configuring or running Storybook MCP automation.

use component_shape_mcp::McpToolError;
use frame_capture::{CaptureEnvError, CaptureLaunchEnvError};
use gpui_storybook_core::automation::StorybookAutomationError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum StorybookMcpError {
    #[error("{0}")]
    Tool(#[from] McpToolError),
    #[error("{0}")]
    CaptureEnv(#[from] CaptureEnvError),
    #[error("{0}")]
    CaptureLaunchEnv(#[from] CaptureLaunchEnvError),
    #[error("{0}")]
    Automation(#[from] StorybookAutomationError),
    #[error("{0}")]
    Io(#[from] std::io::Error),
    #[error("invalid default story key `{key}`: {message}")]
    InvalidDefaultStoryKey { key: String, message: String },
    #[error("capture session was requested before any stories were registered")]
    NoStoriesRegistered,
    #[error("capture session timed out after {seconds} seconds")]
    CaptureSessionTimedOut { seconds: u64 },
}
