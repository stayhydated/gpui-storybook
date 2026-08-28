//! MCP server construction and stdio lifecycle management.

use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
    thread,
};

use component_shape_mcp::{McpServer, McpToolError, ServeStdioResult};
use gpui_storybook_core::automation::SharedStorybookAutomation;
use tokio::sync::oneshot;

use crate::{ALLOW_INTERACTION_ENV_VAR, STDIO_ENV_VAR, tools::tool_registry_with_options};

/// Runtime capabilities exposed by a Storybook MCP server.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct StorybookMcpServerOptions {
    allow_interaction: bool,
}

impl StorybookMcpServerOptions {
    /// Build options from the process environment used by stdio launches.
    pub fn from_env() -> Self {
        Self {
            allow_interaction: std::env::var(ALLOW_INTERACTION_ENV_VAR)
                .is_ok_and(|value| value == "1"),
        }
    }

    /// Enable or disable generic in-process interaction tools.
    pub const fn with_interaction(mut self, allow_interaction: bool) -> Self {
        self.allow_interaction = allow_interaction;
        self
    }

    /// Whether generic in-process interaction tools are enabled.
    pub const fn interaction_enabled(self) -> bool {
        self.allow_interaction
    }
}

pub fn stdio_requested() -> bool {
    std::env::var(STDIO_ENV_VAR).is_ok_and(|value| value == "1")
}

/// Awaitable completion of the MCP stdio server thread.
///
/// The future resolves when stdin reaches EOF or the server fails. Facade
/// initialization uses this signal to quit the GPUI application cleanly.
pub struct StorybookStdioCompletion {
    receiver: oneshot::Receiver<ServeStdioResult>,
}

impl Future for StorybookStdioCompletion {
    type Output = ServeStdioResult;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match Pin::new(&mut self.receiver).poll(cx) {
            Poll::Ready(Ok(result)) => Poll::Ready(result),
            Poll::Ready(Err(error)) => Poll::Ready(Err(Box::new(std::io::Error::other(format!(
                "gpui-storybook MCP stdio thread ended without a result: {error}"
            ))))),
            Poll::Pending => Poll::Pending,
        }
    }
}

pub fn start_stdio(
    automation: SharedStorybookAutomation,
) -> std::io::Result<StorybookStdioCompletion> {
    let (result_tx, receiver) = oneshot::channel();
    thread::Builder::new()
        .name("gpui-storybook-mcp-stdio".to_string())
        .spawn(move || {
            let result = match server(automation) {
                Ok(server) => server.serve_stdio_blocking(),
                Err(error) => Err(Box::new(error) as Box<dyn std::error::Error + Send + Sync>),
            };
            let _ = result_tx.send(result);
        })?;
    Ok(StorybookStdioCompletion { receiver })
}

pub fn server(automation: SharedStorybookAutomation) -> Result<McpServer, McpToolError> {
    server_with_options(automation, StorybookMcpServerOptions::from_env())
}

/// Build an MCP server with explicit runtime capabilities.
pub fn server_with_options(
    automation: SharedStorybookAutomation,
    options: StorybookMcpServerOptions,
) -> Result<McpServer, McpToolError> {
    Ok(McpServer::from_tool_registry(
        "gpui-storybook",
        env!("CARGO_PKG_VERSION"),
        tool_registry_with_options(automation, options)?,
    ))
}
