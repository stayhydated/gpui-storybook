//! Live-window automation shared by gallery, dock, and MCP integrations.
//!
//! [`StorybookAutomation`] serializes navigation, control mutation, capture,
//! and [`StoryInteractionRequest`] batches through one exclusive operation
//! guard. Story and current-route reads, control reads, and action discovery do
//! not acquire the guard; callers may use them while a mutation is active, but
//! they can observe an intermediate rendered state.
//!
//! Interaction requests are completely validated and their keystrokes and
//! registered actions are constructed before input dispatch. The shared
//! frame-aware executor resolves fresh story or substory capture bounds after
//! route preparation and story-region sizing, constrains pointer input to those bounds,
//! honors explicit rendered-frame waits, and performs an optional capture in
//! the same operation. Runtime failures after dispatch report partial progress
//! and must not be retried automatically.
//!
//! This controller uses the application's normal platform window. Linux
//! automation runners can provide a Wayland compositor with Sway's wlroots
//! headless backend; the MCP launch helper generates that platform wrapper.

#[cfg(feature = "capture")]
use crate::capture_output::CaptureOutputStore;
pub(crate) mod interaction;

pub use crate::capture_region::{
    StoryInteractionTargetBounds, StoryInteractionTargetSnapshot, StorySemanticValueSnapshot,
};
pub use crate::story::{StoryScenario, StoryScenarioSnapshot, StoryScenarioStep};
use crate::{
    capture_region::{
        InteractionTargetLookupError, SemanticValueLookupError, capture_region_bounds,
        capture_route_story_key, interaction_targets, scroll_capture_region_into_view,
        semantic_values,
    },
    controls::{ControlSnapshot, ControlValue},
    presentation::StoryViewportPreset,
    story::StoryContainer,
};
use gpui::{App, Entity, Global, Window, px};
#[cfg(feature = "capture")]
use gpui::{Bounds, Pixels, point};
pub use interaction::{
    DEFAULT_INTERACTION_POSTCONDITION_FRAMES, MAX_INTERACTION_POSTCONDITIONS,
    MAX_INTERACTION_STEPS, MAX_INTERACTION_TEXT_BYTES, MAX_INTERACTION_WAITED_FRAMES,
    StoryActionSnapshot, StoryInteractionCaptureRequest, StoryInteractionDispatch,
    StoryInteractionObservation, StoryInteractionPostcondition,
    StoryInteractionPostconditionSnapshot, StoryInteractionRequest, StoryInteractionSnapshot,
    StoryInteractionStep, StoryModifier, StoryModifiers, StoryMouseButton, StoryPoint,
    StoryPointSpace,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::{
    borrow::Borrow,
    collections::BTreeMap,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
};
use thiserror::Error;
use tokio::sync::{mpsc, oneshot, watch};

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

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[schemars(deny_unknown_fields)]
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
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[schemars(deny_unknown_fields)]
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
    /// Reusable interaction scenarios declared by this story.
    #[serde(default)]
    pub scenarios: Vec<StoryScenarioSnapshot>,
}

/// Scenario descriptors available for one selected story.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[schemars(deny_unknown_fields)]
pub struct StoryScenariosSnapshot {
    /// Story that owns the listed scenarios.
    pub story: StorySnapshot,
    /// Stable scenario descriptors in declaration order.
    pub scenarios: Vec<StoryScenarioSnapshot>,
}

/// Completed result for one story-owned interaction scenario.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[schemars(deny_unknown_fields)]
pub struct StoryScenarioRunSnapshot {
    /// Scenario descriptor used to create the fresh interaction request.
    pub scenario: StoryScenarioSnapshot,
    /// Shared interaction executor result, including observations and capture.
    pub interaction: StoryInteractionSnapshot,
}

#[derive(Clone, Debug, Default, Deserialize, JsonSchema, PartialEq, Serialize)]
#[schemars(deny_unknown_fields)]
pub struct StoryCurrentSnapshot {
    pub story: Option<StorySnapshot>,
    pub revision: u64,
}

#[derive(Clone, Debug, Default, Deserialize, JsonSchema, PartialEq, Serialize)]
#[schemars(deny_unknown_fields)]
pub struct StoryScreenshotRequest {
    /// PNG destination, or the route-derived default when omitted.
    pub output_path: Option<PathBuf>,
    /// Requested captured story-region width in physical pixels.
    pub width: Option<u32>,
    /// Requested captured story-region height in physical pixels.
    pub height: Option<u32>,
    /// Named viewport used when explicit dimensions are omitted.
    pub viewport: Option<StoryViewportPreset>,
    /// Serialized controls to apply to the current story before capture.
    #[serde(default)]
    pub controls: BTreeMap<String, ControlValue>,
    #[serde(default)]
    pub quit_after_capture: bool,
}

/// Current values and metadata for the controls on the selected story instance.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[schemars(deny_unknown_fields)]
pub struct StoryControlsSnapshot {
    pub story: StorySnapshot,
    pub controls: Vec<ControlSnapshot>,
}

/// Semantic interaction targets currently rendered by the selected route.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[schemars(deny_unknown_fields)]
pub struct StoryInteractionTargetsSnapshot {
    /// Story or substory route whose rendered targets were inspected.
    pub story: StorySnapshot,
    /// Stable targets in deterministic key order.
    pub targets: Vec<StoryInteractionTargetSnapshot>,
}

/// Machine-readable values currently rendered by the selected story route.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[schemars(deny_unknown_fields)]
pub struct StorySemanticValuesSnapshot {
    /// Story or substory route whose values were read.
    pub story: StorySnapshot,
    /// Stable values in deterministic key order.
    pub values: Vec<StorySemanticValueSnapshot>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[schemars(deny_unknown_fields)]
pub struct StoryCaptureSnapshot {
    pub request_id: u64,
    pub path: PathBuf,
    pub pixel_width: u32,
    pub pixel_height: u32,
    pub story: StorySnapshot,
}

/// Structured live-host, validation, control, interaction, and capture errors.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum StorybookAutomationError {
    /// The standard gallery or dock did not finish publishing and attaching its
    /// live automation host within the MCP startup deadline.
    #[error("GPUI storybook automation did not become ready within {seconds} seconds")]
    StartupTimedOut {
        /// Bounded startup wait in seconds.
        seconds: u64,
    },
    /// No gallery or dock window has attached the automation command receiver.
    #[error("no live GPUI storybook host is attached")]
    NoLiveHost,
    /// The live host disappeared while a request was awaiting completion.
    #[error(
        "live GPUI storybook host disconnected after {steps_dispatched} dispatched step(s): {message}"
    )]
    HostDisconnected {
        /// Oneshot or host failure detail.
        message: String,
        /// Interaction steps completed before disconnection.
        steps_dispatched: usize,
    },
    /// A requested stable story or substory route is unknown.
    #[error("story route `{key}` was not found")]
    StoryNotFound {
        /// Requested route.
        key: String,
    },
    /// Another navigation, control mutation, capture, or batch owns the guard.
    #[error("another storybook automation mutation is already active")]
    AutomationBusy,
    /// The active route could not be rendered or captured.
    #[error("{message}")]
    CaptureUnavailable {
        /// Capture failure detail.
        message: String,
    },
    /// Capture dimensions or viewport input is invalid.
    #[error("{message}")]
    InvalidCaptureRequest {
        /// Validation detail.
        message: String,
    },
    /// Batch-level interaction input is invalid.
    #[error("{message}")]
    InvalidInteractionRequest {
        /// Validation detail.
        message: String,
    },
    /// One indexed interaction step is invalid.
    #[error("interaction step {step_index} is invalid: {message}")]
    InvalidInteractionStep {
        /// Zero-based request step index.
        step_index: usize,
        /// Validation detail.
        message: String,
    },
    /// One indexed semantic postcondition is invalid before dispatch.
    #[error("interaction postcondition {postcondition_index} is invalid: {message}")]
    InvalidInteractionPostcondition {
        /// Zero-based postcondition index.
        postcondition_index: usize,
        /// Validation detail.
        message: String,
    },
    /// A runtime failure occurred after the batch runner started.
    #[error(
        "interaction request {request_id} failed after {steps_dispatched} dispatched step(s): {message}"
    )]
    InteractionFailed {
        /// Controller-assigned interaction request ID.
        request_id: u64,
        /// Steps completed before the runtime failure.
        steps_dispatched: usize,
        /// Runtime failure detail.
        message: String,
    },
    /// The live host has no selected story instance.
    #[error("no story is selected in the live host")]
    NoActiveStory,
    /// The selected story instance has no typed control target.
    #[error("story `{key}` does not expose controls")]
    ControlsUnavailable {
        /// Active story route.
        key: String,
    },
    /// A typed control target rejected a read or mutation.
    #[error("{message}")]
    ControlOperationFailed {
        /// Control failure detail.
        message: String,
    },
    /// The active story route has not rendered semantic target bounds.
    #[error("interaction targets are unavailable because route `{route}` is not rendered")]
    InteractionTargetsUnavailable {
        /// Active story or substory route.
        route: String,
    },
    /// A semantic target key is not present in the active route.
    #[error("interaction target `{key}` was not found in route `{route}`")]
    InteractionTargetNotFound {
        /// Active story or substory route.
        route: String,
        /// Requested stable target key.
        key: String,
    },
    /// A story rendered the same semantic target key more than once.
    #[error("interaction target `{key}` is duplicated in route `{route}`")]
    DuplicateInteractionTarget {
        /// Active story or substory route.
        route: String,
        /// Duplicated stable target key.
        key: String,
    },
    /// The active story route has not rendered semantic values.
    #[error("semantic values are unavailable because route `{route}` is not rendered")]
    SemanticValuesUnavailable {
        /// Active story or substory route.
        route: String,
    },
    /// A semantic value key is not present in the active route.
    #[error("semantic value `{key}` was not found in route `{route}`")]
    SemanticValueNotFound {
        /// Active story or substory route.
        route: String,
        /// Requested stable value key.
        key: String,
    },
    /// A semantic value did not match the requested JSON value within the
    /// bounded number of refreshed frames.
    #[error("semantic value `{key}` in route `{route}` did not match within {max_frames} frame(s)")]
    SemanticValueWaitTimedOut {
        /// Active story or substory route.
        route: String,
        /// Requested stable value key.
        key: String,
        /// Maximum refreshed frames requested by the caller.
        max_frames: u16,
    },
    /// A story rendered the same semantic value key more than once.
    #[error("semantic value `{key}` is duplicated in route `{route}`")]
    DuplicateSemanticValue {
        /// Active story or substory route.
        route: String,
        /// Duplicated stable value key.
        key: String,
    },
    /// A requested story scenario is not declared by that story.
    #[error("scenario `{scenario_key}` was not found in story `{story_key}`")]
    ScenarioNotFound {
        /// Story owning the requested scenario.
        story_key: String,
        /// Requested stable scenario key.
        scenario_key: String,
    },
    /// A story declared the same scenario key more than once.
    #[error("scenario `{scenario_key}` is duplicated in story `{story_key}`")]
    DuplicateScenarioKey {
        /// Story owning the duplicate key.
        story_key: String,
        /// Duplicated stable scenario key.
        scenario_key: String,
    },
}

pub(crate) enum StorybookAutomationCommand {
    OpenStory {
        key: String,
        response: oneshot::Sender<Result<StoryCurrentSnapshot, StorybookAutomationError>>,
        _operation: AutomationOperationGuard,
    },
    CaptureCurrentStory {
        request_id: u64,
        request: StoryScreenshotRequest,
        response: oneshot::Sender<Result<StoryCaptureSnapshot, StorybookAutomationError>>,
        operation: AutomationOperationGuard,
    },
    ReadControls {
        response: oneshot::Sender<Result<StoryControlsSnapshot, StorybookAutomationError>>,
    },
    SetControl {
        key: String,
        value: ControlValue,
        response: oneshot::Sender<Result<StoryControlsSnapshot, StorybookAutomationError>>,
        _operation: AutomationOperationGuard,
    },
    ResetControl {
        key: Option<String>,
        response: oneshot::Sender<Result<StoryControlsSnapshot, StorybookAutomationError>>,
        _operation: AutomationOperationGuard,
    },
    ListActions {
        response: oneshot::Sender<Result<Vec<StoryActionSnapshot>, StorybookAutomationError>>,
    },
    ListInteractionTargets {
        response:
            oneshot::Sender<Result<StoryInteractionTargetsSnapshot, StorybookAutomationError>>,
    },
    ReadSemanticValues {
        response: oneshot::Sender<Result<StorySemanticValuesSnapshot, StorybookAutomationError>>,
    },
    RunSteps {
        request_id: u64,
        request: StoryInteractionRequest,
        /// Recreate the concrete story entity before preparing this batch.
        /// Scenario runs set this so every invocation starts at constructor
        /// defaults; ordinary ad-hoc interaction batches preserve their
        /// existing state.
        fresh_story: bool,
        response: oneshot::Sender<Result<StoryInteractionSnapshot, StorybookAutomationError>>,
        progress: Arc<std::sync::atomic::AtomicUsize>,
        operation: AutomationOperationGuard,
    },
}

pub(crate) struct AutomationOperationGuard {
    pending: Arc<AtomicBool>,
}

impl Drop for AutomationOperationGuard {
    fn drop(&mut self) {
        self.pending.store(false, Ordering::SeqCst);
    }
}

#[derive(Debug, Default)]
struct StorybookAutomationState {
    stories: Vec<StorySnapshot>,
    current_story_key: Option<String>,
    revision: u64,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct StorybookAutomationReadiness {
    catalog_published: bool,
    live_host_attached: bool,
}

impl StorybookAutomationReadiness {
    const fn ready(self) -> bool {
        self.catalog_published && self.live_host_attached
    }
}

pub struct StorybookAutomation {
    state: Mutex<StorybookAutomationState>,
    command_tx: mpsc::UnboundedSender<StorybookAutomationCommand>,
    command_rx: Mutex<Option<mpsc::UnboundedReceiver<StorybookAutomationCommand>>>,
    live_host_attached: AtomicBool,
    startup_wait_required: AtomicBool,
    readiness_tx: watch::Sender<StorybookAutomationReadiness>,
    operation_pending: Arc<AtomicBool>,
    next_request_id: AtomicU64,
}

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
            scenarios: story.scenarios().to_vec(),
        })
    }
}

impl StorybookAutomation {
    pub fn new() -> SharedStorybookAutomation {
        Self::build(Vec::new(), true)
    }

    pub fn with_stories(stories: Vec<StorySnapshot>) -> SharedStorybookAutomation {
        Self::build(stories, false)
    }

    fn build(
        stories: Vec<StorySnapshot>,
        startup_wait_required: bool,
    ) -> SharedStorybookAutomation {
        let (command_tx, command_rx) = mpsc::unbounded_channel();
        let current_story_key = stories.first().map(|story| story.key.clone());
        let (readiness_tx, _) = watch::channel(StorybookAutomationReadiness {
            catalog_published: !startup_wait_required,
            live_host_attached: false,
        });

        Arc::new(Self {
            state: Mutex::new(StorybookAutomationState {
                stories,
                current_story_key,
                revision: 0,
            }),
            command_tx,
            command_rx: Mutex::new(Some(command_rx)),
            live_host_attached: AtomicBool::new(false),
            startup_wait_required: AtomicBool::new(startup_wait_required),
            readiness_tx,
            operation_pending: Arc::new(AtomicBool::new(false)),
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
        drop(state);
        self.update_readiness(|readiness| readiness.catalog_published = true);
    }

    /// Wait until the standard gallery or dock has published its catalog and
    /// attached the live automation command receiver.
    ///
    /// Controllers built with [`with_stories`](Self::with_stories) are treated
    /// as explicitly configured low-level integrations and return immediately.
    /// The MCP layer applies its own bounded timeout around this future.
    pub async fn wait_until_ready(&self) {
        if !self.startup_wait_required.load(Ordering::SeqCst) {
            return;
        }

        let mut readiness = self.readiness_tx.subscribe();
        loop {
            if readiness.borrow_and_update().ready() {
                self.startup_wait_required.store(false, Ordering::SeqCst);
                return;
            }
            if readiness.changed().await.is_err() {
                return;
            }
        }
    }

    fn update_readiness(&self, update: impl FnOnce(&mut StorybookAutomationReadiness)) {
        self.readiness_tx.send_modify(|readiness| {
            update(readiness);
            if readiness.ready() {
                self.startup_wait_required.store(false, Ordering::SeqCst);
            }
        });
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

    /// Lists scenarios declared by the currently selected story.
    pub fn list_scenarios(&self) -> Result<StoryScenariosSnapshot, StorybookAutomationError> {
        let story = self
            .current_story()
            .story
            .ok_or(StorybookAutomationError::NoActiveStory)?;
        Ok(StoryScenariosSnapshot {
            scenarios: story.scenarios.clone(),
            story,
        })
    }

    /// Lists scenarios declared by a registered story or sub-story route.
    pub fn list_scenarios_for(
        &self,
        key: &str,
    ) -> Result<StoryScenariosSnapshot, StorybookAutomationError> {
        let story = self.get_story(key)?;
        Ok(StoryScenariosSnapshot {
            scenarios: story.scenarios.clone(),
            story,
        })
    }

    /// Runs one declared scenario as a fresh, exclusive interaction request.
    ///
    /// The scenario is converted to [`StoryInteractionRequest`] and delegated
    /// to the same executor as [`Self::run_steps`]. The live command marks this
    /// request for concrete story recreation before controls and dispatch. A
    /// failed request reports dispatched progress and is never resumed or
    /// retried by this method.
    pub async fn run_scenario(
        &self,
        story_key: Option<String>,
        scenario_key: impl Into<String>,
    ) -> Result<StoryScenarioRunSnapshot, StorybookAutomationError> {
        let scenario_key = scenario_key.into();
        let story = match story_key {
            Some(key) => self.get_story(&key)?,
            None => self
                .current_story()
                .story
                .ok_or(StorybookAutomationError::NoActiveStory)?,
        };
        let scenario = find_scenario(&story, &scenario_key)?;
        let interaction = self
            .run_steps_with_options(
                scenario.interaction_request(story.capture_route_id.clone()),
                true,
            )
            .await?;
        Ok(StoryScenarioRunSnapshot {
            scenario,
            interaction,
        })
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

        let operation = self.begin_operation()?;
        let (response, receiver) = oneshot::channel();
        self.command_tx
            .send(StorybookAutomationCommand::OpenStory {
                key,
                response,
                _operation: operation,
            })
            .map_err(|_| StorybookAutomationError::NoLiveHost)?;

        receiver
            .await
            .map_err(|error| StorybookAutomationError::HostDisconnected {
                message: error.to_string(),
                steps_dispatched: 0,
            })?
    }

    pub async fn capture_current_story(
        &self,
        request: StoryScreenshotRequest,
    ) -> Result<StoryCaptureSnapshot, StorybookAutomationError> {
        if !self.live_host_attached.load(Ordering::SeqCst) {
            return Err(StorybookAutomationError::NoLiveHost);
        }

        let operation = self.begin_operation()?;

        let request_id = self.next_request_id.fetch_add(1, Ordering::SeqCst);
        let (response, receiver) = oneshot::channel();
        self.command_tx
            .send(StorybookAutomationCommand::CaptureCurrentStory {
                request_id,
                request,
                response,
                operation,
            })
            .map_err(|_| StorybookAutomationError::NoLiveHost)?;

        receiver
            .await
            .map_err(|error| StorybookAutomationError::HostDisconnected {
                message: error.to_string(),
                steps_dispatched: 0,
            })?
    }

    /// Reads controls from the concrete story entity displayed by the live host.
    pub async fn read_controls(&self) -> Result<StoryControlsSnapshot, StorybookAutomationError> {
        let (response, receiver) = self.live_command_channel()?;
        self.command_tx
            .send(StorybookAutomationCommand::ReadControls { response })
            .map_err(|_| StorybookAutomationError::NoLiveHost)?;
        receive_host_response(receiver).await
    }

    /// Updates one control on the concrete story entity displayed by the live host.
    pub async fn set_control(
        &self,
        key: impl Into<String>,
        value: ControlValue,
    ) -> Result<StoryControlsSnapshot, StorybookAutomationError> {
        let (response, receiver) = self.live_command_channel()?;
        let operation = self.begin_operation()?;
        self.command_tx
            .send(StorybookAutomationCommand::SetControl {
                key: key.into(),
                value,
                response,
                _operation: operation,
            })
            .map_err(|_| StorybookAutomationError::NoLiveHost)?;
        receive_host_response(receiver).await
    }

    /// Resets one control, or every control when `key` is `None`.
    pub async fn reset_control(
        &self,
        key: Option<String>,
    ) -> Result<StoryControlsSnapshot, StorybookAutomationError> {
        let (response, receiver) = self.live_command_channel()?;
        let operation = self.begin_operation()?;
        self.command_tx
            .send(StorybookAutomationCommand::ResetControl {
                key,
                response,
                _operation: operation,
            })
            .map_err(|_| StorybookAutomationError::NoLiveHost)?;
        receive_host_response(receiver).await
    }

    /// Lists the non-internal GPUI actions registered in the live application.
    ///
    /// Action registrations are runtime state. Clients should rediscover them
    /// for each application launch before constructing an interaction batch.
    pub async fn list_actions(&self) -> Result<Vec<StoryActionSnapshot>, StorybookAutomationError> {
        let (response, receiver) = self.live_command_channel()?;
        self.command_tx
            .send(StorybookAutomationCommand::ListActions { response })
            .map_err(|_| StorybookAutomationError::NoLiveHost)?;
        receive_host_response(receiver).await
    }

    /// Lists stable semantic targets rendered by the selected story route.
    pub async fn list_interaction_targets(
        &self,
    ) -> Result<StoryInteractionTargetsSnapshot, StorybookAutomationError> {
        let (response, receiver) = self.live_command_channel()?;
        self.command_tx
            .send(StorybookAutomationCommand::ListInteractionTargets { response })
            .map_err(|_| StorybookAutomationError::NoLiveHost)?;
        receive_host_response(receiver).await
    }

    /// Reads stable machine-readable values rendered by the selected route.
    pub async fn read_semantic_values(
        &self,
    ) -> Result<StorySemanticValuesSnapshot, StorybookAutomationError> {
        let (response, receiver) = self.live_command_channel()?;
        self.command_tx
            .send(StorybookAutomationCommand::ReadSemanticValues { response })
            .map_err(|_| StorybookAutomationError::NoLiveHost)?;
        receive_host_response(receiver).await
    }

    /// Runs one validated interaction batch on the live GPUI window thread.
    ///
    /// The batch owns the shared operation guard through every frame wait and
    /// optional capture. Canceling the receiver is observed at executor safety
    /// boundaries; already dispatched clicks and actions are never retried.
    pub async fn run_steps(
        &self,
        request: StoryInteractionRequest,
    ) -> Result<StoryInteractionSnapshot, StorybookAutomationError> {
        self.run_steps_with_options(request, false).await
    }

    async fn run_steps_with_options(
        &self,
        request: StoryInteractionRequest,
        fresh_story: bool,
    ) -> Result<StoryInteractionSnapshot, StorybookAutomationError> {
        interaction::validate_interaction_request(&request)?;
        let (response, receiver) = self.live_command_channel()?;
        let operation = self.begin_operation()?;
        let request_id = self.next_request_id.fetch_add(1, Ordering::SeqCst);
        let progress = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        self.command_tx
            .send(StorybookAutomationCommand::RunSteps {
                request_id,
                request,
                fresh_story,
                response,
                progress: progress.clone(),
                operation,
            })
            .map_err(|_| StorybookAutomationError::NoLiveHost)?;

        receiver
            .await
            .map_err(|error| StorybookAutomationError::HostDisconnected {
                message: error.to_string(),
                steps_dispatched: progress.load(Ordering::SeqCst),
            })?
    }

    pub(crate) fn begin_operation(
        &self,
    ) -> Result<AutomationOperationGuard, StorybookAutomationError> {
        self.operation_pending
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .map_err(|_| StorybookAutomationError::AutomationBusy)?;
        Ok(AutomationOperationGuard {
            pending: self.operation_pending.clone(),
        })
    }

    fn live_command_channel<T>(
        &self,
    ) -> Result<
        (
            oneshot::Sender<Result<T, StorybookAutomationError>>,
            oneshot::Receiver<Result<T, StorybookAutomationError>>,
        ),
        StorybookAutomationError,
    > {
        if !self.live_host_attached.load(Ordering::SeqCst) {
            return Err(StorybookAutomationError::NoLiveHost);
        }
        Ok(oneshot::channel())
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
            self.update_readiness(|readiness| readiness.live_host_attached = true);
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

pub(crate) fn rendered_interaction_targets(
    story: StorySnapshot,
) -> Result<StoryInteractionTargetsSnapshot, StorybookAutomationError> {
    let route = story.capture_route_id.clone();
    let targets = interaction_targets(&route).map_err(|error| match error {
        InteractionTargetLookupError::RouteNotRendered => {
            StorybookAutomationError::InteractionTargetsUnavailable {
                route: route.clone(),
            }
        },
        InteractionTargetLookupError::DuplicateKey(key) => {
            StorybookAutomationError::DuplicateInteractionTarget {
                route: route.clone(),
                key,
            }
        },
    })?;
    Ok(StoryInteractionTargetsSnapshot { story, targets })
}

pub(crate) fn rendered_semantic_values(
    story: StorySnapshot,
) -> Result<StorySemanticValuesSnapshot, StorybookAutomationError> {
    let route = story.capture_route_id.clone();
    let values = semantic_values(&route).map_err(|error| match error {
        SemanticValueLookupError::RouteNotRendered => {
            StorybookAutomationError::SemanticValuesUnavailable {
                route: route.clone(),
            }
        },
        SemanticValueLookupError::DuplicateKey(key) => {
            StorybookAutomationError::DuplicateSemanticValue {
                route: route.clone(),
                key,
            }
        },
    })?;
    Ok(StorySemanticValuesSnapshot { story, values })
}

pub(crate) fn schedule_semantic_value_read(
    story: StorySnapshot,
    response: oneshot::Sender<Result<StorySemanticValuesSnapshot, StorybookAutomationError>>,
    window: &mut Window,
) {
    window.refresh();
    window.on_next_frame(move |_window, _cx| {
        let _ = response.send(rendered_semantic_values(story));
    });
}

async fn receive_host_response<T>(
    receiver: oneshot::Receiver<Result<T, StorybookAutomationError>>,
) -> Result<T, StorybookAutomationError> {
    receiver
        .await
        .map_err(|error| StorybookAutomationError::HostDisconnected {
            message: error.to_string(),
            steps_dispatched: 0,
        })?
}

fn resolve_story_route(stories: &[StorySnapshot], route_id: &str) -> Option<StorySnapshot> {
    let story_key = capture_route_story_key(route_id);
    let story = stories
        .iter()
        .find(|story| story.key == story_key || story.capture_route_id == story_key)?;

    Some(story_snapshot_for_route(story.clone(), route_id))
}

fn find_scenario(
    story: &StorySnapshot,
    scenario_key: &str,
) -> Result<StoryScenarioSnapshot, StorybookAutomationError> {
    let mut matches = story
        .scenarios
        .iter()
        .filter(|scenario| scenario.key == scenario_key);
    let Some(scenario) = matches.next() else {
        return Err(StorybookAutomationError::ScenarioNotFound {
            story_key: story.key.clone(),
            scenario_key: scenario_key.to_owned(),
        });
    };
    if matches.next().is_some() {
        return Err(StorybookAutomationError::DuplicateScenarioKey {
            story_key: story.key.clone(),
            scenario_key: scenario_key.to_owned(),
        });
    }
    Ok(scenario.clone())
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
    operation: AutomationOperationGuard,
    quit_after_capture: bool,
    window: &mut Window,
) {
    if response.is_closed() {
        return;
    }
    window.on_next_frame(move |window, cx| {
        if response.is_closed() {
            return;
        }
        let resized = match ensure_capture_target_visible(&story.capture_route_id, window) {
            Ok(resized) => resized,
            Err(error) => {
                let result = Err(error);
                let exit_code = capture_exit_code(&result);
                let _ = response.send(result);
                if quit_after_capture {
                    exit_after_capture(exit_code, cx);
                }
                return;
            },
        };
        if resized {
            window.refresh();
            window.on_next_frame(move |window, _cx| {
                prepare_story_capture(
                    request_id,
                    request,
                    story,
                    response,
                    operation,
                    quit_after_capture,
                    window,
                )
            });
        } else {
            prepare_story_capture(
                request_id,
                request,
                story,
                response,
                operation,
                quit_after_capture,
                window,
            );
        }
    });
}

fn prepare_story_capture(
    request_id: u64,
    request: StoryScreenshotRequest,
    story: StorySnapshot,
    response: oneshot::Sender<Result<StoryCaptureSnapshot, StorybookAutomationError>>,
    operation: AutomationOperationGuard,
    quit_after_capture: bool,
    window: &mut Window,
) {
    if response.is_closed() {
        return;
    }
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
    window.on_next_frame(move |window, cx| {
        let _operation = operation;
        let result = render_story_capture(request_id, request, story, window);
        let exit_code = capture_exit_code(&result);
        let _ = response.send(result);
        if quit_after_capture {
            exit_after_capture(exit_code, cx);
        }
    });
}

#[cfg(unix)]
fn exit_after_capture(exit_code: i32, _cx: &mut App) -> ! {
    // SAFETY: startup capture owns the process and has completed its output
    // write. `_exit` avoids native platform teardown callbacks after that point.
    unsafe { libc::_exit(exit_code) }
}

#[cfg(not(unix))]
fn exit_after_capture(exit_code: i32, cx: &mut App) {
    if exit_code == 0 {
        cx.quit();
    } else {
        std::process::exit(exit_code);
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
        (None, None) => Ok(request.viewport.and_then(StoryViewportPreset::dimensions)),
        _ => Err(StorybookAutomationError::InvalidCaptureRequest {
            message: "capture width and height must be provided together".to_string(),
        }),
    }
}

pub(crate) fn set_capture_target_size(
    story: &Entity<StoryContainer>,
    window: &Window,
    target_size: Option<(u32, u32)>,
    cx: &mut App,
) {
    let scale_factor = window.scale_factor().max(f32::EPSILON);
    let size = target_size.map(|(width, height)| {
        gpui::size(
            px(width as f32 / scale_factor),
            px(height as f32 / scale_factor),
        )
    });
    story.update(cx, |story, cx| {
        story.set_automation_size(size);
        cx.notify();
    });
}

pub(crate) fn ensure_capture_target_visible(
    route_id: &str,
    window: &mut Window,
) -> Result<bool, StorybookAutomationError> {
    let story_key = capture_route_story_key(route_id);
    let region = capture_region_bounds(story_key).ok_or_else(|| {
        StorybookAutomationError::CaptureUnavailable {
            message: format!(
                "capture route `{story_key}` was not rendered before validating its target size"
            ),
        }
    })?;
    let Some(target_window_size) = expanded_window_size(window.bounds().size, region.bounds) else {
        return Ok(false);
    };
    window.resize(target_window_size);
    Ok(true)
}

fn expanded_window_size(
    window_size: gpui::Size<gpui::Pixels>,
    story_region: gpui::Bounds<gpui::Pixels>,
) -> Option<gpui::Size<gpui::Pixels>> {
    let required_width =
        (f32::from(story_region.origin.x) + f32::from(story_region.size.width)).max(0.0);
    let required_height =
        (f32::from(story_region.origin.y) + f32::from(story_region.size.height)).max(0.0);
    let width = f32::from(window_size.width).max(required_width);
    let height = f32::from(window_size.height).max(required_height);
    if width == f32::from(window_size.width) && height == f32::from(window_size.height) {
        None
    } else {
        Some(gpui::size(px(width), px(height)))
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
            scenarios: Vec::new(),
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
    fn scenario_catalog_lists_stable_descriptors_and_reports_missing_keys() {
        let mut story = sample_story("crate-ButtonStory", "Button");
        story.scenarios = vec![
            StoryScenario::new("press", "Press button")
                .description("Presses the primary button.")
                .step(StoryScenarioStep::new(
                    "focus button",
                    StoryInteractionStep::FocusNext,
                )),
        ];
        let automation = StorybookAutomation::with_stories(vec![story.clone()]);

        let listed = automation
            .list_scenarios()
            .expect("current story scenarios should be listed");
        assert_eq!(listed.story, story);
        assert_eq!(listed.scenarios.len(), 1);
        assert_eq!(listed.scenarios[0].key, "press");
        let route_listing = automation
            .list_scenarios_for("crate-ButtonStory/section")
            .expect("substory scenarios should be listed");
        assert_eq!(
            route_listing.story.capture_route_id,
            "crate-ButtonStory/section"
        );
        assert_eq!(route_listing.story.title, "Button / Section");
        assert_eq!(route_listing.scenarios, listed.scenarios);

        assert_eq!(
            automation.list_scenarios_for("missing"),
            Err(StorybookAutomationError::StoryNotFound {
                key: "missing".to_owned(),
            })
        );
    }

    #[test]
    fn run_scenario_resolves_descriptor_before_requiring_live_host() {
        let mut story = sample_story("crate-ButtonStory", "Button");
        story.scenarios =
            vec![
                StoryScenario::new("press", "Press button").step(StoryScenarioStep::new(
                    "focus button",
                    StoryInteractionStep::FocusNext,
                )),
            ];
        let automation = StorybookAutomation::with_stories(vec![story]);
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("test runtime should build");

        runtime.block_on(async {
            assert_eq!(
                automation
                    .run_scenario(Some("crate-ButtonStory".to_owned()), "missing")
                    .await,
                Err(StorybookAutomationError::ScenarioNotFound {
                    story_key: "crate-ButtonStory".to_owned(),
                    scenario_key: "missing".to_owned(),
                })
            );
            assert_eq!(
                automation
                    .run_scenario(Some("crate-ButtonStory".to_owned()), "press")
                    .await,
                Err(StorybookAutomationError::NoLiveHost)
            );
        });
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

    #[test]
    fn facade_automation_waits_for_catalog_and_live_host() {
        let automation = StorybookAutomation::new();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("test runtime should build");

        runtime.block_on(async {
            let waiting = automation.clone();
            let ready = tokio::spawn(async move { waiting.wait_until_ready().await });
            tokio::task::yield_now().await;
            assert!(!ready.is_finished());

            automation.set_stories(vec![sample_story("crate-ButtonStory", "Button")]);
            tokio::task::yield_now().await;
            assert!(!ready.is_finished());

            let _receiver = automation
                .take_command_receiver()
                .expect("live host should attach once");
            ready.await.expect("readiness task should complete");
        });
    }

    #[test]
    fn explicitly_seeded_automation_skips_facade_startup_wait() {
        let automation =
            StorybookAutomation::with_stories(vec![sample_story("crate-ButtonStory", "Button")]);
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("test runtime should build");

        runtime.block_on(automation.wait_until_ready());
    }

    #[test]
    fn operation_guard_rejects_mutations_until_its_owner_drops() {
        let automation =
            StorybookAutomation::with_stories(vec![sample_story("crate-ButtonStory", "Button")]);
        let operation = automation
            .begin_operation()
            .expect("first mutation should acquire the operation guard");

        assert!(matches!(
            automation.begin_operation(),
            Err(StorybookAutomationError::AutomationBusy)
        ));
        assert_eq!(automation.stories().len(), 1, "reads remain available");

        drop(operation);
        assert!(automation.begin_operation().is_ok());
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
        assert_eq!(
            validate_capture_target_size(&StoryScreenshotRequest {
                viewport: Some(StoryViewportPreset::Mobile),
                ..StoryScreenshotRequest::default()
            }),
            Ok(Some((390, 844)))
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
    fn capture_target_size_only_expands_a_clipped_story_region() {
        let window = gpui::size(px(800.), px(600.));
        assert_eq!(
            expanded_window_size(
                window,
                gpui::Bounds {
                    origin: gpui::point(px(200.), px(100.)),
                    size: gpui::size(px(400.), px(300.)),
                },
            ),
            None
        );
        assert_eq!(
            expanded_window_size(
                window,
                gpui::Bounds {
                    origin: gpui::point(px(300.), px(150.)),
                    size: gpui::size(px(600.), px(500.)),
                },
            ),
            Some(gpui::size(px(900.), px(650.)))
        );
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
                    steps_dispatched: 2,
                },
                "live GPUI storybook host disconnected after 2 dispatched step(s): closed",
            ),
            (
                StorybookAutomationError::StoryNotFound {
                    key: "missing".to_string(),
                },
                "story route `missing` was not found",
            ),
            (
                StorybookAutomationError::AutomationBusy,
                "another storybook automation mutation is already active",
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
            (
                StorybookAutomationError::InteractionTargetsUnavailable {
                    route: "story/section".to_string(),
                },
                "interaction targets are unavailable because route `story/section` is not rendered",
            ),
            (
                StorybookAutomationError::InteractionTargetNotFound {
                    route: "story".to_string(),
                    key: "execute".to_string(),
                },
                "interaction target `execute` was not found in route `story`",
            ),
            (
                StorybookAutomationError::DuplicateInteractionTarget {
                    route: "story".to_string(),
                    key: "execute".to_string(),
                },
                "interaction target `execute` is duplicated in route `story`",
            ),
            (
                StorybookAutomationError::SemanticValuesUnavailable {
                    route: "story/section".to_string(),
                },
                "semantic values are unavailable because route `story/section` is not rendered",
            ),
            (
                StorybookAutomationError::DuplicateSemanticValue {
                    route: "story".to_string(),
                    key: "response".to_string(),
                },
                "semantic value `response` is duplicated in route `story`",
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
