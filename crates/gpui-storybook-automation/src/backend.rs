//! Object-safe asynchronous boundary implemented by each automation host.

use crate::*;
use std::{future::Future, pin::Pin, sync::Arc};

/// Future returned by a host operation. Dropping it never authorizes replay.
pub type BackendFuture<'a, T> =
    Pin<Box<dyn Future<Output = Result<T, StorybookAutomationError>> + Send + 'a>>;

/// Shared backend retained for the entire client connection.
pub type SharedAutomationBackend = Arc<dyn AutomationBackend>;

/// Independently advertised host operations and interaction families.
#[derive(
    Clone,
    Copy,
    Debug,
    serde::Deserialize,
    Eq,
    schemars::JsonSchema,
    Ord,
    PartialEq,
    PartialOrd,
    serde::Serialize,
)]
#[serde(rename_all = "snake_case")]
pub enum AutomationCapability {
    Navigation,
    Controls,
    ControlMutation,
    SemanticValues,
    Focus,
    Keystrokes,
    TextInsertion,
    Actions,
    Pointer,
    Scroll,
    FrameWaits,
    SemanticTargets,
    FreshScenarios,
    StoryCapture,
    CaptureControls,
    InteractionCapture,
    StorySizing,
    Presentation,
    DesktopLaunch,
    HostDiscovery,
    HostActions,
    DisplayCapture,
}

/// Immutable capabilities negotiated before building a client's tool catalog.
///
/// A replacement host or changed capability set requires a fresh connection.
#[derive(
    Clone, Debug, Default, serde::Deserialize, Eq, schemars::JsonSchema, PartialEq, serde::Serialize,
)]
#[serde(transparent)]
pub struct AutomationCapabilities(std::collections::BTreeSet<AutomationCapability>);

impl AutomationCapabilities {
    pub fn new(capabilities: impl IntoIterator<Item = AutomationCapability>) -> Self {
        Self(capabilities.into_iter().collect())
    }

    pub fn contains(&self, capability: AutomationCapability) -> bool {
        self.0.contains(&capability)
    }
    pub fn insert(&mut self, capability: AutomationCapability) {
        self.0.insert(capability);
    }
    pub fn iter(&self) -> impl Iterator<Item = AutomationCapability> + '_ {
        self.0.iter().copied()
    }

    pub fn require(
        &self,
        capability: AutomationCapability,
    ) -> Result<(), StorybookAutomationError> {
        if self.contains(capability) {
            Ok(())
        } else {
            Err(StorybookAutomationError::UnsupportedCapability { capability })
        }
    }

    /// Validate a complete batch before any route, control, or input dispatch.
    pub fn validate_interaction(
        &self,
        request: &StoryInteractionRequest,
    ) -> Result<(), StorybookAutomationError> {
        validate_interaction_request(request)?;
        if request.story_key.is_some() {
            self.require(AutomationCapability::Navigation)?;
        }
        if !request.controls.is_empty() {
            self.require(AutomationCapability::ControlMutation)?;
        }
        if request.width.is_some()
            || request.height.is_some()
            || request
                .viewport
                .and_then(StoryViewportPreset::dimensions)
                .is_some()
            || request
                .presentation
                .is_some_and(|presentation| presentation.viewport.dimensions().is_some())
        {
            self.require(AutomationCapability::StorySizing)?;
        }
        if request.presentation.is_some() {
            self.require(AutomationCapability::Presentation)?;
        }
        if request.capture.is_some() {
            self.require(AutomationCapability::InteractionCapture)?;
        }
        if !request.postconditions.is_empty() {
            self.require(AutomationCapability::SemanticValues)?;
        }
        for step in &request.steps {
            let capability = match step {
                StoryInteractionStep::FocusNext {}
                | StoryInteractionStep::FocusPrevious {}
                | StoryInteractionStep::Blur {} => AutomationCapability::Focus,
                StoryInteractionStep::Keystrokes { .. } => AutomationCapability::Keystrokes,
                StoryInteractionStep::Text { .. } => AutomationCapability::TextInsertion,
                StoryInteractionStep::DispatchAction { .. } => AutomationCapability::Actions,
                StoryInteractionStep::PointerMove { .. }
                | StoryInteractionStep::PointerClick { .. } => AutomationCapability::Pointer,
                StoryInteractionStep::ClickTarget { .. } => {
                    self.require(AutomationCapability::Pointer)?;
                    AutomationCapability::SemanticTargets
                },
                StoryInteractionStep::Scroll { .. } => AutomationCapability::Scroll,
                StoryInteractionStep::WaitFrames { .. } => AutomationCapability::FrameWaits,
            };
            self.require(capability)?;
        }
        Ok(())
    }

    pub fn validate_capture(
        &self,
        request: &StoryScreenshotRequest,
    ) -> Result<(), StorybookAutomationError> {
        self.require(AutomationCapability::StoryCapture)?;
        if request.quit_after_capture {
            self.require(AutomationCapability::DesktopLaunch)?;
        }
        if validate_capture_target_size(request)?.is_some() {
            self.require(AutomationCapability::StorySizing)?;
        }
        if !request.controls.is_empty() {
            self.require(AutomationCapability::ControlMutation)?;
            self.require(AutomationCapability::CaptureControls)?;
        }
        Ok(())
    }
}

/// Host-owned automation over registered public application surfaces.
///
/// Implementations dispatch to the application's existing owner thread. One
/// host-side exclusive operation covers navigation, mutations, captures, and
/// scenarios, including all rendered-frame waits. Reads may observe intermediate
/// state. Validate complete batches before dispatch and retain ownership until
/// submitted work settles, even when its client future is canceled.
///
/// A timeout or disconnect after submission must report actual known progress
/// or an unknown outcome; clients must never retry automatically. Scenarios
/// construct fresh fixtures; ad-hoc navigation and interactions preserve state.
pub trait AutomationBackend: Send + Sync + 'static {
    fn capabilities(&self) -> AutomationCapabilities;
    fn wait_until_ready(&self) -> BackendFuture<'_, ()>;
    fn host(&self) -> BackendFuture<'_, wire::HostDescriptor> {
        Box::pin(async {
            Err(StorybookAutomationError::UnsupportedCapability {
                capability: AutomationCapability::HostDiscovery,
            })
        })
    }
    fn list_host_actions(&self) -> BackendFuture<'_, Vec<wire::HostActionDescriptor>> {
        Box::pin(async {
            Err(StorybookAutomationError::UnsupportedCapability {
                capability: AutomationCapability::HostActions,
            })
        })
    }
    fn dispatch_host_action(&self, _: wire::HostAction) -> BackendFuture<'_, wire::HostDescriptor> {
        Box::pin(async {
            Err(StorybookAutomationError::UnsupportedCapability {
                capability: AutomationCapability::HostActions,
            })
        })
    }
    fn capture_host(
        &self,
        _: wire::HostCaptureScope,
        _: std::path::PathBuf,
    ) -> BackendFuture<'_, wire::HostCaptureSnapshot> {
        Box::pin(async {
            Err(StorybookAutomationError::UnsupportedCapability {
                capability: AutomationCapability::DisplayCapture,
            })
        })
    }
    fn stories(&self) -> BackendFuture<'_, Vec<StorySnapshot>>;
    fn get_story(&self, key: String) -> BackendFuture<'_, StorySnapshot>;
    fn current_story(&self) -> BackendFuture<'_, StoryCurrentSnapshot>;
    fn list_scenarios(
        &self,
        story_key: Option<String>,
    ) -> BackendFuture<'_, StoryScenariosSnapshot>;
    fn open_story(&self, key: String) -> BackendFuture<'_, StoryCurrentSnapshot>;
    fn read_controls(&self) -> BackendFuture<'_, StoryControlsSnapshot>;
    fn set_control(
        &self,
        key: String,
        value: ControlValue,
    ) -> BackendFuture<'_, StoryControlsSnapshot>;
    fn reset_control(&self, key: Option<String>) -> BackendFuture<'_, StoryControlsSnapshot>;
    fn list_actions(&self) -> BackendFuture<'_, Vec<StoryActionSnapshot>>;
    fn list_interaction_targets(&self) -> BackendFuture<'_, StoryInteractionTargetsSnapshot>;
    /// Resolve values after a newly rendered frame, for bounded semantic waits.
    fn read_semantic_values(&self) -> BackendFuture<'_, StorySemanticValuesSnapshot>;
    fn run_steps(
        &self,
        request: StoryInteractionRequest,
    ) -> BackendFuture<'_, StoryInteractionSnapshot>;
    fn run_scenario(
        &self,
        story_key: Option<String>,
        scenario_key: String,
    ) -> BackendFuture<'_, StoryScenarioRunSnapshot>;
    fn capture_current_story(
        &self,
        request: StoryScreenshotRequest,
    ) -> BackendFuture<'_, StoryCaptureSnapshot>;
}
