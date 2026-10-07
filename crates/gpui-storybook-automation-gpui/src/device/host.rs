//! Own the common attachment, shell synchronization, and polling lifecycle.

use super::*;
use gpui::{Entity, Render, WindowHandle};

/// Device endpoint and native appearance choices for one embedded application.
#[derive(Clone, bon::Builder)]
pub struct DeviceHostOptions {
    /// Explicit application build/runtime opt-in. The default opens no endpoint.
    #[builder(default)]
    automation: bool,
    #[builder(default = gpui_storybook_mobile::DEVICE_PORT)]
    port: u16,
    #[builder(default = HostAppearance::standard())]
    appearances: Vec<HostAppearance>,
}
impl Default for DeviceHostOptions {
    fn default() -> Self {
        Self::builder().build()
    }
}

/// An automation owner for the application's existing root and window. It owns
/// transport shutdown and attachment generations; it creates no application.
pub struct DeviceHost<Root: EmbeddedRoot> {
    root: Entity<Root>,
    attachment: GpuiHostAttachment<Root>,
    coordinator: Option<DeviceCoordinator<Root>>,
    shell: NativeShellHandle,
    surface: u64,
    appearance: Option<String>,
    catalog: Vec<StorySnapshot>,
    capabilities: AutomationCapabilities,
}
impl<Root: EmbeddedRoot> DeviceHost<Root> {
    /// Start only after application build/runtime opt-in. The queue callback
    /// receives one serializable payload and must distinguish known rejection
    /// from an unknown outcome after submission.
    pub fn attach(
        root: &Entity<Root>,
        catalog: Vec<StorySnapshot>,
        capabilities: AutomationCapabilities,
        options: DeviceHostOptions,
        dispatch: impl Fn(NativeDispatch) -> Result<(), NativeSubmissionError> + 'static,
        window: &Window,
        cx: &mut App,
    ) -> Result<Self, StorybookAutomationError> {
        let attachment = GpuiHostAttachment::attach(
            root,
            catalog.clone(),
            capabilities.clone(),
            Rc::new(crate::interaction::NoCaptureProvider),
            window,
            cx,
        )
        .map_err(|error| StorybookAutomationError::ControlOperationFailed {
            message: error.to_string(),
        })?;
        let endpoint = options
            .automation
            .then(|| DeviceEndpoint::listen_fresh(options.port))
            .transpose()
            .map_err(|error| StorybookAutomationError::CaptureUnavailable {
                message: format!("device endpoint: {error}"),
            })?;
        let shell = NativeShellHandle::new(
            endpoint
                .as_ref()
                .map(DeviceEndpoint::gate)
                .unwrap_or_default(),
            options.appearances,
        )?;
        let native = shell.clone();
        Ok(Self {
            root: root.clone(),
            attachment,
            coordinator: endpoint.map(|endpoint| {
                DeviceCoordinator::new(endpoint, move |selection| {
                    native.submit(selection, &dispatch)
                })
            }),
            shell,
            surface: u64::MAX,
            appearance: None,
            catalog,
            capabilities,
        })
    }

    /// Clone into native lifecycle/JNI callbacks.
    pub fn native_shell(&self) -> NativeShellHandle {
        self.shell.clone()
    }

    /// Apply the committed native observation on the GPUI owner and process
    /// device work. Application input and fixture state remain on the real root.
    pub fn poll(
        &mut self,
        window: &mut Window,
        cx: &mut App,
    ) -> Result<(), StorybookAutomationError> {
        let mut shell = self.shell.snapshot();
        if self.surface != shell.surface() {
            self.attachment.invalidate(window, cx);
            self.attachment = GpuiHostAttachment::attach(
                &self.root,
                self.catalog.clone(),
                self.capabilities.clone(),
                Rc::new(crate::interaction::NoCaptureProvider),
                window,
                cx,
            )
            .map_err(|error| StorybookAutomationError::ControlOperationFailed {
                message: error.to_string(),
            })?;
            if let Some(coordinator) = &mut self.coordinator {
                coordinator.surface_replaced();
            }
            self.surface = shell.surface();
            self.appearance = None;
            window.refresh();
        }
        let synchronization: Result<(), StorybookAutomationError> = (|| {
            if shell.active() && !shell.route().is_empty() {
                if self.root.read(cx).active_route(cx) != shell.route() {
                    self.attachment.open_story(shell.route(), window, cx)?;
                }
                if self
                    .capabilities
                    .contains(AutomationCapability::Presentation)
                    && self.appearance.as_deref() != Some(shell.appearance().id())
                {
                    self.root.update(cx, |root, cx| {
                        root.apply_host_appearance(shell.appearance(), window, cx)
                    })?;
                    self.appearance = Some(shell.appearance().id().to_owned());
                    window.refresh();
                }
            }
            Ok(())
        })();
        // Continue servicing observations even if application synchronization
        // failed, so callers receive readiness diagnostics instead of starvation.
        if let Err(error) = &synchronization {
            shell.sync_issue = Some(HostReadinessIssue::ApplicationSynchronization {
                message: error.to_string(),
            });
        }
        if let Some(coordinator) = &mut self.coordinator {
            coordinator.poll(&self.attachment, &shell, window, cx);
        }
        synchronization
    }

    /// Run the owner until the existing window closes. Logs go to the host's
    /// tracing subscriber; repeated identical synchronization failures are bounded.
    pub fn run<View: Render>(mut self, window: WindowHandle<View>, cx: &mut App) {
        cx.spawn(async move |cx| {
            let mut previous = None;
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(16))
                    .await;
                let result = window.update(cx, |_, window, cx| self.poll(window, cx));
                match result {
                    Err(_) => break,
                    Ok(Ok(())) => previous = None,
                    Ok(Err(error)) => {
                        if previous.as_ref() != Some(&error) {
                            tracing::warn!(%error, "embedded automation synchronization");
                        }
                        previous = Some(error);
                    },
                }
            }
        })
        .detach();
    }
}
