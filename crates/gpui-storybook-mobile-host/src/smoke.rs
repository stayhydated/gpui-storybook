//! Configurable consumer checks over the maintained device backend.

use crate::*;
use gpui_storybook_toml::AndroidApplication;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Application-owned fixture expectations. Steps and scenarios execute once;
/// semantic postconditions repeat only freshly rendered observations.
#[derive(Clone, Debug, Default, Deserialize, Serialize, bon::Builder)]
#[serde(deny_unknown_fields)]
pub struct SmokePlan {
    route: Option<String>,
    scenario: Option<String>,
    #[serde(default)]
    #[builder(default)]
    steps: Vec<StoryInteractionStep>,
    #[serde(default)]
    #[builder(default)]
    postconditions: Vec<StoryInteractionPostcondition>,
}

impl SmokePlan {
    pub fn load(path: impl AsRef<Path>) -> Result<Self, StorybookAutomationError> {
        let path = path.as_ref();
        let metadata = std::fs::metadata(path).map_err(|error| unavailable(error.to_string()))?;
        if !metadata.is_file() || metadata.len() > 128 * 1024 {
            return Err(unavailable(
                "smoke plan must be a regular file of at most 128 KiB",
            ));
        }
        let source =
            std::fs::read_to_string(path).map_err(|error| unavailable(error.to_string()))?;
        serde_json::from_str(&source)
            .map_err(|error| unavailable(format!("{}: {error}", path.display())))
    }
    fn validate(&self, interaction: bool) -> Result<(), StorybookAutomationError> {
        if self.scenario.is_some() && !self.steps.is_empty() {
            return Err(unavailable(
                "smoke plan selects either a fresh scenario or ad-hoc steps",
            ));
        }
        if !interaction && (self.scenario.is_some() || !self.steps.is_empty()) {
            return Err(unavailable("smoke effects require --allow-interaction"));
        }
        if self
            .route
            .as_ref()
            .is_some_and(|route| route.trim().is_empty() || route.len() > 256)
            || self
                .scenario
                .as_ref()
                .is_some_and(|scenario| scenario.trim().is_empty() || scenario.len() > 128)
        {
            return Err(unavailable("smoke route or scenario ID is invalid"));
        }
        validate_interaction_request(&self.request())
    }
    fn request(&self) -> StoryInteractionRequest {
        StoryInteractionRequest {
            story_key: self.route.clone(),
            controls: Default::default(),
            width: None,
            height: None,
            viewport: None,
            presentation: None,
            steps: if self.steps.is_empty() {
                vec![StoryInteractionStep::WaitFrames { count: 1 }]
            } else {
                self.steps.clone()
            },
            postconditions: self.postconditions.clone(),
            capture: None,
        }
    }
}

#[derive(Clone, Debug, bon::Builder)]
pub struct SmokeOptions {
    output: PathBuf,
    #[builder(default)]
    allow_interaction: bool,
    #[builder(default)]
    lifecycle: bool,
}

/// Evidence from completed checks, including typed failures and partial results.
#[derive(Clone, Debug, Serialize)]
pub struct SmokeReport {
    passed: bool,
    serial: String,
    checks: Vec<String>,
    initial_host: Option<HostDescriptor>,
    final_host: Option<HostDescriptor>,
    captures: Vec<HostCaptureSnapshot>,
    error: Option<StorybookAutomationError>,
}
impl SmokeReport {
    pub fn is_passed(&self) -> bool {
        self.passed
    }
    pub fn error(&self) -> Option<&StorybookAutomationError> {
        self.error.as_ref()
    }
}

/// Retained owner for smoke work and lifecycle restoration. Construct inside the
/// initial Tokio runtime; dropping a response future preserves admitted work.
/// Call [`Self::shutdown`] before dropping that runtime.
#[derive(Clone)]
pub struct SmokeRunner {
    transport: AdbTransport,
    device_port: u16,
    application: Option<AndroidApplication>,
    jobs: Arc<OwnedTasks>,
    active: Arc<std::sync::atomic::AtomicBool>,
}
impl SmokeRunner {
    /// Whether this runner owns an admitted smoke operation.
    pub fn is_running(&self) -> bool {
        self.active.load(Ordering::Acquire)
    }
    pub fn new(
        transport: AdbTransport,
        device_port: u16,
        application: Option<AndroidApplication>,
    ) -> Self {
        Self {
            transport,
            device_port,
            application,
            jobs: OwnedTasks::current(),
            active: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }
    pub async fn run(
        &self,
        plan: SmokePlan,
        options: SmokeOptions,
    ) -> Result<SmokeReport, StorybookAutomationError> {
        if self
            .active
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return Err(StorybookAutomationError::AutomationBusy);
        }
        let lease = SmokeLease(self.active.clone());
        let (reply, receiver) = oneshot::channel();
        let transport = self.transport.clone();
        let application = self.application.clone();
        let port = self.device_port;
        let id = request_id();
        self.jobs.spawn(async move {
            let report =
                run_smoke_owned(transport, port, application.as_ref(), plan, options).await;
            let _ = reply.send(report);
            drop(lease);
        })?;
        receiver
            .await
            .map_err(|error| StorybookAutomationError::OutcomeUnknown {
                request_id: id,
                message: format!("owned smoke task ended: {error}"),
            })
    }
    pub async fn shutdown(&self) {
        self.jobs.settle().await;
    }
}
struct SmokeLease(Arc<std::sync::atomic::AtomicBool>);
impl Drop for SmokeLease {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

/// Run against an already running application. Lifecycle checks require explicit
/// interaction opt-in and an exclusively owned disposable emulator. The harness
/// restores observed rotation policy and resumes the app on failure. It owns no
/// app process and never replays a submitted input, scenario, or host action.
async fn run_smoke_owned(
    transport: AdbTransport,
    device_port: u16,
    application: Option<&AndroidApplication>,
    plan: SmokePlan,
    options: SmokeOptions,
) -> SmokeReport {
    let mut report = SmokeReport {
        passed: false,
        serial: transport.serial().to_owned(),
        checks: Vec::new(),
        initial_host: None,
        final_host: None,
        captures: Vec::new(),
        error: None,
    };
    let mut backend = None;
    let mut rotation = None;
    let mut backgrounded = false;
    let operation = async {
        plan.validate(options.allow_interaction)?;
        if options.lifecycle && (!options.allow_interaction || !transport.serial().starts_with("emulator-") || application.is_none()) {
            return Err(unavailable("lifecycle checks require --allow-interaction, [android] launch metadata, and an owned emulator"));
        }
        std::fs::create_dir_all(&options.output).map_err(|error| unavailable(error.to_string()))?;
        let attached = RemoteBackend::attach_when_ready(transport.clone(), device_port, ReadinessOptions::default()).await?;
        report.initial_host = Some(attached.descriptor.clone());
        backend = Some(attached);
        let current = backend.as_ref().unwrap();
        let stories = current.stories().await?;
        if stories.is_empty() { return Err(unavailable("application advertises no public routes")); }
        if let Some(route) = &plan.route {
            current.get_story(route.clone()).await?;
            if options.allow_interaction { current.open_story(route.clone()).await?; }
            else if current.host().await?.active_route() != route { return Err(unavailable("readonly smoke route differs from the active application route")); }
        }
        report.checks.push("public route catalog and native/GPUI agreement".to_owned());
        if let Some(scenario) = &plan.scenario { current.run_scenario(plan.route.clone(), scenario.clone()).await?; }
        else if !plan.steps.is_empty() { current.run_steps(plan.request()).await?; }
        verify_values(current, &plan.postconditions).await?;
        report.checks.push("application semantic postconditions".to_owned());
        capture_pair(current, &options.output, "initial", &mut report).await?;
        if options.lifecycle {
            let app = application.unwrap();
            let pid = process_id(&transport, &app.package).await?.ok_or_else(|| unavailable("lifecycle app has no live PID"))?;
            let original = RotationPolicy::observe(&transport).await?;
            rotation = Some(original);
            // Blur once; issuing Android Back after this can finish the Activity.
            current.run_steps(SmokePlan::builder().steps(vec![StoryInteractionStep::Blur {}, StoryInteractionStep::WaitFrames { count: 2 }]).build().request()).await?;
            let before = current.host().await?;
            let (orientation, angle) = match before.orientation() { DisplayOrientation::Portrait => (DisplayOrientation::Landscape, "1"), DisplayOrientation::Landscape => (DisplayOrientation::Portrait, "0") };
            transport.shell(&["cmd", "window", "user-rotation", "lock", angle]).await?.require_success()?;
            let next = RemoteBackend::attach_when_ready(transport.clone(), device_port, ReadinessOptions::builder()
                .orientation(orientation).previous_session(before.session().to_owned()).build()).await?;
            if !matches!(current.host().await, Err(StorybookAutomationError::StaleHost { .. })) { return Err(unavailable("old surface attachment remained valid after display replacement")); }
            current.shutdown().await;
            backend = Some(next);
            let current = backend.as_ref().unwrap();
            verify_values(current, &plan.postconditions).await?;
            capture_pair(current, &options.output, "rotated", &mut report).await?;
            report.checks.push("display replacement, fresh session, stale attachment, retained public state".to_owned());
            transport.shell(&["input", "keyevent", "3"]).await?.require_success()?;
            backgrounded = true;
            wait_background(&transport, device_port).await?;
            transport.shell(&["am", "start", "-W", "-n", &app.component()]).await?.require_success()?;
            backgrounded = false;
            let resumed = RemoteBackend::attach_when_ready(transport.clone(), device_port, ReadinessOptions::default()).await?;
            current.shutdown().await;
            backend = Some(resumed);
            if process_id(&transport, &app.package).await?.as_deref() != Some(&pid) { return Err(unavailable("lifecycle replaced the application process")); }
            verify_values(backend.as_ref().unwrap(), &plan.postconditions).await?;
            report.checks.push("background admission, resume, retained PID and public state".to_owned());
        }
        report.final_host = Some(backend.as_ref().unwrap().host().await?);
        Ok::<_, StorybookAutomationError>(())
    }.await;
    if let Some(backend) = backend {
        backend.shutdown().await;
    }
    let cleanup = async {
        if backgrounded && let Some(app) = application {
            transport
                .shell(&["am", "start", "-W", "-n", &app.component()])
                .await?
                .require_success()?;
        }
        if let Some(rotation) = rotation {
            let expected = rotation.clone();
            rotation.restore(&transport).await?;
            let restored = RotationPolicy::observe(&transport).await?;
            if restored != expected {
                return Err(unavailable(
                    "emulator rotation policy restoration disagrees with its initial observation",
                ));
            }
            let backend = RemoteBackend::attach_when_ready(
                transport.clone(),
                device_port,
                ReadinessOptions::builder()
                    .maybe_orientation(
                        report
                            .initial_host
                            .as_ref()
                            .map(HostDescriptor::orientation),
                    )
                    .build(),
            )
            .await?;
            report.final_host = Some(backend.host().await?);
            backend.shutdown().await;
            report
                .checks
                .push("emulator rotation policy and rendered orientation restored".to_owned());
        }
        Ok::<_, StorybookAutomationError>(())
    }
    .await;
    report.error = settle(operation, cleanup, "smoke lifecycle restoration", 0).err();
    report.passed = report.error.is_none();
    report
}

async fn verify_values(
    backend: &RemoteBackend,
    postconditions: &[StoryInteractionPostcondition],
) -> Result<(), StorybookAutomationError> {
    for condition in postconditions {
        let max_frames = condition
            .max_frames
            .unwrap_or(MAX_INTERACTION_WAITED_FRAMES);
        let wait = async {
            let mut route = backend.descriptor.active_route().to_owned();
            for _ in 0..max_frames {
                let values = backend.read_semantic_values().await?;
                route = values.story.capture_route_id;
                if values
                    .values
                    .iter()
                    .find(|value| value.key == condition.value_key)
                    .is_some_and(|value| {
                        condition
                            .json_pointer
                            .as_deref()
                            .map_or(Some(&value.value), |pointer| value.value.pointer(pointer))
                            == Some(&condition.expected)
                    })
                {
                    return Ok(());
                }
            }
            Err(StorybookAutomationError::SemanticValueWaitTimedOut {
                route,
                key: condition.value_key.clone(),
                max_frames,
            })
        };
        tokio::time::timeout(Duration::from_secs(10), wait)
            .await
            .map_err(|_| StorybookAutomationError::SemanticValueWaitTimedOut {
                route: backend.descriptor.active_route().to_owned(),
                key: condition.value_key.clone(),
                max_frames,
            })??;
    }
    Ok(())
}
async fn capture_pair(
    backend: &RemoteBackend,
    output: &Path,
    prefix: &str,
    report: &mut SmokeReport,
) -> Result<(), StorybookAutomationError> {
    for (scope, label) in [
        (HostCaptureScope::Display, "display"),
        (HostCaptureScope::Gpui, "gpui"),
    ] {
        let observation = backend
            .capture_host(scope, output.join(format!("{prefix}-{label}.png")))
            .await?;
        let dimensions = image::image_dimensions(observation.path())
            .map_err(|error| unavailable(error.to_string()))?;
        if dimensions != (observation.pixel_width(), observation.pixel_height())
            || dimensions.0 == 0
            || dimensions.1 == 0
        {
            return Err(unavailable(
                "published PNG dimensions disagree with capture metadata",
            ));
        }
        report.captures.push(observation);
    }
    report
        .checks
        .push(format!("{prefix} display/GPUI PNGs and capture provenance"));
    Ok(())
}
async fn wait_background(
    transport: &AdbTransport,
    port: u16,
) -> Result<(), StorybookAutomationError> {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    loop {
        match RemoteBackend::attach_with_transport(transport.clone(), port).await {
            Err(StorybookAutomationError::HostNotReady {
                issue: HostReadinessIssue::Background,
            }) => return Ok(()),
            Ok(backend) => backend.shutdown().await,
            Err(
                StorybookAutomationError::NoLiveHost
                | StorybookAutomationError::StaleHost { .. }
                | StorybookAutomationError::HostNotReady {
                    issue: HostReadinessIssue::SurfaceUnavailable,
                },
            ) => {},
            Err(error) => return Err(error),
        }
        if tokio::time::Instant::now() >= deadline {
            return Err(unavailable("background readiness did not settle"));
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}
#[derive(Clone, Eq, PartialEq)]
struct RotationPolicy {
    automatic: String,
    angle: String,
}
impl RotationPolicy {
    async fn observe(transport: &AdbTransport) -> Result<Self, StorybookAutomationError> {
        let read = async |key| {
            let output = transport
                .shell(&["settings", "get", "system", key])
                .await?
                .require_success()?;
            Ok::<_, StorybookAutomationError>(
                String::from_utf8_lossy(output.stdout()).trim().to_owned(),
            )
        };
        let automatic = read("accelerometer_rotation").await?;
        let angle = read("user_rotation").await?;
        if !matches!(automatic.as_str(), "0" | "1")
            || !matches!(angle.as_str(), "0" | "1" | "2" | "3")
        {
            return Err(unavailable(
                "cannot preserve the observed emulator rotation policy",
            ));
        }
        Ok(Self { automatic, angle })
    }
    async fn restore(self, transport: &AdbTransport) -> Result<(), StorybookAutomationError> {
        transport
            .shell(&["cmd", "window", "user-rotation", "lock", &self.angle])
            .await?
            .require_success()?;
        if self.automatic == "1" {
            transport
                .shell(&["cmd", "window", "user-rotation", "free"])
                .await?
                .require_success()?;
        }
        Ok(())
    }
}
