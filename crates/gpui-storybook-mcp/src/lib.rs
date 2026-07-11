//! MCP tools for driving a live `gpui-storybook` window.

use component_shape_mcp::{
    McpSchema, McpSchemaProperties, McpServer, McpToolError, McpToolInput, McpToolMetadata,
    McpTypedTool, ServeStdioResult, nullable_schema, tool_definition_for_input_with_metadata,
    tool_error_result_for, tool_structured_result,
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
use rmcp::model::CallToolResult as ToolCallResult;
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

/// No arguments.
#[derive(Clone, Debug, Default)]
struct EmptyInput {}

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
struct StoryKeyInput {
    /// Stable story key or `story-key/substory-key` capture route.
    key: String,
}

/// Capture the story currently displayed by the live storybook window.
#[derive(Clone, Debug, Default, component_shape_mcp::McpToolInput)]
struct CaptureCurrentStoryInput {
    /// PNG output path. The capture runtime chooses its default when omitted.
    output_path: Option<PathBuf>,
    /// Requested capture width in pixels. Set together with `height`.
    width: Option<u32>,
    /// Requested capture height in pixels. Set together with `width`.
    height: Option<u32>,
}

/// Build the environment and Cargo command for launching a capture-enabled storybook.
#[derive(Clone, Debug, component_shape_mcp::McpToolInput)]
struct CaptureLaunchEnvInput {
    /// Stable story key or `story-key/substory-key` capture route.
    key: String,
    /// PNG output path. Omit it to open the route without taking a capture.
    output_path: Option<PathBuf>,
    /// One-based frame number to capture.
    frame: Option<u32>,
    /// Requested capture width in pixels. Set together with `height`.
    width: Option<u32>,
    /// Requested capture height in pixels. Set together with `width`.
    height: Option<u32>,
    /// Optional Cargo package passed with `-p`.
    package: Option<String>,
    /// Optional Cargo binary passed with `--bin`.
    bin: Option<String>,
    /// Cargo features passed with `--features`.
    features: Option<Vec<String>>,
    /// Whether to include `GPUI_STORYBOOK_MCP_STDIO=1`; defaults to `true`.
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
    server.add_typed_tool(
        tool::<EmptyInput>(
            TOOL_LIST_STORIES,
            "List Stories",
            "List the registered stories and their stable capture route metadata.",
            list_stories_output_schema(),
            ToolHints::read_only(),
        )?,
        {
            let automation = automation.clone();
            move |_input| tool_structured_result(json!({ "stories": automation.stories() }))
        },
    )?;

    server.add_typed_tool(
        tool::<StoryKeyInput>(
            TOOL_GET_STORY,
            "Get Story",
            "Inspect one registered story or sub-story route by its stable key.",
            get_story_output_schema(),
            ToolHints::read_only(),
        )?,
        {
            let automation = automation.clone();
            move |input| match automation.get_story(&input.key) {
                Ok(story) => tool_structured_result(json!({ "story": story })),
                Err(error) => automation_tool_error(error),
            }
        },
    )?;

    server.add_typed_tool(
        tool::<EmptyInput>(
            TOOL_CURRENT_STORY,
            "Current Story",
            "Inspect the story currently displayed by the live storybook window.",
            current_story_output_schema(),
            ToolHints::read_only(),
        )?,
        {
            let automation = automation.clone();
            move |_input| tool_structured_result(json!(automation.current_story()))
        },
    )?;

    server.add_typed_tool_async(
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
                    match automation.open_story(input.key).await {
                        Ok(snapshot) => tool_structured_result(json!(snapshot)),
                        Err(error) => automation_tool_error(error),
                    }
                }
            }
        },
    )?;

    server.add_typed_tool_async(
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
                let request = StoryScreenshotRequest {
                    output_path: input.output_path,
                    width: input.width,
                    height: input.height,
                    quit_after_capture: false,
                };

                match automation.capture_current_story(request).await {
                    Ok(snapshot) => tool_structured_result(json!(snapshot)),
                    Err(error) => automation_tool_error(error),
                }
            }
        },
    )?;

    server.add_typed_tool(
        capture_tool::<CaptureLaunchEnvInput>(
            TOOL_CAPTURE_LAUNCH_ENV,
            "Capture Launch Env",
            "Build frame-capture environment variables and a Cargo launch command for a story route.",
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

#[derive(Clone, Copy)]
struct ToolHints {
    read_only: bool,
    destructive: bool,
    idempotent: bool,
}

impl ToolHints {
    const fn read_only() -> Self {
        Self {
            read_only: true,
            destructive: false,
            idempotent: true,
        }
    }

    const fn mutation(idempotent: bool, destructive: bool) -> Self {
        Self {
            read_only: false,
            destructive,
            idempotent,
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
            .with_open_world_hint(false),
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

fn set_optional_positive_integer(properties: &mut serde_json::Map<String, Value>, field: &str) {
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

fn automation_tool_error(error: StorybookAutomationError) -> ToolCallResult {
    let error = match error {
        StorybookAutomationError::StoryNotFound { key } => {
            McpToolError::invalid_field_value("key", key)
        },
        error => McpToolError::handler(error.to_string()),
    };
    tool_error_result_for(error)
}

fn object_schema<const N: usize>(
    properties: [(String, McpSchema); N],
    required: impl IntoIterator<Item = &'static str>,
) -> McpSchema {
    McpSchema::object()
        .with_properties(McpSchemaProperties::from(properties))
        .with_required(required)
        .with_additional_properties(false)
}

fn story_size_schema() -> McpSchema {
    object_schema(
        [
            ("width".to_string(), McpSchema::integer()),
            ("height".to_string(), McpSchema::integer()),
        ],
        ["width", "height"],
    )
}

fn story_schema() -> McpSchema {
    object_schema(
        [
            ("key".to_string(), McpSchema::string()),
            ("crate_name".to_string(), McpSchema::string()),
            ("story_name".to_string(), McpSchema::string()),
            ("title".to_string(), McpSchema::string()),
            ("description".to_string(), McpSchema::string()),
            ("group".to_string(), nullable_schema(McpSchema::string())),
            ("section".to_string(), nullable_schema(McpSchema::string())),
            ("source_file".to_string(), McpSchema::string()),
            ("source_line".to_string(), McpSchema::integer()),
            ("capture_route_id".to_string(), McpSchema::string()),
            ("default_size".to_string(), story_size_schema()),
        ],
        [
            "key",
            "crate_name",
            "story_name",
            "title",
            "description",
            "group",
            "section",
            "source_file",
            "source_line",
            "capture_route_id",
            "default_size",
        ],
    )
}

fn list_stories_output_schema() -> McpSchema {
    object_schema(
        [("stories".to_string(), McpSchema::array(story_schema()))],
        ["stories"],
    )
}

fn get_story_output_schema() -> McpSchema {
    object_schema([("story".to_string(), story_schema())], ["story"])
}

fn current_story_output_schema() -> McpSchema {
    object_schema(
        [
            ("story".to_string(), nullable_schema(story_schema())),
            ("revision".to_string(), McpSchema::integer()),
        ],
        ["story", "revision"],
    )
}

fn capture_story_output_schema() -> McpSchema {
    object_schema(
        [
            ("request_id".to_string(), McpSchema::integer()),
            ("path".to_string(), McpSchema::string()),
            ("pixel_width".to_string(), McpSchema::integer()),
            ("pixel_height".to_string(), McpSchema::integer()),
            ("story".to_string(), story_schema()),
        ],
        ["request_id", "path", "pixel_width", "pixel_height", "story"],
    )
}

fn capture_launch_env_output_schema() -> McpSchema {
    let string_array = || McpSchema::array(McpSchema::string());
    object_schema(
        [
            (
                "env".to_string(),
                McpSchema::object().with_additional_properties(McpSchema::string()),
            ),
            ("cargo_args".to_string(), string_array()),
            ("command".to_string(), string_array()),
        ],
        ["env", "cargo_args", "command"],
    )
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

        fn remove(names: &[&str]) -> Self {
            let previous = names
                .iter()
                .map(|name| ((*name).to_string(), env::var_os(name)))
                .collect();

            unsafe {
                for name in names {
                    env::remove_var(name);
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

    fn sample_story() -> StorySnapshot {
        StorySnapshot {
            key: "example-ButtonStory".to_string(),
            crate_name: "example".to_string(),
            story_name: "ButtonStory".to_string(),
            title: "Button".to_string(),
            description: "Button states".to_string(),
            group: Some("Inputs".to_string()),
            section: None,
            source_file: "src/stories/button.rs".to_string(),
            source_line: 12,
            capture_route_id: "example-ButtonStory".to_string(),
            default_size: StoryDefaultSize::default(),
        }
    }

    #[test]
    fn tools_advertise_typed_inputs_outputs_and_annotations() {
        let server = server(StorybookAutomation::with_stories(vec![sample_story()]))
            .expect("server should build");
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
        assert_eq!(get["inputSchema"]["required"], json!(["key"]));
        assert_eq!(get["inputSchema"]["properties"]["key"]["type"], "string");
        assert_eq!(get["outputSchema"]["properties"]["story"]["type"], "object");

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

        let launch = find(TOOL_CAPTURE_LAUNCH_ENV);
        assert_eq!(launch["inputSchema"]["required"], json!(["key"]));
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
            server.call_tool(TOOL_GET_STORY, Some(json!({ "key": story.key }))),
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

        let unknown = serde_json::to_value(
            server.call_tool(TOOL_GET_STORY, Some(json!({ "key": "missing-story" }))),
        )
        .expect("result should serialize");
        assert_eq!(
            unknown["structuredContent"]["error"]["kind"],
            "invalid_field_value"
        );
    }

    #[test]
    fn capture_output_schema_accepts_runtime_snapshot_shape() {
        let snapshot = StoryCaptureSnapshot {
            request_id: 7,
            path: PathBuf::from("target/storybook-captures/button.png"),
            pixel_width: 900,
            pixel_height: 700,
            story: sample_story(),
        };
        let definition = component_shape_mcp::tool_definition(
            "capture_schema_test",
            None,
            None,
            McpSchema::object().with_additional_properties(false),
            Some(capture_story_output_schema()),
        )
        .expect("capture schema should define a valid tool");
        let mut server = McpServer::new("capture-schema-test", "0.0.0");
        server
            .add_tool(definition, move |_| tool_structured_result(json!(snapshot)))
            .expect("schema test tool should register");

        let result = serde_json::to_value(server.call_tool("capture_schema_test", Some(json!({}))))
            .expect("result should serialize");
        assert_eq!(result["isError"], false);
        assert_eq!(result["structuredContent"]["pixel_width"], 900);
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

    #[test]
    fn stdio_flag_requires_the_explicit_enabled_value() {
        let _lock = ENV_LOCK.lock().expect("env lock should not be poisoned");
        let _unset = EnvGuard::remove(&[STDIO_ENV_VAR]);
        assert!(!stdio_requested());

        {
            let _disabled = EnvGuard::set(&[(STDIO_ENV_VAR, "0")]);
            assert!(!stdio_requested());
        }
        {
            let _enabled = EnvGuard::set(&[(STDIO_ENV_VAR, "1")]);
            assert!(stdio_requested());
        }
    }

    #[test]
    fn capture_catalog_exposes_route_metadata_only() {
        let story = sample_story();
        assert_eq!(
            capture_catalog(std::slice::from_ref(&story)),
            json!({
                "routes": [{
                    "id": story.capture_route_id,
                    "title": story.title,
                    "default_size": story.default_size,
                }]
            })
        );
        assert_eq!(capture_catalog(&[]), json!({ "routes": [] }));
    }

    #[test]
    fn capture_session_defaults_without_capture_environment() {
        let _lock = ENV_LOCK.lock().expect("env lock should not be poisoned");
        let _env = EnvGuard::remove(&[
            "WGPU_CAPTURE_ROUTE",
            "WGPU_CAPTURE_PATH",
            "WGPU_CAPTURE_FRAME",
            "WGPU_CAPTURE_WIDTH",
            "WGPU_CAPTURE_HEIGHT",
        ]);

        let session = read_capture_session("fallback-story").expect("fallback should be valid");
        assert_eq!(
            session,
            StorybookCaptureSession {
                story_key: "fallback-story".to_string(),
                capture: None,
            }
        );

        let error = read_capture_session("").expect_err("blank fallback route should fail");
        assert!(matches!(
            error,
            StorybookMcpError::InvalidDefaultStoryKey { key, .. } if key.is_empty()
        ));
    }

    #[test]
    fn capture_launch_env_supports_minimal_non_stdio_commands() {
        let launch = build_capture_launch_env(CaptureLaunchEnvInput {
            key: "example-ButtonStory".to_string(),
            output_path: None,
            frame: None,
            width: None,
            height: None,
            package: None,
            bin: None,
            features: Some(Vec::new()),
            stdio: Some(false),
        })
        .expect("minimal launch environment should build");

        assert_eq!(launch.cargo_args, vec!["run"]);
        assert_eq!(launch.command, vec!["cargo", "run"]);
        assert!(!launch.env.contains_key(STDIO_ENV_VAR));
        assert_eq!(launch.env["WGPU_CAPTURE_ROUTE"], "example-ButtonStory");
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
            Some(json!({ "key": "example-ButtonStory" })),
        ))
        .expect("open result should serialize");
        assert_eq!(open["isError"], true);
        assert_eq!(open["structuredContent"]["error"]["kind"], "handler");

        let capture = serde_json::to_value(server.call_tool(
            TOOL_CAPTURE_CURRENT_STORY,
            Some(json!({ "output_path": "capture.png", "width": 800, "height": 600 })),
        ))
        .expect("capture result should serialize");
        assert_eq!(capture["isError"], true);
        assert_eq!(capture["structuredContent"]["error"]["kind"], "handler");
    }

    #[test]
    fn capture_session_thread_reports_missing_live_host() {
        let automation = StorybookAutomation::with_stories(vec![sample_story()]);
        let handle = start_capture_session(
            automation,
            StorybookCaptureSession {
                story_key: "example-ButtonStory".to_string(),
                capture: None,
            },
            false,
        )
        .expect("capture session thread should start");

        let error = handle
            .join()
            .expect("capture session thread should not panic")
            .expect_err("a detached automation host should fail");
        assert!(matches!(
            error,
            StorybookMcpError::Automation(StorybookAutomationError::NoLiveHost)
        ));
    }

    #[test]
    fn capture_session_from_env_handles_absent_and_late_story_registration() {
        let _lock = ENV_LOCK.lock().expect("env lock should not be poisoned");
        let _clean = EnvGuard::remove(&["WGPU_CAPTURE_ROUTE", "WGPU_CAPTURE_PATH"]);
        assert!(
            start_capture_session_from_env(StorybookAutomation::new())
                .expect("absent capture env should not fail")
                .is_none()
        );

        let _route = EnvGuard::set(&[("WGPU_CAPTURE_ROUTE", "example-ButtonStory")]);
        let automation = StorybookAutomation::new();
        let handle = start_capture_session_from_env(automation.clone())
            .expect("capture waiter should start")
            .expect("capture route should request a session");
        automation.set_stories(vec![sample_story()]);

        let error = handle
            .join()
            .expect("capture waiter should not panic")
            .expect_err("a detached automation host should fail");
        assert!(matches!(
            error,
            StorybookMcpError::Automation(StorybookAutomationError::NoLiveHost)
        ));
    }
}
