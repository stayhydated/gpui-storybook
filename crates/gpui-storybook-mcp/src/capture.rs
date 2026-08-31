//! Startup capture sessions and capture launch metadata.

use std::{collections::BTreeMap, path::PathBuf, thread, time::Duration};

use frame_capture::CaptureRouteId;
pub use frame_capture::{CaptureConfig, CaptureEnv, CaptureFrame, PixelSize};
use gpui_storybook_core::automation::{
    DEFAULT_STORY_CAPTURE_HEIGHT, DEFAULT_STORY_CAPTURE_WIDTH, SharedStorybookAutomation,
    StoryDefaultSize, StoryScreenshotRequest, StorySnapshot,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::StorybookMcpError;

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

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[schemars(deny_unknown_fields)]
pub struct CaptureLaunchEnv {
    /// Environment variables to merge into the launched process.
    pub env: BTreeMap<String, String>,
    /// Cargo arguments appended to the platform launch command.
    pub cargo_args: Vec<String>,
    /// Platform launch command: the Sway wrapper on Linux or Cargo on macOS.
    pub command: Vec<String>,
}

/// Returns whether frame-capture route or output configuration is present.
pub fn capture_requested() -> bool {
    let env = storybook_capture_env();
    std::env::var_os(env.route_var()).is_some() || std::env::var_os(env.path_var()).is_some()
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
                        viewport: None,
                        controls: BTreeMap::new(),
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

fn default_capture_size() -> PixelSize {
    PixelSize::new(DEFAULT_STORY_CAPTURE_WIDTH, DEFAULT_STORY_CAPTURE_HEIGHT)
}

pub(crate) fn storybook_capture_env() -> CaptureEnv {
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
