//! MCP tools for driving a live `gpui-storybook` window.

use component_shape_mcp::{
    McpSchema, McpServer, McpToolCall, McpToolError, ServeStdioResult, ToolCallResult,
    tool_definition, tool_error_result, tool_structured_result,
};
use frame_capture::{
    CaptureConfig, CaptureEnv, CaptureEnvError, CaptureLaunchEnv as FrameCaptureLaunchEnv,
    CaptureLaunchEnvError, CaptureRouteId, PixelSize,
};
pub use gpui_storybook_core::automation::{
    DEFAULT_STORY_CAPTURE_HEIGHT, DEFAULT_STORY_CAPTURE_WIDTH, SharedStoryCaptureController,
    SharedStoryController, SharedStorybookAutomation, StoryCaptureSnapshot, StoryCurrentSnapshot,
    StoryDefaultSize, StoryScreenshotRequest, StorySnapshot, StorybookAutomation,
    StorybookAutomationError,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{collections::BTreeMap, path::PathBuf, thread, time::Duration};
use thiserror::Error;

pub use gpui_storybook_core::automation;

pub const STDIO_ENV_VAR: &str = "GPUI_STORYBOOK_MCP_STDIO";

pub const TOOL_LIST_STORIES: &str = "storybook_list_stories";
pub const TOOL_GET_STORY: &str = "storybook_get_story";
pub const TOOL_CURRENT_STORY: &str = "storybook_current_story";
pub const TOOL_OPEN_STORY: &str = "storybook_open_story";
pub const TOOL_CAPTURE_CURRENT_STORY: &str = "storybook_capture_current_story";
pub const TOOL_CAPTURE_LAUNCH_ENV: &str = "storybook_capture_launch_env";

const CAPTURE_SESSION_TIMEOUT_SECS: u64 = 30;
const CAPTURE_ENV_PREFIX: &str = "WGPU";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct StorybookCaptureConfig {
    pub path: PathBuf,
    pub frame: u32,
    pub size: StoryDefaultSize,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct StorybookCaptureSession {
    pub story_key: String,
    pub capture: Option<StorybookCaptureConfig>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CaptureLaunchEnv {
    pub env: BTreeMap<String, String>,
    pub cargo_args: Vec<String>,
    pub command: Vec<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
struct EmptyInput {}

#[derive(Clone, Debug, Deserialize)]
struct StoryKeyInput {
    key: String,
}

#[derive(Clone, Debug, Default, Deserialize)]
struct CaptureCurrentStoryInput {
    output_path: Option<PathBuf>,
    width: Option<u32>,
    height: Option<u32>,
}

#[derive(Clone, Debug, Deserialize)]
struct CaptureLaunchEnvInput {
    key: String,
    output_path: Option<PathBuf>,
    frame: Option<u32>,
    width: Option<u32>,
    height: Option<u32>,
    package: Option<String>,
    bin: Option<String>,
    features: Option<Vec<String>>,
    stdio: Option<bool>,
}

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

pub fn stdio_requested() -> bool {
    std::env::var(STDIO_ENV_VAR).is_ok_and(|value| value == "1")
}

pub fn start_stdio(
    automation: SharedStorybookAutomation,
) -> std::io::Result<thread::JoinHandle<ServeStdioResult>> {
    thread::Builder::new()
        .name("gpui-storybook-mcp-stdio".to_string())
        .spawn(move || match server(automation) {
            Ok(server) => server.serve_stdio_blocking(),
            Err(error) => Err(Box::new(error) as Box<dyn std::error::Error + Send + Sync>),
        })
}

pub fn start_capture_session_from_env(
    automation: SharedStorybookAutomation,
) -> Result<Option<thread::JoinHandle<Result<(), StorybookMcpError>>>, StorybookMcpError> {
    let env = storybook_capture_env();
    if std::env::var_os(env.route_var()).is_none() && std::env::var_os(env.path_var()).is_none() {
        return Ok(None);
    }

    let default_story_key = automation.stories().first().map(|story| story.key.clone());

    if let Some(default_story_key) = default_story_key {
        let session = read_capture_session(default_story_key)?;
        start_capture_session(automation, session, true).map(Some)
    } else {
        start_capture_session_from_env_when_ready(automation).map(Some)
    }
}

pub fn start_capture_session(
    automation: SharedStorybookAutomation,
    session: StorybookCaptureSession,
    exit_after_capture: bool,
) -> Result<thread::JoinHandle<Result<(), StorybookMcpError>>, StorybookMcpError> {
    thread::Builder::new()
        .name("gpui-storybook-capture-session".to_string())
        .spawn(move || {
            let should_exit = exit_after_capture && session.capture.is_some();
            let runtime = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()?;

            let result =
                runtime.block_on(run_capture_session(automation, session, exit_after_capture));

            if should_exit && let Err(error) = &result {
                eprintln!("gpui-storybook capture session failed: {error}");
            }

            result
        })
        .map_err(StorybookMcpError::Io)
}

fn start_capture_session_from_env_when_ready(
    automation: SharedStorybookAutomation,
) -> Result<thread::JoinHandle<Result<(), StorybookMcpError>>, StorybookMcpError> {
    thread::Builder::new()
        .name("gpui-storybook-capture-session".to_string())
        .spawn(move || {
            let runtime = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()?;

            runtime.block_on(async move {
                let default_story_key = wait_for_default_story_key(automation.clone()).await?;
                let session = read_capture_session(default_story_key)?;
                let should_exit = session.capture.is_some();
                let result = run_capture_session(automation, session, true).await;
                if should_exit && let Err(error) = &result {
                    eprintln!("gpui-storybook capture session failed: {error}");
                }
                result
            })
        })
        .map_err(StorybookMcpError::Io)
}

async fn wait_for_default_story_key(
    automation: SharedStorybookAutomation,
) -> Result<String, StorybookMcpError> {
    tokio::time::timeout(
        Duration::from_secs(CAPTURE_SESSION_TIMEOUT_SECS),
        async move {
            loop {
                if let Some(default_story_key) =
                    automation.stories().first().map(|story| story.key.clone())
                {
                    return default_story_key;
                }

                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        },
    )
    .await
    .map_err(|_| StorybookMcpError::CaptureSessionTimedOut {
        seconds: CAPTURE_SESSION_TIMEOUT_SECS,
    })
}

async fn run_capture_session(
    automation: SharedStorybookAutomation,
    session: StorybookCaptureSession,
    exit_after_capture: bool,
) -> Result<(), StorybookMcpError> {
    tokio::time::timeout(
        Duration::from_secs(CAPTURE_SESSION_TIMEOUT_SECS),
        async move {
            automation.open_story(session.story_key).await?;

            if let Some(capture) = session.capture {
                tokio::time::sleep(Duration::from_millis(
                    u64::from(capture.frame.saturating_sub(1)) * 16,
                ))
                .await;

                automation
                    .capture_current_story(StoryScreenshotRequest {
                        output_path: Some(capture.path),
                        width: Some(capture.size.width),
                        height: Some(capture.size.height),
                        quit_after_capture: exit_after_capture,
                    })
                    .await?;
            }

            Ok(())
        },
    )
    .await
    .map_err(|_| StorybookMcpError::CaptureSessionTimedOut {
        seconds: CAPTURE_SESSION_TIMEOUT_SECS,
    })?
}

pub fn server(automation: SharedStorybookAutomation) -> Result<McpServer, McpToolError> {
    let mut server = McpServer::new("gpui-storybook", env!("CARGO_PKG_VERSION"));
    register_tools(&mut server, automation)?;
    Ok(server)
}

pub fn register_tools(
    server: &mut McpServer,
    automation: SharedStorybookAutomation,
) -> Result<(), McpToolError> {
    server.add_tool(tool(TOOL_LIST_STORIES, "List Stories")?, {
        let automation = automation.clone();
        move |call| {
            decode::<EmptyInput>(call)
                .map(|_| json!({ "stories": automation.stories() }))
                .map_or_else(tool_error, tool_structured_result)
        }
    })?;

    server.add_tool(tool(TOOL_GET_STORY, "Get Story")?, {
        let automation = automation.clone();
        move |call| {
            decode::<StoryKeyInput>(call)
                .and_then(|input| {
                    automation
                        .get_story(&input.key)
                        .map(|story| json!({ "story": story }))
                        .map_err(|error| error.to_string())
                })
                .map_or_else(tool_error, tool_structured_result)
        }
    })?;

    server.add_tool(tool(TOOL_CURRENT_STORY, "Current Story")?, {
        let automation = automation.clone();
        move |call| {
            decode::<EmptyInput>(call)
                .map(|_| json!(automation.current_story()))
                .map_or_else(tool_error, tool_structured_result)
        }
    })?;

    server.add_tool_async(tool(TOOL_OPEN_STORY, "Open Story")?, {
        let automation = automation.clone();
        move |call| {
            let automation = automation.clone();
            async move {
                let input = match decode::<StoryKeyInput>(call) {
                    Ok(input) => input,
                    Err(error) => return tool_error(error),
                };

                match automation.open_story(input.key).await {
                    Ok(snapshot) => tool_structured_result(json!(snapshot)),
                    Err(error) => tool_error(error.to_string()),
                }
            }
        }
    })?;

    server.add_tool_async(
        tool(TOOL_CAPTURE_CURRENT_STORY, "Capture Current Story")?,
        {
            let automation = automation.clone();
            move |call| {
                let automation = automation.clone();
                async move {
                    let input = match decode::<CaptureCurrentStoryInput>(call) {
                        Ok(input) => input,
                        Err(error) => return tool_error(error),
                    };

                    let request = StoryScreenshotRequest {
                        output_path: input.output_path,
                        width: input.width,
                        height: input.height,
                        quit_after_capture: false,
                    };

                    match automation.capture_current_story(request).await {
                        Ok(snapshot) => tool_structured_result(json!(snapshot)),
                        Err(error) => tool_error(error.to_string()),
                    }
                }
            }
        },
    )?;

    server.add_tool(
        tool(TOOL_CAPTURE_LAUNCH_ENV, "Capture Launch Env")?,
        move |call| {
            decode::<CaptureLaunchEnvInput>(call)
                .and_then(|input| {
                    build_capture_launch_env(input)
                        .map(|env| json!(env))
                        .map_err(|error| error.to_string())
                })
                .map_or_else(tool_error, tool_structured_result)
        },
    )?;

    Ok(())
}

pub fn read_capture_session(
    default_story_key: impl AsRef<str>,
) -> Result<StorybookCaptureSession, StorybookMcpError> {
    let default_story_key = default_story_key.as_ref();
    let default_route = CaptureRouteId::new(default_story_key).map_err(|error| {
        StorybookMcpError::InvalidDefaultStoryKey {
            key: default_story_key.to_string(),
            message: error.to_string(),
        }
    })?;
    let env = storybook_capture_env();
    let (story_key, _) = env.read_route_id_or(&default_route)?;
    let capture = env.read_capture(default_capture_size())?;

    Ok(StorybookCaptureSession {
        story_key: story_key.into_string(),
        capture: capture.map(StorybookCaptureConfig::from),
    })
}

pub fn capture_catalog(stories: &[StorySnapshot]) -> Value {
    json!({
        "routes": stories.iter().map(|story| {
            json!({
                "id": story.capture_route_id,
                "title": story.title,
                "default_size": story.default_size,
            })
        }).collect::<Vec<_>>()
    })
}

fn tool(name: &str, title: &str) -> Result<component_shape_mcp::ToolDefinition, McpToolError> {
    tool_definition(
        name,
        Some(title.to_string()),
        None,
        McpSchema::object().with_additional_properties(true),
        None,
    )
}

fn decode<T>(call: McpToolCall) -> Result<T, String>
where
    T: for<'de> Deserialize<'de>,
{
    serde_json::from_value(Value::Object(call.into_arguments().into_inner()))
        .map_err(|error| error.to_string())
}

fn tool_error(message: impl Into<String>) -> ToolCallResult {
    tool_error_result(message.into())
}

fn build_capture_launch_env(
    input: CaptureLaunchEnvInput,
) -> Result<CaptureLaunchEnv, StorybookMcpError> {
    let size = FrameCaptureLaunchEnv::optional_size(input.width, input.height)?;
    let mut env = FrameCaptureLaunchEnv::builder()
        .route_id(input.key)?
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

    let mut command = vec!["cargo".to_string()];
    command.extend(cargo_args.clone());

    Ok(CaptureLaunchEnv {
        env,
        cargo_args,
        command,
    })
}

fn default_capture_size() -> PixelSize {
    PixelSize::new(DEFAULT_STORY_CAPTURE_WIDTH, DEFAULT_STORY_CAPTURE_HEIGHT)
}

fn storybook_capture_env() -> CaptureEnv {
    CaptureEnv::with_prefix(CAPTURE_ENV_PREFIX)
}

impl From<CaptureConfig> for StorybookCaptureConfig {
    fn from(config: CaptureConfig) -> Self {
        Self {
            path: config.path().to_path_buf(),
            frame: config.frame().get(),
            size: StoryDefaultSize {
                width: config.size().width(),
                height: config.size().height(),
            },
        }
    }
}

pub mod capture {
    pub use super::{
        CaptureLaunchEnv, StorybookCaptureSession, capture_catalog, read_capture_session,
        start_capture_session, start_capture_session_from_env,
    };
    pub use frame_capture::{CaptureConfig, CaptureEnv, CaptureFrame, PixelSize};
}

pub mod prelude {
    pub use super::{
        CaptureLaunchEnv, SharedStoryCaptureController, SharedStoryController,
        SharedStorybookAutomation, StoryCaptureSnapshot, StoryCurrentSnapshot, StoryDefaultSize,
        StoryScreenshotRequest, StorySnapshot, StorybookAutomation, StorybookAutomationError,
        StorybookCaptureConfig, StorybookCaptureSession, capture_catalog, read_capture_session,
        server, start_capture_session, start_capture_session_from_env, start_stdio,
        stdio_requested,
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use component_shape_mcp::tool_call_structured_content;
    use std::{env, ffi::OsString, sync::Mutex};

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    struct EnvGuard(Vec<(String, Option<OsString>)>);

    impl EnvGuard {
        fn set(vars: &[(&str, &str)]) -> Self {
            let previous = vars
                .iter()
                .map(|(name, _)| ((*name).to_string(), env::var_os(name)))
                .collect();

            unsafe {
                for (name, value) in vars {
                    env::set_var(name, value);
                }
            }

            Self(previous)
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            unsafe {
                for (name, value) in self.0.drain(..) {
                    if let Some(value) = value {
                        env::set_var(name, value);
                    } else {
                        env::remove_var(name);
                    }
                }
            }
        }
    }

    #[test]
    fn capture_launch_env_returns_wgpu_env_and_command() {
        let automation = StorybookAutomation::with_stories(Vec::new());
        let server = server(automation).expect("server should build");

        let result = server.call_tool(
            TOOL_CAPTURE_LAUNCH_ENV,
            Some(json!({
                "key": "gpui-storybook-example-story-ButtonStory",
                "output_path": "target/storybook-captures/button.png",
                "width": 900,
                "height": 700,
                "package": "gpui-storybook-example-story",
                "bin": "story",
                "features": ["mcp"],
            })),
        );
        let result = serde_json::to_value(result).unwrap();
        let structured =
            tool_call_structured_content(&result).expect("tool should return structured content");

        assert_eq!(
            structured["env"]["WGPU_CAPTURE_ROUTE"],
            "gpui-storybook-example-story-ButtonStory"
        );
        assert_eq!(
            structured["env"]["WGPU_CAPTURE_PATH"],
            "target/storybook-captures/button.png"
        );
        assert_eq!(structured["env"]["WGPU_CAPTURE_WIDTH"], "900");
        assert_eq!(structured["env"]["WGPU_CAPTURE_HEIGHT"], "700");
        assert_eq!(structured["env"][STDIO_ENV_VAR], "1");
        assert_eq!(
            structured["command"],
            json!([
                "cargo",
                "run",
                "-p",
                "gpui-storybook-example-story",
                "--features",
                "mcp",
                "--bin",
                "story"
            ])
        );
    }

    #[test]
    fn read_capture_session_reads_wgpu_env() {
        let _lock = ENV_LOCK.lock().expect("env lock should not be poisoned");
        let _env = EnvGuard::set(&[
            (
                "WGPU_CAPTURE_ROUTE",
                "gpui-storybook-example-story-ButtonStory",
            ),
            ("WGPU_CAPTURE_PATH", "target/storybook-captures/button.png"),
            ("WGPU_CAPTURE_WIDTH", "900"),
            ("WGPU_CAPTURE_HEIGHT", "700"),
        ]);

        let session = read_capture_session("fallback-story").unwrap();
        let capture = session.capture.expect("capture config should be read");

        assert_eq!(
            session.story_key,
            "gpui-storybook-example-story-ButtonStory"
        );
        assert_eq!(
            capture.path,
            PathBuf::from("target/storybook-captures/button.png")
        );
        assert_eq!(capture.size.width, 900);
        assert_eq!(capture.size.height, 700);
    }

    #[test]
    fn capture_launch_env_rejects_invalid_frame_capture_values() {
        let error = build_capture_launch_env(CaptureLaunchEnvInput {
            key: "gpui-storybook-example-story-ButtonStory".to_string(),
            output_path: Some(PathBuf::from("target/storybook-captures/button.png")),
            frame: Some(0),
            width: None,
            height: None,
            package: None,
            bin: None,
            features: None,
            stdio: None,
        })
        .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("capture frame must be greater than zero")
        );

        let error = build_capture_launch_env(CaptureLaunchEnvInput {
            key: "gpui-storybook-example-story-ButtonStory".to_string(),
            output_path: None,
            frame: None,
            width: Some(900),
            height: None,
            package: None,
            bin: None,
            features: None,
            stdio: None,
        })
        .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("set both capture width and height")
        );
    }
}
