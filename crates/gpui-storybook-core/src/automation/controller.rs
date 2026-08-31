use super::*;

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

pub(crate) type StorybookAutomationCommandReceiver =
    mpsc::UnboundedReceiver<StorybookAutomationCommand>;

pub(crate) struct AutomationOperationGuard {
    pub(super) pending: Arc<AtomicBool>,
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

    pub(crate) fn take_command_receiver(&self) -> Option<StorybookAutomationCommandReceiver> {
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
