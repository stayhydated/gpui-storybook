//! Computer-owned ADB forwarding, remote backend, and compositor capture.
//!
//! The backend negotiates once. Every request validates response identity and
//! uses a new connection without replay. Capture paths originate on this host;
//! device responses never select filesystem destinations. The device retains
//! an exclusive capture ticket through ADB observation and final validation.

use gpui_storybook_automation::{wire::*, *};
use std::{
    net::TcpStream,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

pub struct AdbForward {
    serial: String,
    port: u16,
}
impl AdbForward {
    pub fn attach(serial: String, device_port: u16) -> Result<Self, StorybookAutomationError> {
        let result = adb(
            &serial,
            &["forward", "tcp:0", &format!("tcp:{device_port}")],
        )?;
        let port = String::from_utf8_lossy(&result.stdout)
            .trim()
            .parse()
            .map_err(|_| unavailable("ADB returned an invalid forwarded port"))?;
        Ok(Self { serial, port })
    }
    pub fn port(&self) -> u16 {
        self.port
    }
}
impl Drop for AdbForward {
    fn drop(&mut self) {
        let _ = adb(
            &self.serial,
            &["forward", "--remove", &format!("tcp:{}", self.port)],
        );
    }
}

pub struct RemoteBackend {
    forwarding: AdbForward,
    descriptor: HostDescriptor,
}
impl RemoteBackend {
    pub fn attach(serial: String, device_port: u16) -> Result<Self, StorybookAutomationError> {
        let forwarding = AdbForward::attach(serial, device_port)?;
        let response = exchange(
            forwarding.port,
            "",
            request_id(),
            DeviceOperation::GetHost {},
        )?;
        let DeviceResult::Host(descriptor) = response else {
            return Err(unavailable("handshake returned an unexpected result"));
        };
        if descriptor.protocol_version() != PROTOCOL_VERSION {
            return Err(StorybookAutomationError::ProtocolMismatch {
                expected: PROTOCOL_VERSION,
                actual: descriptor.protocol_version(),
            });
        }
        // This provider observes fixed device geometry through ADB. Negotiate
        // only operations whose wire delivery and host artifact policy it owns.
        for capability in [
            AutomationCapability::StorySizing,
            AutomationCapability::CaptureControls,
            AutomationCapability::InteractionCapture,
            AutomationCapability::DesktopLaunch,
        ] {
            if descriptor.capabilities().contains(capability) {
                return Err(StorybookAutomationError::UnsupportedCapability { capability });
            }
        }
        if descriptor
            .capabilities()
            .contains(AutomationCapability::StoryCapture)
        {
            descriptor
                .capabilities()
                .require(AutomationCapability::DisplayCapture)?;
        }
        if !descriptor.agrees() {
            return Err(unavailable("native route and GPUI geometry are not ready"));
        }
        Ok(Self {
            forwarding,
            descriptor,
        })
    }
    async fn request(
        &self,
        command: DeviceOperation,
    ) -> Result<DeviceResult, StorybookAutomationError> {
        let port = self.forwarding.port;
        let session = self.descriptor.session().to_owned();
        let id = request_id();
        tokio::task::spawn_blocking(move || exchange(port, &session, id, command))
            .await
            .map_err(|error| StorybookAutomationError::OutcomeUnknown {
                request_id: id,
                message: error.to_string(),
            })?
    }
    async fn capture(
        &self,
        scope: HostCaptureScope,
        output_path: PathBuf,
        expected_route: Option<String>,
    ) -> Result<HostCaptureSnapshot, StorybookAutomationError> {
        self.capabilities()
            .require(AutomationCapability::DisplayCapture)?;
        let DeviceResult::CaptureTicket { ticket, host } =
            self.request(DeviceOperation::PrepareCapture {}).await?
        else {
            return Err(unavailable(
                "capture preparation returned an unexpected result",
            ));
        };
        if expected_route
            .as_deref()
            .is_some_and(|route| route != host.active_route())
        {
            let _ = self
                .request(DeviceOperation::FinishCapture { ticket })
                .await;
            return Err(StorybookAutomationError::StaleHost {
                message: "route changed before story capture".to_owned(),
            });
        }
        let serial = self.forwarding.serial.clone();
        let geometry = host.geometry().clone();
        let result = tokio::task::spawn_blocking(move || {
            let bytes = adb(&serial, &["exec-out", "screencap", "-p"])?;
            let display =
                image::load_from_memory_with_format(&bytes.stdout, image::ImageFormat::Png)
                    .map_err(|error| unavailable(error.to_string()))?;
            if !geometry.validate()
                || (display.width(), display.height())
                    != (geometry.display_width(), geometry.display_height())
            {
                return Err(unavailable(
                    "ADB image dimensions disagree with observed device geometry",
                ));
            }
            let image = match scope {
                HostCaptureScope::Display => display,
                HostCaptureScope::Gpui => display.crop_imm(
                    geometry.x(),
                    geometry.y(),
                    geometry.width(),
                    geometry.height(),
                ),
            };
            Ok(image)
        })
        .await
        .map_err(|error| unavailable(error.to_string()))?;
        // Finish even after a provider error. The device validates the interval
        // before releasing its ticket; a failed validation never writes a PNG.
        let finished = self
            .request(DeviceOperation::FinishCapture { ticket })
            .await;
        let image = result?;
        let DeviceResult::Host(after) = finished? else {
            return Err(unavailable(
                "capture completion returned an unexpected result",
            ));
        };
        if host != after {
            return Err(StorybookAutomationError::StaleHost {
                message: "host changed during ADB compositor observation".to_owned(),
            });
        }
        let destination = output_path.clone();
        let width = image.width();
        let height = image.height();
        tokio::task::spawn_blocking(move || {
            if let Some(parent) = destination
                .parent()
                .filter(|path| !path.as_os_str().is_empty())
            {
                std::fs::create_dir_all(parent).map_err(|error| unavailable(error.to_string()))?;
            }
            image
                .save_with_format(&destination, image::ImageFormat::Png)
                .map_err(|error| unavailable(error.to_string()))
        })
        .await
        .map_err(|error| unavailable(error.to_string()))??;
        Ok(HostCaptureSnapshot::builder()
            .request_id(ticket)
            .scope(scope)
            .path(output_path)
            .pixel_width(width)
            .pixel_height(height)
            .host(host)
            .provider("adb_compositor_observation".to_owned())
            .build())
    }
}

macro_rules! request_method {
    ($name:ident, $output:ty, $command:expr, $variant:ident) => {
        fn $name(&self) -> BackendFuture<'_, $output> {
            Box::pin(async move {
                match self.request($command).await? {
                    DeviceResult::$variant(value) => Ok(value),
                    _ => Err(unavailable("unexpected device result")),
                }
            })
        }
    };
}
impl AutomationBackend for RemoteBackend {
    fn capabilities(&self) -> AutomationCapabilities {
        self.descriptor.capabilities().clone()
    }
    fn wait_until_ready(&self) -> BackendFuture<'_, ()> {
        Box::pin(async move { self.host().await.map(|_| ()) })
    }
    request_method!(host, HostDescriptor, DeviceOperation::GetHost {}, Host);
    request_method!(
        stories,
        Vec<StorySnapshot>,
        DeviceOperation::ListStories {},
        Stories
    );
    request_method!(
        current_story,
        StoryCurrentSnapshot,
        DeviceOperation::CurrentStory {},
        Current
    );
    request_method!(
        read_controls,
        StoryControlsSnapshot,
        DeviceOperation::ReadControls {},
        Controls
    );
    request_method!(
        list_actions,
        Vec<StoryActionSnapshot>,
        DeviceOperation::ListActions {},
        Actions
    );
    request_method!(
        list_interaction_targets,
        StoryInteractionTargetsSnapshot,
        DeviceOperation::ListTargets {},
        Targets
    );
    request_method!(
        read_semantic_values,
        StorySemanticValuesSnapshot,
        DeviceOperation::ReadValues {},
        Values
    );
    request_method!(
        list_host_actions,
        Vec<HostActionDescriptor>,
        DeviceOperation::ListHostActions {},
        HostActions
    );
    fn get_story(&self, key: String) -> BackendFuture<'_, StorySnapshot> {
        Box::pin(async move {
            match self.request(DeviceOperation::GetStory { key }).await? {
                DeviceResult::Story(story) => Ok(story),
                _ => Err(unavailable("unexpected story result")),
            }
        })
    }
    fn open_story(&self, key: String) -> BackendFuture<'_, StoryCurrentSnapshot> {
        Box::pin(async move {
            match self.request(DeviceOperation::OpenStory { key }).await? {
                DeviceResult::Current(current) => Ok(current),
                _ => Err(unavailable("unexpected navigation result")),
            }
        })
    }
    fn list_scenarios(
        &self,
        story_key: Option<String>,
    ) -> BackendFuture<'_, StoryScenariosSnapshot> {
        Box::pin(async move {
            let story = match story_key {
                Some(key) => self.get_story(key).await?,
                None => self
                    .current_story()
                    .await?
                    .story
                    .ok_or(StorybookAutomationError::NoActiveStory)?,
            };
            Ok(StoryScenariosSnapshot {
                scenarios: story.scenarios.clone(),
                story,
            })
        })
    }
    fn set_control(
        &self,
        key: String,
        value: ControlValue,
    ) -> BackendFuture<'_, StoryControlsSnapshot> {
        Box::pin(async move {
            match self
                .request(DeviceOperation::SetControl { key, value })
                .await?
            {
                DeviceResult::Controls(value) => Ok(value),
                _ => Err(unavailable("unexpected controls result")),
            }
        })
    }
    fn reset_control(&self, key: Option<String>) -> BackendFuture<'_, StoryControlsSnapshot> {
        Box::pin(async move {
            match self.request(DeviceOperation::ResetControl { key }).await? {
                DeviceResult::Controls(value) => Ok(value),
                _ => Err(unavailable("unexpected controls result")),
            }
        })
    }
    fn run_steps(
        &self,
        request: StoryInteractionRequest,
    ) -> BackendFuture<'_, StoryInteractionSnapshot> {
        Box::pin(async move {
            self.capabilities().validate_interaction(&request)?;
            match self.request(DeviceOperation::RunSteps { request }).await? {
                DeviceResult::Interaction(value) => Ok(value),
                _ => Err(unavailable("unexpected interaction result")),
            }
        })
    }
    fn run_scenario(
        &self,
        story_key: Option<String>,
        scenario_key: String,
    ) -> BackendFuture<'_, StoryScenarioRunSnapshot> {
        Box::pin(async move {
            self.capabilities()
                .require(AutomationCapability::FreshScenarios)?;
            match self
                .request(DeviceOperation::RunScenario {
                    story_key,
                    scenario_key,
                })
                .await?
            {
                DeviceResult::Scenario(value) => Ok(value),
                _ => Err(unavailable("unexpected scenario result")),
            }
        })
    }
    fn dispatch_host_action(&self, action: HostAction) -> BackendFuture<'_, HostDescriptor> {
        Box::pin(async move {
            match self
                .request(DeviceOperation::DispatchHostAction { action })
                .await?
            {
                DeviceResult::Host(value) => Ok(value),
                _ => Err(unavailable("unexpected host action result")),
            }
        })
    }
    fn capture_host(
        &self,
        scope: HostCaptureScope,
        output_path: PathBuf,
    ) -> BackendFuture<'_, HostCaptureSnapshot> {
        Box::pin(self.capture(scope, output_path, None))
    }
    fn capture_current_story(
        &self,
        request: StoryScreenshotRequest,
    ) -> BackendFuture<'_, StoryCaptureSnapshot> {
        Box::pin(async move {
            self.capabilities().validate_capture(&request)?;
            let story = self
                .current_story()
                .await?
                .story
                .ok_or(StorybookAutomationError::NoActiveStory)?;
            let path = request.output_path.unwrap_or_else(|| {
                Path::new("target/mobile-captures").join(format!("{}.png", story.key))
            });
            let captured = self
                .capture(HostCaptureScope::Gpui, path, Some(story.key.clone()))
                .await?;
            Ok(StoryCaptureSnapshot {
                request_id: captured.request_id(),
                path: captured.path().to_owned(),
                pixel_width: captured.pixel_width(),
                pixel_height: captured.pixel_height(),
                story,
                observation: Some(captured),
            })
        })
    }
}

fn exchange(
    port: u16,
    session: &str,
    id: u64,
    command: DeviceOperation,
) -> Result<DeviceResult, StorybookAutomationError> {
    let mut stream = TcpStream::connect_timeout(
        &std::net::SocketAddr::from(([127, 0, 0, 1], port)),
        Duration::from_secs(5),
    )
    .map_err(|error| unavailable(error.to_string()))?;
    stream
        .set_read_timeout(Some(Duration::from_secs(30)))
        .map_err(|error| unavailable(error.to_string()))?;
    stream
        .set_write_timeout(Some(Duration::from_secs(5)))
        .map_err(|error| unavailable(error.to_string()))?;
    let request = DeviceRequest::builder()
        .protocol_version(PROTOCOL_VERSION)
        .session(session.to_owned())
        .request_id(id)
        .command(command)
        .build();
    // Any write failure can be partial, so the outcome becomes unknown even
    // when the peer has not yet returned an acknowledgment.
    write_frame(&mut stream, &request).map_err(|error| {
        StorybookAutomationError::OutcomeUnknown {
            request_id: id,
            message: error.to_string(),
        }
    })?;
    let response: DeviceResponse =
        read_frame(&mut stream).map_err(|error| StorybookAutomationError::OutcomeUnknown {
            request_id: id,
            message: error.to_string(),
        })?;
    if response.protocol_version() != PROTOCOL_VERSION {
        return Err(StorybookAutomationError::ProtocolMismatch {
            expected: PROTOCOL_VERSION,
            actual: response.protocol_version(),
        });
    }
    if response.request_id() != id || (!session.is_empty() && response.session() != session) {
        return Err(StorybookAutomationError::StaleHost {
            message: "device response identity does not match submitted request".to_owned(),
        });
    }
    response.into_outcome()
}
// IDs are unique across attachments in this process and seeded per host process.
// Device admission retains a bounded duplicate window; clients never replay IDs.
fn request_id() -> u64 {
    static IDS: std::sync::OnceLock<AtomicU64> = std::sync::OnceLock::new();
    IDS.get_or_init(|| {
        AtomicU64::new(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("host clock")
                .as_nanos() as u64
                & ((1 << 53) - 1),
        )
    })
    .fetch_add(1, Ordering::Relaxed)
}

pub fn adb(serial: &str, arguments: &[&str]) -> Result<Output, StorybookAutomationError> {
    use std::io::Read as _;
    let mut child = Command::new("adb")
        .arg("-s")
        .arg(serial)
        .args(arguments)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|error| unavailable(error.to_string()))?;
    fn drain(reader: impl std::io::Read) -> std::io::Result<Vec<u8>> {
        let mut bytes = Vec::new();
        reader.take(64 * 1024 * 1024 + 1).read_to_end(&mut bytes)?;
        if bytes.len() > 64 * 1024 * 1024 {
            return Err(std::io::Error::other("ADB output exceeds 64 MiB"));
        }
        Ok(bytes)
    }
    let stdout = child.stdout.take().expect("piped stdout");
    let stderr = child.stderr.take().expect("piped stderr");
    let stdout = std::thread::spawn(move || drain(stdout));
    let stderr = std::thread::spawn(move || drain(stderr));
    let timeout = if arguments.first() == Some(&"install") {
        120
    } else {
        10
    };
    let deadline = std::time::Instant::now() + Duration::from_secs(timeout);
    let mut timed_out = false;
    let status = loop {
        if let Some(status) = child
            .try_wait()
            .map_err(|error| unavailable(error.to_string()))?
        {
            break status;
        }
        if std::time::Instant::now() >= deadline {
            timed_out = true;
            let _ = child.kill();
            break child
                .wait()
                .map_err(|error| unavailable(error.to_string()))?;
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    let stdout = stdout
        .join()
        .map_err(|_| unavailable("ADB stdout reader failed"))?
        .map_err(|error| unavailable(error.to_string()))?;
    let stderr = stderr
        .join()
        .map_err(|_| unavailable("ADB stderr reader failed"))?
        .map_err(|error| unavailable(error.to_string()))?;
    if timed_out {
        return Err(unavailable("ADB process exceeded its deadline"));
    }
    let output = Output {
        status,
        stdout,
        stderr,
    };
    if output.status.success() {
        Ok(output)
    } else {
        Err(unavailable(String::from_utf8_lossy(&output.stderr).trim()))
    }
}
fn unavailable(message: impl Into<String>) -> StorybookAutomationError {
    StorybookAutomationError::CaptureUnavailable {
        message: message.into(),
    }
}

pub fn shared(backend: RemoteBackend) -> SharedAutomationBackend {
    Arc::new(backend)
}
