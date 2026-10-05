//! Computer-owned direct ADB transport, remote backend, and compositor capture.
//!
//! The backend negotiates once. Every request validates response identity and
//! uses a new connection without replay. Capture paths originate on this host;
//! device responses never select filesystem destinations. The device retains
//! an exclusive capture ticket through ADB observation and final validation.

mod adb;
mod tasks;
pub use adb::{AdbTransport, AdbTransportOptions, ShellOutput};

use gpui_storybook_automation::{wire::*, *};
use std::{
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};
use tasks::OwnedTasks;
use tokio::sync::oneshot;

/// An attachment to one negotiated device session. Mutations use a fresh direct
/// ADB connection and are never replayed. Capture tasks run on the runtime used
/// by attachment, retaining their device ticket across caller cancellation.
/// Keep that runtime alive and call [`Self::shutdown`] before stopping its app.
#[derive(Clone)]
pub struct RemoteBackend {
    transport: AdbTransport,
    device_port: u16,
    descriptor: HostDescriptor,
    jobs: Arc<OwnedTasks>,
}
impl RemoteBackend {
    pub async fn attach(
        serial: String,
        device_port: u16,
    ) -> Result<Self, StorybookAutomationError> {
        let transport = AdbTransport::new(AdbTransportOptions::builder().serial(serial).build())?;
        Self::attach_with_transport(transport, device_port).await
    }

    /// Negotiate an explicitly configured transport on the current Tokio runtime.
    pub async fn attach_with_transport(
        transport: AdbTransport,
        device_port: u16,
    ) -> Result<Self, StorybookAutomationError> {
        if device_port == 0 {
            return Err(unavailable("device port must be nonzero"));
        }
        let response = transport
            .exchange(device_port, "", request_id(), DeviceOperation::GetHost {})
            .await?;
        let DeviceResult::Host(descriptor) = response else {
            return Err(unavailable("handshake returned an unexpected result"));
        };
        if descriptor.protocol_version() != PROTOCOL_VERSION {
            return Err(StorybookAutomationError::ProtocolMismatch {
                expected: PROTOCOL_VERSION,
                actual: descriptor.protocol_version(),
            });
        }
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
            transport,
            device_port,
            descriptor,
            jobs: OwnedTasks::current(),
        })
    }

    /// Close capture admission and await all admitted captures, including those
    /// whose caller disconnected. Call after the MCP server stops admitting work.
    pub async fn shutdown(&self) {
        self.jobs.settle().await;
    }

    async fn request(
        &self,
        command: DeviceOperation,
    ) -> Result<DeviceResult, StorybookAutomationError> {
        self.request_with_id(request_id(), command).await
    }

    async fn request_with_id(
        &self,
        id: u64,
        command: DeviceOperation,
    ) -> Result<DeviceResult, StorybookAutomationError> {
        let result = self
            .transport
            .exchange(self.device_port, self.descriptor.session(), id, command)
            .await?;
        let host = match &result {
            DeviceResult::Host(host) | DeviceResult::CaptureTicket { host, .. } => Some(host),
            _ => None,
        };
        if host.is_some_and(|host| host.capabilities() != self.descriptor.capabilities()) {
            return Err(StorybookAutomationError::StaleHost {
                message: "device capabilities changed; rediscover the attachment".to_owned(),
            });
        }
        Ok(result)
    }

    async fn capture(
        &self,
        scope: HostCaptureScope,
        output_path: PathBuf,
        expected_route: Option<String>,
    ) -> Result<HostCaptureSnapshot, StorybookAutomationError> {
        self.capabilities()
            .require(AutomationCapability::DisplayCapture)?;
        let backend = self.clone();
        let (reply, receiver) = oneshot::channel();
        self.jobs.spawn(async move {
            let result = backend
                .capture_owned(scope, output_path, expected_route)
                .await;
            let _ = reply.send(result);
        })?;
        receiver
            .await
            .map_err(|error| unavailable(format!("owned capture ended: {error}")))?
    }

    #[tracing::instrument(skip_all, fields(session = self.descriptor.session(), ?scope, request_id = tracing::field::Empty))]
    async fn capture_owned(
        &self,
        scope: HostCaptureScope,
        output_path: PathBuf,
        expected_route: Option<String>,
    ) -> Result<HostCaptureSnapshot, StorybookAutomationError> {
        let id = request_id();
        tracing::Span::current().record("request_id", id);
        let prepared = self
            .request_with_id(id, DeviceOperation::PrepareCapture {})
            .await;
        let (ticket, host) = match prepared {
            Ok(DeviceResult::CaptureTicket { ticket, host }) if ticket == id => (ticket, host),
            result => {
                // Preparation can have been admitted even if its reply was lost.
                // Finish also revokes a pending capture frame by its request ID.
                let finished = self.finish_capture(id).await;
                let error = result.err().unwrap_or_else(|| {
                    unavailable("capture preparation returned an invalid ticket")
                });
                return settle(Err(error), finished, "finish capture preparation", id)
                    .map(|(capture, _)| capture);
            },
        };
        let result = async {
            if expected_route
                .as_deref()
                .is_some_and(|route| route != host.active_route())
            {
                return Err(StorybookAutomationError::StaleHost {
                    message: "route changed before story capture".to_owned(),
                });
            }
            let bytes = self.transport.screencap().await?;
            let geometry = host.geometry().clone();
            tokio::task::spawn_blocking(move || decode_capture(bytes, geometry, scope))
                .await
                .map_err(|error| unavailable(error.to_string()))?
        }
        .await;
        // Provider failures and decoder panics both settle the admitted ticket.
        let finished = self.finish_capture(ticket).await;
        let (image, after) = settle(result, finished, "finish capture", ticket)?;
        if host != after {
            return Err(StorybookAutomationError::StaleHost {
                message: "host changed during ADB compositor observation".to_owned(),
            });
        }
        let destination = output_path.clone();
        let width = image.width();
        let height = image.height();
        tokio::task::spawn_blocking(move || {
            atomic_write(&destination, |file| {
                image
                    .write_to(file, image::ImageFormat::Png)
                    .map_err(|error| unavailable(error.to_string()))
            })
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

    async fn finish_capture(
        &self,
        ticket: u64,
    ) -> Result<HostDescriptor, StorybookAutomationError> {
        match self
            .request(DeviceOperation::FinishCapture { ticket })
            .await?
        {
            DeviceResult::Host(host) => Ok(host),
            _ => Err(unavailable(
                "capture completion returned an unexpected result",
            )),
        }
    }
}

/// Preserve both typed outcomes, including the primary mutation uncertainty.
fn settle<T, U>(
    operation: Result<T, StorybookAutomationError>,
    cleanup: Result<U, StorybookAutomationError>,
    phase: &str,
    request_id: u64,
) -> Result<(T, U), StorybookAutomationError> {
    match (operation, cleanup) {
        (Ok(value), Ok(settled)) => Ok((value, settled)),
        (Err(error), Ok(_)) | (Ok(_), Err(error)) => Err(error),
        (Err(operation), Err(cleanup)) => {
            tracing::warn!(request_id, phase, %operation, %cleanup, "operation and cleanup failed");
            Err(StorybookAutomationError::SettlementFailed {
                operation: Box::new(operation),
                cleanup: Box::new(cleanup),
            })
        },
    }
}

/// Encode beside the destination; a failed encode or replace preserves the
/// existing artifact. Atomic replacement is an observation guarantee, not a
/// power-loss durability guarantee.
fn atomic_write(
    destination: &Path,
    encode: impl FnOnce(&mut std::fs::File) -> Result<(), StorybookAutomationError>,
) -> Result<(), StorybookAutomationError> {
    use std::io::Write as _;
    let parent = destination
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(parent).map_err(|error| unavailable(error.to_string()))?;
    let mut file =
        tempfile::NamedTempFile::new_in(parent).map_err(|error| unavailable(error.to_string()))?;
    encode(file.as_file_mut())?;
    file.as_file_mut()
        .flush()
        .map_err(|error| unavailable(error.to_string()))?;
    file.persist(destination)
        .map_err(|error| unavailable(error.to_string()))?;
    Ok(())
}

fn decode_capture(
    bytes: Vec<u8>,
    geometry: SurfaceGeometry,
    scope: HostCaptureScope,
) -> Result<image::DynamicImage, StorybookAutomationError> {
    if !geometry.validate() {
        return Err(unavailable("invalid observed device geometry"));
    }
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(geometry.display_width());
    limits.max_image_height = Some(geometry.display_height());
    limits.max_alloc = Some(adb::MAX_CAPTURE_BYTES as u64);
    let mut reader =
        image::ImageReader::with_format(std::io::Cursor::new(&bytes), image::ImageFormat::Png);
    reader.limits(limits.clone());
    let dimensions = reader
        .into_dimensions()
        .map_err(|error| unavailable(error.to_string()))?;
    if dimensions != (geometry.display_width(), geometry.display_height()) {
        return Err(unavailable(
            "ADB image dimensions disagree with observed device geometry",
        ));
    }
    let mut reader =
        image::ImageReader::with_format(std::io::Cursor::new(bytes), image::ImageFormat::Png);
    reader.limits(limits);
    let display = reader
        .decode()
        .map_err(|error| unavailable(error.to_string()))?;
    Ok(match scope {
        HostCaptureScope::Display => display,
        HostCaptureScope::Gpui => display.crop_imm(
            geometry.x(),
            geometry.y(),
            geometry.width(),
            geometry.height(),
        ),
    })
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
                Path::new("target/mobile-captures").join(format!(
                    "story-{}-{}.png",
                    std::process::id(),
                    request_id()
                ))
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

// IDs are unique across attachments in this process and seeded per host process.
// Device admission retains a bounded duplicate window; clients never replay IDs.
fn request_id() -> u64 {
    static IDS: std::sync::OnceLock<AtomicU64> = std::sync::OnceLock::new();
    IDS.get_or_init(|| {
        AtomicU64::new(
            (std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("host clock")
                .as_nanos() as u64
                & ((1 << 53) - 1))
                .max(1),
        )
    })
    .fetch_add(1, Ordering::Relaxed)
}

fn unavailable(message: impl Into<String>) -> StorybookAutomationError {
    StorybookAutomationError::CaptureUnavailable {
        message: message.into(),
    }
}

pub fn shared(backend: RemoteBackend) -> SharedAutomationBackend {
    Arc::new(backend)
}

#[cfg(test)]
mod tests;
