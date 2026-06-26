use crate::story::StoryContainer;
use gpui::{App, Global, Window, px};
use serde::{Deserialize, Serialize};
use std::{
    borrow::Borrow,
    fmt,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
};
use tokio::sync::{mpsc, oneshot};

pub const DEFAULT_STORY_CAPTURE_WIDTH: u32 = 1280;
pub const DEFAULT_STORY_CAPTURE_HEIGHT: u32 = 720;

/// Shared automation handle used by live storybook views and MCP integrations.
pub type SharedStorybookAutomation = Arc<StorybookAutomation>;

/// Shared story navigation controller.
pub type SharedStoryController = SharedStorybookAutomation;

/// Shared story screenshot controller.
pub type SharedStoryCaptureController = SharedStorybookAutomation;

/// App-wide automation controller used by base storybook constructors.
///
/// When this global is installed, [`Gallery`](crate::gallery::Gallery) and
/// the dock workspace attach it from their base `view(...)` constructors.
#[derive(Clone)]
pub struct DefaultStorybookAutomation {
    automation: SharedStorybookAutomation,
}

impl Global for DefaultStorybookAutomation {}

impl DefaultStorybookAutomation {
    pub fn new(automation: SharedStorybookAutomation) -> Self {
        Self { automation }
    }

    pub fn automation(&self) -> SharedStorybookAutomation {
        self.automation.clone()
    }
}

pub fn set_default_storybook_automation(
    cx: &mut App,
    automation: SharedStorybookAutomation,
) -> SharedStorybookAutomation {
    cx.set_global(DefaultStorybookAutomation::new(automation.clone()));
    automation
}

pub fn default_storybook_automation(cx: &App) -> Option<SharedStorybookAutomation> {
    cx.try_global::<DefaultStorybookAutomation>()
        .map(DefaultStorybookAutomation::automation)
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct StoryDefaultSize {
    pub width: u32,
    pub height: u32,
}

impl Default for StoryDefaultSize {
    fn default() -> Self {
        Self {
            width: DEFAULT_STORY_CAPTURE_WIDTH,
            height: DEFAULT_STORY_CAPTURE_HEIGHT,
        }
    }
}

/// Machine-readable story metadata used by automation and capture tools.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct StorySnapshot {
    pub key: String,
    pub crate_name: String,
    pub story_name: String,
    pub title: String,
    pub description: String,
    pub group: Option<String>,
    pub section: Option<String>,
    pub source_file: String,
    pub source_line: u32,
    pub capture_route_id: String,
    pub default_size: StoryDefaultSize,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct StoryCurrentSnapshot {
    pub story: Option<StorySnapshot>,
    pub revision: u64,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct StoryScreenshotRequest {
    pub output_path: Option<PathBuf>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    #[serde(default)]
    pub quit_after_capture: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct StoryCaptureSnapshot {
    pub request_id: u64,
    pub path: PathBuf,
    pub pixel_width: u32,
    pub pixel_height: u32,
    pub story: StorySnapshot,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StorybookAutomationError {
    NoLiveHost,
    HostDisconnected { message: String },
    StoryNotFound { key: String },
    CaptureAlreadyPending,
    CaptureUnavailable { message: String },
    InvalidCaptureRequest { message: String },
}

pub(crate) enum StorybookAutomationCommand {
    OpenStory {
        key: String,
        response: oneshot::Sender<Result<StoryCurrentSnapshot, StorybookAutomationError>>,
    },
    CaptureCurrentStory {
        request_id: u64,
        request: StoryScreenshotRequest,
        response: oneshot::Sender<Result<StoryCaptureSnapshot, StorybookAutomationError>>,
    },
}

#[derive(Debug, Default)]
struct StorybookAutomationState {
    stories: Vec<StorySnapshot>,
    current_story_key: Option<String>,
    revision: u64,
}

pub struct StorybookAutomation {
    state: Mutex<StorybookAutomationState>,
    command_tx: mpsc::UnboundedSender<StorybookAutomationCommand>,
    command_rx: Mutex<Option<mpsc::UnboundedReceiver<StorybookAutomationCommand>>>,
    live_host_attached: AtomicBool,
    capture_pending: AtomicBool,
    next_request_id: AtomicU64,
}

impl fmt::Display for StorybookAutomationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoLiveHost => write!(formatter, "no live GPUI storybook host is attached"),
            Self::HostDisconnected { message } => {
                write!(
                    formatter,
                    "live GPUI storybook host disconnected: {message}"
                )
            },
            Self::StoryNotFound { key } => write!(formatter, "story key `{key}` was not found"),
            Self::CaptureAlreadyPending => {
                write!(formatter, "a story screenshot request is already pending")
            },
            Self::CaptureUnavailable { message } => write!(formatter, "{message}"),
            Self::InvalidCaptureRequest { message } => write!(formatter, "{message}"),
        }
    }
}

impl std::error::Error for StorybookAutomationError {}

impl StorySnapshot {
    pub fn from_container(story: &StoryContainer, cx: &impl Borrow<App>) -> Option<Self> {
        let key = story.story_key.as_ref()?.to_string();
        let story_name = story
            .story_name
            .as_ref()
            .or(story.story_klass.as_ref())?
            .to_string();

        Some(Self {
            capture_route_id: key.clone(),
            key,
            crate_name: story
                .crate_name
                .as_ref()
                .map(ToString::to_string)
                .unwrap_or_default(),
            story_name,
            title: story.display_title(cx),
            description: story.display_description(cx),
            group: story.group.as_ref().map(ToString::to_string),
            section: story.section.as_ref().map(ToString::to_string),
            source_file: story
                .source_file
                .as_ref()
                .map(ToString::to_string)
                .unwrap_or_default(),
            source_line: story.source_line.unwrap_or_default(),
            default_size: StoryDefaultSize::default(),
        })
    }
}

impl StorybookAutomation {
    pub fn new() -> SharedStorybookAutomation {
        Self::with_stories(Vec::new())
    }

    pub fn with_stories(stories: Vec<StorySnapshot>) -> SharedStorybookAutomation {
        let (command_tx, command_rx) = mpsc::unbounded_channel();
        let current_story_key = stories.first().map(|story| story.key.clone());

        Arc::new(Self {
            state: Mutex::new(StorybookAutomationState {
                stories,
                current_story_key,
                revision: 0,
            }),
            command_tx,
            command_rx: Mutex::new(Some(command_rx)),
            live_host_attached: AtomicBool::new(false),
            capture_pending: AtomicBool::new(false),
            next_request_id: AtomicU64::new(1),
        })
    }

    pub fn set_stories(&self, stories: Vec<StorySnapshot>) {
        let mut state = self.state.lock().expect("automation state poisoned");
        let current_exists = state
            .current_story_key
            .as_ref()
            .is_some_and(|key| stories.iter().any(|story| &story.key == key));

        if !current_exists {
            state.current_story_key = stories.first().map(|story| story.key.clone());
            state.revision = state.revision.saturating_add(1);
        }

        state.stories = stories;
    }

    pub fn stories(&self) -> Vec<StorySnapshot> {
        self.state
            .lock()
            .expect("automation state poisoned")
            .stories
            .clone()
    }

    pub fn get_story(&self, key: &str) -> Result<StorySnapshot, StorybookAutomationError> {
        self.state
            .lock()
            .expect("automation state poisoned")
            .stories
            .iter()
            .find(|story| story.key == key)
            .cloned()
            .ok_or_else(|| StorybookAutomationError::StoryNotFound {
                key: key.to_string(),
            })
    }

    pub fn current_story(&self) -> StoryCurrentSnapshot {
        let state = self.state.lock().expect("automation state poisoned");
        let story = state.current_story_key.as_ref().and_then(|key| {
            state
                .stories
                .iter()
                .find(|story| &story.key == key)
                .cloned()
        });

        StoryCurrentSnapshot {
            story,
            revision: state.revision,
        }
    }

    pub async fn open_story(
        &self,
        key: impl Into<String>,
    ) -> Result<StoryCurrentSnapshot, StorybookAutomationError> {
        let key = key.into();
        self.get_story(&key)?;

        if !self.live_host_attached.load(Ordering::SeqCst) {
            return Err(StorybookAutomationError::NoLiveHost);
        }

        let (response, receiver) = oneshot::channel();
        self.command_tx
            .send(StorybookAutomationCommand::OpenStory { key, response })
            .map_err(|_| StorybookAutomationError::NoLiveHost)?;

        receiver
            .await
            .map_err(|error| StorybookAutomationError::HostDisconnected {
                message: error.to_string(),
            })?
    }

    pub async fn capture_current_story(
        &self,
        request: StoryScreenshotRequest,
    ) -> Result<StoryCaptureSnapshot, StorybookAutomationError> {
        if !self.live_host_attached.load(Ordering::SeqCst) {
            return Err(StorybookAutomationError::NoLiveHost);
        }

        self.capture_pending
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .map_err(|_| StorybookAutomationError::CaptureAlreadyPending)?;

        let request_id = self.next_request_id.fetch_add(1, Ordering::SeqCst);
        let (response, receiver) = oneshot::channel();
        let send_result = self
            .command_tx
            .send(StorybookAutomationCommand::CaptureCurrentStory {
                request_id,
                request,
                response,
            })
            .map_err(|_| StorybookAutomationError::NoLiveHost);

        if let Err(error) = send_result {
            self.capture_pending.store(false, Ordering::SeqCst);
            return Err(error);
        }

        let result =
            receiver
                .await
                .map_err(|error| StorybookAutomationError::HostDisconnected {
                    message: error.to_string(),
                })?;
        self.capture_pending.store(false, Ordering::SeqCst);
        result
    }

    pub(crate) fn take_command_receiver(
        &self,
    ) -> Option<mpsc::UnboundedReceiver<StorybookAutomationCommand>> {
        let receiver = self
            .command_rx
            .lock()
            .expect("automation receiver poisoned")
            .take();

        if receiver.is_some() {
            self.live_host_attached.store(true, Ordering::SeqCst);
        }

        receiver
    }

    pub(crate) fn confirm_current_story(
        &self,
        key: &str,
    ) -> Result<StoryCurrentSnapshot, StorybookAutomationError> {
        let mut state = self.state.lock().expect("automation state poisoned");
        if !state.stories.iter().any(|story| story.key == key) {
            return Err(StorybookAutomationError::StoryNotFound {
                key: key.to_string(),
            });
        }

        if state.current_story_key.as_deref() != Some(key) {
            state.current_story_key = Some(key.to_string());
            state.revision = state.revision.saturating_add(1);
        }

        let story = state.stories.iter().find(|story| story.key == key).cloned();

        Ok(StoryCurrentSnapshot {
            story,
            revision: state.revision,
        })
    }
}

pub fn story_snapshots_from_containers(
    stories: &[gpui::Entity<StoryContainer>],
    cx: &impl Borrow<App>,
) -> Vec<StorySnapshot> {
    fn collect(
        story: &gpui::Entity<StoryContainer>,
        snapshots: &mut Vec<StorySnapshot>,
        cx: &impl Borrow<App>,
    ) {
        let (snapshot, members) = {
            let story = story.read(cx.borrow());
            (
                StorySnapshot::from_container(story, cx),
                story.list_members.clone(),
            )
        };

        if let Some(snapshot) = snapshot {
            snapshots.push(snapshot);
        }

        for member in members {
            collect(&member, snapshots, cx);
        }
    }

    let mut snapshots = Vec::new();
    for story in stories {
        collect(story, &mut snapshots, cx);
    }
    snapshots
}

pub fn default_capture_output_path(story: &StorySnapshot) -> PathBuf {
    PathBuf::from("target")
        .join("storybook-captures")
        .join(format!("{}.png", story.key))
}

pub(crate) fn validate_capture_target_size(
    request: &StoryScreenshotRequest,
) -> Result<Option<(u32, u32)>, StorybookAutomationError> {
    match (request.width, request.height) {
        (Some(width), Some(height)) if width > 0 && height > 0 => Ok(Some((width, height))),
        (Some(_), Some(_)) => Err(StorybookAutomationError::InvalidCaptureRequest {
            message: "capture width and height must be greater than zero".to_string(),
        }),
        (None, None) => Ok(None),
        _ => Err(StorybookAutomationError::InvalidCaptureRequest {
            message: "capture width and height must be provided together".to_string(),
        }),
    }
}

pub(crate) fn apply_capture_target_size(window: &mut Window, target_size: Option<(u32, u32)>) {
    if let Some((width, height)) = target_size {
        let scale_factor = window.scale_factor().max(f32::EPSILON);
        window.resize(gpui::size(
            px(width as f32 / scale_factor),
            px(height as f32 / scale_factor),
        ));
    }
}

pub(crate) fn render_story_capture(
    request_id: u64,
    request: StoryScreenshotRequest,
    story: StorySnapshot,
    window: &mut Window,
) -> Result<StoryCaptureSnapshot, StorybookAutomationError> {
    #[cfg(feature = "capture")]
    {
        let image = window.render_to_image().map_err(|error| {
            StorybookAutomationError::CaptureUnavailable {
                message: format!("failed to render current story to image: {error}"),
            }
        })?;
        let path = request
            .output_path
            .unwrap_or_else(|| default_capture_output_path(&story));

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| {
                StorybookAutomationError::CaptureUnavailable {
                    message: format!("failed to create capture output directory: {error}"),
                }
            })?;
        }

        image
            .save(&path)
            .map_err(|error| StorybookAutomationError::CaptureUnavailable {
                message: format!(
                    "failed to save story capture to {}: {error}",
                    path.display()
                ),
            })?;

        Ok(StoryCaptureSnapshot {
            request_id,
            path,
            pixel_width: image.width(),
            pixel_height: image.height(),
            story,
        })
    }

    #[cfg(not(feature = "capture"))]
    {
        let _ = (request_id, request, story, window);
        Err(StorybookAutomationError::CaptureUnavailable {
            message: "story capture requires the gpui-storybook-core `capture` feature".to_string(),
        })
    }
}

pub(crate) fn capture_exit_code(
    result: &Result<StoryCaptureSnapshot, StorybookAutomationError>,
) -> i32 {
    if let Err(error) = result {
        eprintln!("gpui-storybook capture session failed: {error}");
        1
    } else {
        0
    }
}
