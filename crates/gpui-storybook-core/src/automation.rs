#[cfg(feature = "capture")]
use crate::capture_output::CaptureOutputStore;
#[cfg(feature = "capture")]
use crate::capture_region::capture_region_bounds;
use crate::{
    capture_region::{capture_route_story_key, scroll_capture_region_into_view},
    story::StoryContainer,
};
use gpui::{App, Global, Window, px};
#[cfg(feature = "capture")]
use gpui::{Bounds, Pixels, point};
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
            Self::StoryNotFound { key } => write!(formatter, "story route `{key}` was not found"),
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
        let key = story.story_key_label()?.to_string();
        let story_name = story
            .story_name_label()
            .or_else(|| {
                story
                    .story_klass
                    .as_ref()
                    .map(|story_klass| story_klass.as_ref())
            })?
            .to_string();

        Some(Self {
            capture_route_id: key.clone(),
            key,
            crate_name: story.crate_name_label().unwrap_or_default().to_string(),
            story_name,
            title: story.display_title(cx),
            description: story.display_description(cx),
            group: story.group.as_ref().map(ToString::to_string),
            section: story.section.as_ref().map(ToString::to_string),
            source_file: story.source_file_label().unwrap_or_default().to_string(),
            source_line: story.source_line().unwrap_or_default(),
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
            .is_some_and(|key| resolve_story_route(&stories, key).is_some());

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
        let state = self.state.lock().expect("automation state poisoned");

        resolve_story_route(&state.stories, key).ok_or_else(|| {
            StorybookAutomationError::StoryNotFound {
                key: key.to_string(),
            }
        })
    }

    pub fn current_story(&self) -> StoryCurrentSnapshot {
        let state = self.state.lock().expect("automation state poisoned");
        let story = state
            .current_story_key
            .as_ref()
            .and_then(|key| resolve_story_route(&state.stories, key));

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

        let result = receiver
            .await
            .map_err(|error| StorybookAutomationError::HostDisconnected {
                message: error.to_string(),
            });
        self.capture_pending.store(false, Ordering::SeqCst);
        result?
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
        let story = resolve_story_route(&state.stories, key).ok_or_else(|| {
            StorybookAutomationError::StoryNotFound {
                key: key.to_string(),
            }
        })?;

        if state.current_story_key.as_deref() != Some(key) {
            state.current_story_key = Some(key.to_string());
            state.revision = state.revision.saturating_add(1);
        }

        Ok(StoryCurrentSnapshot {
            story: Some(story),
            revision: state.revision,
        })
    }
}

fn resolve_story_route(stories: &[StorySnapshot], route_id: &str) -> Option<StorySnapshot> {
    let story_key = capture_route_story_key(route_id);
    let story = stories
        .iter()
        .find(|story| story.key == story_key || story.capture_route_id == story_key)?;

    Some(story_snapshot_for_route(story.clone(), route_id))
}

fn story_snapshot_for_route(mut story: StorySnapshot, route_id: &str) -> StorySnapshot {
    if route_id != story.capture_route_id {
        story.capture_route_id = route_id.to_string();
        if let Some((_, slug)) = route_id.split_once('/') {
            story.title = format!("{} / {}", story.title, humanize_capture_slug(slug));
        }
    }

    story
}

fn humanize_capture_slug(slug: &str) -> String {
    let mut result = String::new();
    let mut capitalize_next = true;

    for ch in slug.chars() {
        if ch == '-' || ch == '_' {
            result.push(' ');
            capitalize_next = true;
        } else if capitalize_next {
            result.push(ch.to_ascii_uppercase());
            capitalize_next = false;
        } else {
            result.push(ch);
        }
    }

    result
}

pub(crate) fn schedule_story_capture(
    request_id: u64,
    request: StoryScreenshotRequest,
    story: StorySnapshot,
    response: oneshot::Sender<Result<StoryCaptureSnapshot, StorybookAutomationError>>,
    quit_after_capture: bool,
    window: &mut Window,
) {
    window.on_next_frame(move |window, _cx| {
        if !scroll_capture_region_into_view(&story.capture_route_id) {
            let result = Err(StorybookAutomationError::CaptureUnavailable {
                message: format!(
                    "capture route `{}` was not rendered by the current story view",
                    story.capture_route_id
                ),
            });
            let exit_code = capture_exit_code(&result);
            let _ = response.send(result);
            if quit_after_capture {
                std::process::exit(exit_code);
            }
            return;
        }

        window.refresh();
        window.on_next_frame(move |window, _cx| {
            let result = render_story_capture(request_id, request, story, window);
            let exit_code = capture_exit_code(&result);
            let _ = response.send(result);
            if quit_after_capture {
                std::process::exit(exit_code);
            }
        });
    });
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
        .join(format!("{}.png", story.capture_route_id))
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
        let image = crop_story_capture_image(image, &story, window)?;
        let path = request
            .output_path
            .unwrap_or_else(|| default_capture_output_path(&story));

        CaptureOutputStore::create_parent(&path).map_err(|error| {
            StorybookAutomationError::CaptureUnavailable {
                message: format!("failed to create capture output directory: {error}"),
            }
        })?;
        CaptureOutputStore::save_png(&image, &path).map_err(|error| {
            StorybookAutomationError::CaptureUnavailable {
                message: format!(
                    "failed to save story capture to {}: {error}",
                    path.display()
                ),
            }
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

#[cfg(feature = "capture")]
fn crop_story_capture_image(
    image: image::RgbaImage,
    story: &StorySnapshot,
    window: &Window,
) -> Result<image::RgbaImage, StorybookAutomationError> {
    let region = capture_region_bounds(&story.capture_route_id).ok_or_else(|| {
        StorybookAutomationError::CaptureUnavailable {
            message: format!(
                "capture route `{}` was not rendered by the current story view",
                story.capture_route_id
            ),
        }
    })?;
    let window_size = window.bounds().size;
    let window_bounds = Bounds {
        origin: point(px(0.), px(0.)),
        size: window_size,
    };
    let bounds = region.bounds.intersect(&window_bounds);

    let Some((x, y, width, height)) = image_crop_rect(bounds, window_size, &image) else {
        return Err(StorybookAutomationError::CaptureUnavailable {
            message: format!(
                "capture route `{}` is outside the rendered story view",
                story.capture_route_id
            ),
        });
    };

    Ok(image::imageops::crop_imm(&image, x, y, width, height).to_image())
}

#[cfg(feature = "capture")]
fn image_crop_rect(
    bounds: Bounds<Pixels>,
    window_size: gpui::Size<Pixels>,
    image: &image::RgbaImage,
) -> Option<(u32, u32, u32, u32)> {
    let window_width = f32::from(window_size.width);
    let window_height = f32::from(window_size.height);
    if window_width <= 0. || window_height <= 0. || image.width() == 0 || image.height() == 0 {
        return None;
    }

    let x_scale = image.width() as f32 / window_width;
    let y_scale = image.height() as f32 / window_height;
    let left = (f32::from(bounds.origin.x) * x_scale)
        .floor()
        .clamp(0., image.width() as f32) as u32;
    let top = (f32::from(bounds.origin.y) * y_scale)
        .floor()
        .clamp(0., image.height() as f32) as u32;
    let right = ((f32::from(bounds.origin.x) + f32::from(bounds.size.width)) * x_scale)
        .ceil()
        .clamp(0., image.width() as f32) as u32;
    let bottom = ((f32::from(bounds.origin.y) + f32::from(bounds.size.height)) * y_scale)
        .ceil()
        .clamp(0., image.height() as f32) as u32;

    let width = right.checked_sub(left)?;
    let height = bottom.checked_sub(top)?;
    if width == 0 || height == 0 {
        return None;
    }

    Some((left, top, width, height))
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

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_story(key: &str, title: &str) -> StorySnapshot {
        StorySnapshot {
            key: key.to_string(),
            crate_name: "crate".to_string(),
            story_name: format!("{title}Story"),
            title: title.to_string(),
            description: format!("{title} description"),
            group: Some("Examples".to_string()),
            section: Some("Components".to_string()),
            source_file: format!("src/{}.rs", title.to_lowercase()),
            source_line: 7,
            capture_route_id: key.to_string(),
            default_size: StoryDefaultSize::default(),
        }
    }

    #[test]
    fn story_routes_resolve_substory_capture_ids() {
        let automation =
            StorybookAutomation::with_stories(vec![sample_story("crate-ButtonStory", "Button")]);

        let story = automation
            .get_story("crate-ButtonStory/with-progress")
            .expect("substory route should resolve through its base story");

        assert_eq!(story.key, "crate-ButtonStory");
        assert_eq!(story.capture_route_id, "crate-ButtonStory/with-progress");
        assert_eq!(story.title, "Button / With Progress");
    }

    #[test]
    fn automation_state_tracks_stories_current_route_and_revision() {
        let button = sample_story("crate-ButtonStory", "Button");
        let table = sample_story("crate-TableStory", "Table");
        let automation = StorybookAutomation::with_stories(vec![button.clone(), table.clone()]);

        assert_eq!(automation.stories(), vec![button.clone(), table.clone()]);
        assert_eq!(automation.current_story().story, Some(button.clone()));
        assert_eq!(automation.current_story().revision, 0);

        let current = automation
            .confirm_current_story("crate-TableStory")
            .expect("registered story should become current");
        assert_eq!(current.story, Some(table.clone()));
        assert_eq!(current.revision, 1);

        let unchanged = automation
            .confirm_current_story("crate-TableStory")
            .expect("current story should remain valid");
        assert_eq!(unchanged.revision, 1);

        automation.set_stories(vec![table.clone()]);
        assert_eq!(automation.current_story().story, Some(table));
        assert_eq!(automation.current_story().revision, 1);

        automation.set_stories(vec![button.clone()]);
        assert_eq!(automation.current_story().story, Some(button));
        assert_eq!(automation.current_story().revision, 2);

        automation.set_stories(Vec::new());
        assert_eq!(automation.current_story().story, None);
        assert_eq!(automation.current_story().revision, 3);
    }

    #[test]
    fn missing_story_routes_return_typed_errors() {
        let automation = StorybookAutomation::new();

        assert_eq!(
            automation.get_story("missing"),
            Err(StorybookAutomationError::StoryNotFound {
                key: "missing".to_string(),
            })
        );
        assert_eq!(
            automation.confirm_current_story("missing"),
            Err(StorybookAutomationError::StoryNotFound {
                key: "missing".to_string(),
            })
        );
    }

    #[test]
    fn open_and_capture_require_a_live_host() {
        let story = sample_story("crate-ButtonStory", "Button");
        let automation = StorybookAutomation::with_stories(vec![story]);
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("test runtime should build");

        runtime.block_on(async {
            assert_eq!(
                automation.open_story("crate-ButtonStory").await,
                Err(StorybookAutomationError::NoLiveHost)
            );
            assert_eq!(
                automation
                    .capture_current_story(StoryScreenshotRequest::default())
                    .await,
                Err(StorybookAutomationError::NoLiveHost)
            );
            assert_eq!(
                automation.open_story("missing").await,
                Err(StorybookAutomationError::StoryNotFound {
                    key: "missing".to_string(),
                })
            );
        });
    }

    #[test]
    fn command_receiver_attaches_once() {
        let automation = StorybookAutomation::new();

        assert!(automation.take_command_receiver().is_some());
        assert!(automation.take_command_receiver().is_none());
    }

    #[gpui::test]
    fn default_automation_round_trips_through_app_global(cx: &mut App) {
        assert!(default_storybook_automation(cx).is_none());
        let automation = StorybookAutomation::new();
        let installed = set_default_storybook_automation(cx, automation.clone());
        let restored = default_storybook_automation(cx).expect("automation should be installed");

        assert!(Arc::ptr_eq(&automation, &installed));
        assert!(Arc::ptr_eq(&automation, &restored));
        let wrapper = DefaultStorybookAutomation::new(automation.clone());
        assert!(Arc::ptr_eq(&wrapper.automation(), &automation));
    }

    #[test]
    fn default_sizes_paths_and_capture_validation_are_stable() {
        let story = sample_story("crate-ButtonStory/with-icon", "Button");
        assert_eq!(
            StoryDefaultSize::default(),
            StoryDefaultSize {
                width: DEFAULT_STORY_CAPTURE_WIDTH,
                height: DEFAULT_STORY_CAPTURE_HEIGHT,
            }
        );
        assert_eq!(
            default_capture_output_path(&story),
            PathBuf::from("target/storybook-captures/crate-ButtonStory/with-icon.png")
        );

        assert_eq!(
            validate_capture_target_size(&StoryScreenshotRequest::default()),
            Ok(None)
        );
        assert_eq!(
            validate_capture_target_size(&StoryScreenshotRequest {
                width: Some(800),
                height: Some(600),
                ..StoryScreenshotRequest::default()
            }),
            Ok(Some((800, 600)))
        );
        assert!(matches!(
            validate_capture_target_size(&StoryScreenshotRequest {
                width: Some(0),
                height: Some(600),
                ..StoryScreenshotRequest::default()
            }),
            Err(StorybookAutomationError::InvalidCaptureRequest { message })
                if message.contains("greater than zero")
        ));
        assert!(matches!(
            validate_capture_target_size(&StoryScreenshotRequest {
                width: Some(800),
                ..StoryScreenshotRequest::default()
            }),
            Err(StorybookAutomationError::InvalidCaptureRequest { message })
                if message.contains("provided together")
        ));
    }

    #[test]
    fn automation_errors_have_actionable_messages_and_exit_codes() {
        let errors = [
            (
                StorybookAutomationError::NoLiveHost,
                "no live GPUI storybook host is attached",
            ),
            (
                StorybookAutomationError::HostDisconnected {
                    message: "closed".to_string(),
                },
                "live GPUI storybook host disconnected: closed",
            ),
            (
                StorybookAutomationError::StoryNotFound {
                    key: "missing".to_string(),
                },
                "story route `missing` was not found",
            ),
            (
                StorybookAutomationError::CaptureAlreadyPending,
                "a story screenshot request is already pending",
            ),
            (
                StorybookAutomationError::CaptureUnavailable {
                    message: "unavailable".to_string(),
                },
                "unavailable",
            ),
            (
                StorybookAutomationError::InvalidCaptureRequest {
                    message: "invalid".to_string(),
                },
                "invalid",
            ),
        ];

        for (error, expected) in errors {
            assert_eq!(error.to_string(), expected);
        }

        let successful = Ok(StoryCaptureSnapshot {
            request_id: 1,
            path: PathBuf::from("capture.png"),
            pixel_width: 1,
            pixel_height: 1,
            story: sample_story("crate-ButtonStory", "Button"),
        });
        assert_eq!(capture_exit_code(&successful), 0);
        assert_eq!(
            capture_exit_code(&Err(StorybookAutomationError::CaptureUnavailable {
                message: "unavailable".to_string(),
            })),
            1
        );
    }

    #[cfg(feature = "capture")]
    #[test]
    fn image_crop_rect_scales_clamps_and_rejects_empty_regions() {
        let image = image::RgbaImage::new(200, 100);
        let window_size = gpui::size(px(100.), px(50.));

        assert_eq!(
            image_crop_rect(
                Bounds {
                    origin: point(px(10.), px(5.)),
                    size: gpui::size(px(40.), px(20.)),
                },
                window_size,
                &image,
            ),
            Some((20, 10, 80, 40))
        );
        assert_eq!(
            image_crop_rect(
                Bounds {
                    origin: point(px(-10.), px(-5.)),
                    size: gpui::size(px(200.), px(100.)),
                },
                window_size,
                &image,
            ),
            Some((0, 0, 200, 100))
        );
        assert_eq!(
            image_crop_rect(Bounds::default(), window_size, &image),
            None
        );
        assert_eq!(
            image_crop_rect(Bounds::default(), gpui::size(px(0.), px(50.)), &image,),
            None
        );
        assert_eq!(
            image_crop_rect(Bounds::default(), window_size, &image::RgbaImage::new(0, 0)),
            None
        );
    }
}
