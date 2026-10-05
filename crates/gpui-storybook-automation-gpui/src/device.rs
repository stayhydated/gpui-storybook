//! Device-to-GPUI coordinator for an application-owned root and native shell.
//!
//! Poll on the GPUI owner thread after applying the observed native route. Surface
//! invalidation revokes queued permits. A native acknowledgment timeout reports
//! an unknown outcome once and retains its lease until acknowledgment or explicit
//! surface invalidation. Native callbacks check their permit and surface revision
//! immediately before applying changes on the native lifecycle owner thread.

use crate::{
    AttachedInteraction, EmbeddedRoot, GpuiHostAttachment, regions::capture_region_bounds,
};
use gpui::{App, Window};
use gpui_storybook_automation::{wire::*, *};
use gpui_storybook_mobile::{
    AdmittedRequest, DeviceEndpoint, MutationLease, MutationPermit, OperationGate,
};
use std::{
    cell::Cell,
    collections::BTreeMap,
    marker::PhantomData,
    rc::Rc,
    sync::{Arc, atomic::AtomicUsize, mpsc::SyncSender},
    time::{Duration, Instant},
};
use tokio::sync::oneshot;

type Reply = SyncSender<Result<DeviceResult, StorybookAutomationError>>;
/// Immutable native observation captured alongside an owner-thread update.
#[derive(Clone, bon::Builder)]
pub struct NativeShellSnapshot {
    route: String,
    dark: bool,
    revision: u64,
    surface: u64,
    #[builder(default)]
    ack: u64,
    #[builder(default = true)]
    applied: bool,
    active: bool,
    geometry: SurfaceGeometry,
}
impl NativeShellSnapshot {
    pub fn route(&self) -> &str {
        &self.route
    }
    pub fn dark(&self) -> bool {
        self.dark
    }
    pub fn active(&self) -> bool {
        self.active
    }
    pub fn revision(&self) -> u64 {
        self.revision
    }
    pub fn surface(&self) -> u64 {
        self.surface
    }
    pub fn ack(&self) -> u64 {
        self.ack
    }
    pub fn geometry(&self) -> SurfaceGeometry {
        self.geometry.clone()
    }
}

/// One native selection, already preflighted and owned by a device lease.
/// The native callback retains this permit and checks it with the observed
/// surface immediately before dispatch; it acknowledges applied or revoked work.
pub struct NativeSelection {
    route: String,
    dark: bool,
    request_id: u64,
    surface: u64,
    permit: MutationPermit,
}
/// Submission knowledge returned by the native queue adapter.
#[derive(Debug, thiserror::Error)]
pub enum NativeSubmissionError {
    /// Rejected before enqueueing; the device may release its lease.
    #[error("native queue rejected submission: {0}")]
    Rejected(String),
    /// Delivery may have happened; retain ownership until acknowledgment or
    /// explicit host invalidation, and never submit this command again.
    #[error("native submission outcome is unknown: {0}")]
    OutcomeUnknown(String),
}
impl NativeSelection {
    pub fn route(&self) -> &str {
        &self.route
    }
    pub fn dark(&self) -> bool {
        self.dark
    }
    pub fn request_id(&self) -> u64 {
        self.request_id
    }
    pub fn surface(&self) -> u64 {
        self.surface
    }
    pub fn permit(&self) -> MutationPermit {
        self.permit.clone()
    }
}
struct PendingShell {
    admitted: AdmittedRequest,
    deadline: Instant,
    reported: bool,
    route: String,
    dark: bool,
}
enum NativeSettlement {
    Waiting,
    Applied,
    Rejected,
    AlreadyReported,
}
impl PendingShell {
    fn advance(
        &mut self,
        shell: &NativeShellSnapshot,
        ready: bool,
        now: Instant,
    ) -> NativeSettlement {
        if shell.ack() == self.admitted.request().request_id() {
            if self.reported {
                return NativeSettlement::AlreadyReported;
            }
            if !shell.applied {
                return NativeSettlement::Rejected;
            }
            if ready && shell.route() == self.route && shell.dark() == self.dark {
                return NativeSettlement::Applied;
            }
        }
        if !self.reported && self.deadline <= now {
            self.admitted.report_unknown(
                "native shell acknowledgment timed out; ownership retained until settlement",
            );
            self.reported = true;
        }
        NativeSettlement::Waiting
    }
}
struct PendingFrame {
    id: u64,
    response: Reply,
    lease: Option<MutationLease>,
    host: HostDescriptor,
    ready: Rc<Cell<bool>>,
    controls: Option<StoryControlsSnapshot>,
}
struct CaptureTicket {
    host: HostDescriptor,
    deadline: Instant,
    _lease: Option<MutationLease>,
}

/// Owner-thread bridge between bounded device admission, an attached GPUI root,
/// and a native shell queue. It retains each operation through native settlement,
/// deferred GPUI execution, or capture completion, even after a response deadline.
pub struct DeviceCoordinator<Root: EmbeddedRoot> {
    select_native: Box<dyn Fn(NativeSelection) -> Result<(), NativeSubmissionError>>,
    root: PhantomData<fn() -> Root>,
    endpoint: DeviceEndpoint,
    shell: Option<PendingShell>,
    captures: BTreeMap<u64, CaptureTicket>,
    frames: Vec<PendingFrame>,
}
impl<Root: EmbeddedRoot> DeviceCoordinator<Root> {
    /// Take an opted-in endpoint and the native queue submission function. The
    /// function enqueues once and retains the selection permit until dispatch.
    /// The adapter distinguishes rejection before enqueueing from an unknown
    /// submission outcome so caller failure never releases queued native work.
    pub fn new(
        endpoint: DeviceEndpoint,
        select_native: impl Fn(NativeSelection) -> Result<(), NativeSubmissionError> + 'static,
    ) -> Self {
        Self {
            endpoint,
            select_native: Box::new(select_native),
            root: PhantomData,
            shell: None,
            captures: BTreeMap::new(),
            frames: Vec::new(),
        }
    }
    /// Clone the gate for native lifecycle callbacks. Suspend it immediately on
    /// surface release before handing replacement to the GPUI owner.
    pub fn operation_gate(&self) -> OperationGate {
        self.endpoint.gate()
    }
    /// Install a fresh surface session and reopen admission atomically. Call on
    /// the GPUI owner after invalidating the old attachment. Pending shell work,
    /// frame waits, and capture tickets are revoked without replay.
    pub fn surface_replaced(&mut self) {
        self.endpoint.replace_session();
        if let Some(pending) = self.shell.take() {
            let (_, response, _) = pending.admitted.into_parts();
            let _ = response.try_send(Err(stale()));
        }
        self.captures.clear();
        for frame in self.frames.drain(..) {
            let _ = frame.response.try_send(Err(stale()));
        }
    }
    fn descriptor(
        &self,
        attachment: &GpuiHostAttachment<Root>,
        shell: &NativeShellSnapshot,
        window: &Window,
        cx: &App,
    ) -> Result<HostDescriptor, StorybookAutomationError> {
        let current = attachment.current_story(window, cx)?;
        let story = current
            .story
            .ok_or(StorybookAutomationError::NoActiveStory)?;
        let capabilities = attachment.capabilities();
        let mut capabilities: Vec<AutomationCapability> = capabilities.iter().collect();
        capabilities.extend([
            AutomationCapability::HostDiscovery,
            AutomationCapability::HostActions,
            AutomationCapability::DisplayCapture,
            AutomationCapability::StoryCapture,
        ]);
        let host = HostDescriptor::builder()
            .protocol_version(PROTOCOL_VERSION)
            .session(self.endpoint.session())
            .surface_revision(shell.surface())
            .route_revision(shell.revision().saturating_add(current.revision))
            .active_route(story.key)
            .native_route(shell.route().to_owned())
            .dark(shell.dark())
            .orientation(
                if shell.geometry().display_width() > shell.geometry().display_height() {
                    DisplayOrientation::Landscape
                } else {
                    DisplayOrientation::Portrait
                },
            )
            .geometry(shell.geometry())
            .capabilities(AutomationCapabilities::new(capabilities))
            .build();
        if !shell.active()
            || !host.agrees()
            || capture_region_bounds(host.active_route(), window, cx).is_none()
        {
            return Err(StorybookAutomationError::NoLiveHost);
        }
        let viewport = window.viewport_size();
        let geometry = host.geometry();
        if (f32::from(viewport.width) * window.scale_factor() - geometry.width() as f32).abs() > 1.0
            || (f32::from(viewport.height) * window.scale_factor() - geometry.height() as f32).abs()
                > 1.0
        {
            return Err(StorybookAutomationError::NoLiveHost);
        }
        Ok(host)
    }
    /// Poll after the ordinary application path has applied the observed native
    /// route and appearance. The snapshot's acknowledgment identifies the native
    /// selection request; its applied flag distinguishes dispatch from revocation.
    pub fn poll(
        &mut self,
        attachment: &GpuiHostAttachment<Root>,
        shell: &NativeShellSnapshot,
        window: &mut Window,
        cx: &mut App,
    ) {
        let mut waiting = Vec::new();
        for frame in std::mem::take(&mut self.frames) {
            if frame
                .lease
                .as_ref()
                .is_some_and(|lease| !lease.is_current())
            {
                let _ = frame.response.try_send(Err(stale()));
                continue;
            }
            if !frame.ready.get() {
                waiting.push(frame);
                continue;
            }
            let outcome = self
                .descriptor(attachment, shell, window, cx)
                .and_then(|host| {
                    if let Some(controls) = frame.controls {
                        // A control can reset IME composition/layout itself. Its completion
                        // requires the same attached route, rather than frozen geometry.
                        if host.session() != frame.host.session()
                            || host.surface_revision() != frame.host.surface_revision()
                            || host.active_route() != frame.host.active_route()
                            || host.native_route() != frame.host.native_route()
                            || host.dark() != frame.host.dark()
                        {
                            return Err(stale());
                        }
                        Ok(DeviceResult::Controls(controls))
                    } else {
                        if host != frame.host {
                            return Err(stale());
                        }
                        self.captures.insert(
                            frame.id,
                            CaptureTicket {
                                host: host.clone(),
                                deadline: Instant::now() + Duration::from_secs(15),
                                _lease: frame.lease,
                            },
                        );
                        Ok(DeviceResult::CaptureTicket {
                            ticket: frame.id,
                            host,
                        })
                    }
                });
            let _ = frame.response.try_send(outcome);
        }
        self.frames = waiting;
        self.captures
            .retain(|_, ticket| ticket.deadline > Instant::now());
        if let Some(mut pending) = self.shell.take() {
            let ready = self.descriptor(attachment, shell, window, cx).is_ok();
            match pending.advance(shell, ready, Instant::now()) {
                NativeSettlement::AlreadyReported => {},
                NativeSettlement::Rejected => {
                    let (_, response, _) = pending.admitted.into_parts();
                    let _ = response.try_send(Err(stale()));
                },
                NativeSettlement::Applied => {
                    self.execute(pending.admitted, attachment, shell, window, cx)
                },
                NativeSettlement::Waiting => self.shell = Some(pending),
            }
        }
        for _ in 0..32 {
            let Some(admitted) = self.endpoint.try_recv() else {
                break;
            };
            if let Err(error) = admitted
                .request()
                .validate_identity(&self.endpoint.session())
            {
                let (_, response, _) = admitted.into_parts();
                let _ = response.try_send(Err(error));
                continue;
            }
            let command = admitted.request().command();
            let native = match command {
                DeviceOperation::OpenStory { key } => attachment
                    .capabilities()
                    .require(AutomationCapability::Navigation)
                    .and_then(|()| attachment.get_story(key))
                    .map(|_| Some((key.clone(), shell.dark()))),
                DeviceOperation::DispatchHostAction {
                    action: HostAction::SetAppearance { dark },
                } => Ok(Some((shell.route().to_owned(), *dark))),
                DeviceOperation::RunSteps { request } => attachment
                    .validate_steps(request, false, window, cx)
                    .map(|_| {
                        let dark = requested_dark(request.presentation, shell.dark());
                        if request.story_key.is_some() || dark != shell.dark() {
                            Some((
                                request
                                    .story_key
                                    .clone()
                                    .unwrap_or_else(|| shell.route().to_owned()),
                                dark,
                            ))
                        } else {
                            None
                        }
                    }),
                DeviceOperation::RunScenario {
                    story_key,
                    scenario_key,
                } => resolve_scenario(attachment, story_key.as_deref(), scenario_key, window, cx)
                    .and_then(|scenario| {
                        let request = scenario.interaction_request(
                            story_key
                                .clone()
                                .unwrap_or_else(|| shell.route().to_owned()),
                        );
                        attachment.validate_steps(&request, true, window, cx)?;
                        Ok(Some((
                            story_key
                                .clone()
                                .unwrap_or_else(|| shell.route().to_owned()),
                            requested_dark(request.presentation, shell.dark()),
                        )))
                    }),
                _ => Ok(None),
            };
            match native {
                Err(error) => {
                    let (_, response, _) = admitted.into_parts();
                    let _ = response.try_send(Err(error));
                },
                Ok(Some((route, dark))) => {
                    let submitted = (self.select_native)(NativeSelection {
                        route: route.clone(),
                        dark,
                        request_id: admitted.request().request_id(),
                        surface: shell.surface(),
                        permit: admitted.permit().expect("native selection mutation lease"),
                    });
                    let reported = match submitted {
                        Err(NativeSubmissionError::Rejected(message)) => {
                            let (_, response, _) = admitted.into_parts();
                            let _ = response.try_send(Err(
                                StorybookAutomationError::CaptureUnavailable { message },
                            ));
                            continue;
                        },
                        Err(NativeSubmissionError::OutcomeUnknown(message)) => {
                            admitted.report_unknown(message);
                            true
                        },
                        Ok(()) => false,
                    };
                    self.shell = Some(PendingShell {
                        admitted,
                        deadline: Instant::now() + Duration::from_secs(5),
                        reported,
                        route,
                        dark,
                    });
                },
                Ok(None) => self.execute(admitted, attachment, shell, window, cx),
            }
        }
    }
    fn execute(
        &mut self,
        admitted: AdmittedRequest,
        attachment: &GpuiHostAttachment<Root>,
        shell: &NativeShellSnapshot,
        window: &mut Window,
        cx: &mut App,
    ) {
        if admitted.permit().is_some_and(|permit| !permit.is_current()) {
            let (_, response, _) = admitted.into_parts();
            let _ = response.try_send(Err(stale()));
            return;
        }
        let (request, response, lease) = admitted.into_parts();
        let id = request.request_id();
        let command = request.into_command();
        if let DeviceOperation::FinishCapture { ticket } = command {
            if let Some(index) = self
                .frames
                .iter()
                .position(|frame| frame.id == ticket && frame.controls.is_none())
            {
                let pending = self.frames.remove(index);
                let _ = pending.response.try_send(Err(stale()));
                drop(pending);
            }
            let outcome = self
                .captures
                .remove(&ticket)
                .ok_or_else(stale)
                .and_then(|ticket| {
                    let host = self.descriptor(attachment, shell, window, cx)?;
                    if ticket.host != host {
                        return Err(stale());
                    }
                    Ok(DeviceResult::Host(host))
                });
            let _ = response.try_send(outcome);
            return;
        }
        let result = (|| {
            let host = self.descriptor(attachment, shell, window, cx)?;
            match command {
                DeviceOperation::GetHost {} | DeviceOperation::DispatchHostAction { .. } => {
                    Ok(DeviceResult::Host(host))
                },
                DeviceOperation::ListStories {} => Ok(DeviceResult::Stories(attachment.stories())),
                DeviceOperation::GetStory { key } => {
                    attachment.get_story(&key).map(DeviceResult::Story)
                },
                DeviceOperation::CurrentStory {} | DeviceOperation::OpenStory { .. } => attachment
                    .current_story(window, cx)
                    .map(DeviceResult::Current),
                DeviceOperation::ReadControls {} => attachment
                    .read_controls(window, cx)
                    .map(DeviceResult::Controls),
                DeviceOperation::SetControl { key, value } => {
                    let controls = attachment.set_control(&key, value, window, cx)?;
                    let host = self.descriptor(attachment, shell, window, cx)?;
                    self.wait_for_frame(id, response.clone(), lease, host, Some(controls), window);
                    return Ok(None);
                },
                DeviceOperation::ResetControl { key } => {
                    let controls = attachment.reset_control(key.as_deref(), window, cx)?;
                    let host = self.descriptor(attachment, shell, window, cx)?;
                    self.wait_for_frame(id, response.clone(), lease, host, Some(controls), window);
                    return Ok(None);
                },
                DeviceOperation::ListActions {} => attachment
                    .list_actions(window, cx)
                    .map(DeviceResult::Actions),
                DeviceOperation::ListTargets {} => {
                    let (send, receive) = oneshot::channel();
                    attachment.list_targets(send, window, cx)?;
                    forward(receive, response.clone(), lease, DeviceResult::Targets, cx);
                    return Ok(None);
                },
                DeviceOperation::ReadValues {} => {
                    let (send, receive) = oneshot::channel();
                    attachment.read_values(send, window, cx)?;
                    forward(receive, response.clone(), lease, DeviceResult::Values, cx);
                    return Ok(None);
                },
                DeviceOperation::RunSteps { request } => {
                    let permit = lease.as_ref().expect("interaction lease").permit();
                    let (send, receive) = oneshot::channel();
                    attachment.run_steps(
                        AttachedInteraction::builder()
                            .request_id(id)
                            .request(request)
                            .fresh_fixture(false)
                            .response(send)
                            .progress(Arc::new(AtomicUsize::new(0)))
                            .lease(lease)
                            .admission_current(Rc::new(move || permit.is_current()))
                            .build(),
                        window,
                        cx,
                    );
                    forward(receive, response.clone(), (), DeviceResult::Interaction, cx);
                    return Ok(None);
                },
                DeviceOperation::RunScenario {
                    story_key,
                    scenario_key,
                } => {
                    let permit = lease.as_ref().expect("scenario lease").permit();
                    let scenario = resolve_scenario(
                        attachment,
                        story_key.as_deref(),
                        &scenario_key,
                        window,
                        cx,
                    )?;
                    let request = scenario
                        .interaction_request(story_key.unwrap_or_else(|| shell.route().to_owned()));
                    let (send, receive) = oneshot::channel();
                    attachment.run_steps(
                        AttachedInteraction::builder()
                            .request_id(id)
                            .request(request)
                            .fresh_fixture(true)
                            .response(send)
                            .progress(Arc::new(AtomicUsize::new(0)))
                            .lease(lease)
                            .admission_current(Rc::new(move || permit.is_current()))
                            .build(),
                        window,
                        cx,
                    );
                    forward(
                        receive,
                        response.clone(),
                        (),
                        move |interaction| {
                            DeviceResult::Scenario(StoryScenarioRunSnapshot {
                                scenario,
                                interaction,
                            })
                        },
                        cx,
                    );
                    return Ok(None);
                },
                DeviceOperation::ListHostActions {} => Ok(DeviceResult::HostActions(vec![
                    HostActionDescriptor::builder()
                        .name("set_appearance".to_owned())
                        .description(
                            "Select light or dark appearance through the native shell".to_owned(),
                        )
                        .input_schema(
                            serde_json::to_value(schemars::schema_for!(HostAction))
                                .expect("action schema"),
                        )
                        .build(),
                ])),
                DeviceOperation::PrepareCapture {} => {
                    self.wait_for_frame(id, response.clone(), lease, host, None, window);
                    return Ok(None);
                },
                DeviceOperation::FinishCapture { .. } => unreachable!("handled before readiness"),
            }
            .map(Some)
        })();
        match result {
            Ok(None) => {},
            Ok(Some(result)) => {
                let _ = response.try_send(Ok(result));
            },
            Err(error) => {
                let _ = response.try_send(Err(error));
            },
        }
    }
    fn wait_for_frame(
        &mut self,
        id: u64,
        response: Reply,
        lease: Option<MutationLease>,
        host: HostDescriptor,
        controls: Option<StoryControlsSnapshot>,
        window: &mut Window,
    ) {
        let ready = Rc::new(Cell::new(false));
        let rendered = ready.clone();
        window.refresh();
        window.on_next_frame(move |window, _| {
            window.refresh();
            window.on_next_frame(move |_, _| rendered.set(true));
        });
        self.frames.push(PendingFrame {
            id,
            response,
            lease,
            host,
            ready,
            controls,
        });
    }
}
fn requested_dark(presentation: Option<StoryPresentation>, current: bool) -> bool {
    match presentation.map(|presentation| presentation.background) {
        Some(StoryCanvasBackground::Dark) => true,
        Some(StoryCanvasBackground::Light) => false,
        _ => current,
    }
}
fn stale() -> StorybookAutomationError {
    StorybookAutomationError::StaleHost {
        message: "native surface or route changed during the operation".to_owned(),
    }
}

fn forward<T: 'static, Lease: 'static>(
    receiver: oneshot::Receiver<Result<T, StorybookAutomationError>>,
    response: Reply,
    lease: Lease,
    map: impl FnOnce(T) -> DeviceResult + 'static,
    cx: &mut App,
) {
    cx.spawn(async move |_| {
        let result = receiver
            .await
            .unwrap_or(Err(StorybookAutomationError::NoLiveHost));
        let _ = response.try_send(result.map(map));
        drop(lease);
    })
    .detach();
}
fn resolve_scenario<Root: EmbeddedRoot>(
    attachment: &GpuiHostAttachment<Root>,
    key: Option<&str>,
    scenario_key: &str,
    window: &Window,
    cx: &App,
) -> Result<StoryScenarioSnapshot, StorybookAutomationError> {
    let story = match key {
        Some(key) => attachment.get_story(key)?,
        None => attachment
            .current_story(window, cx)?
            .story
            .ok_or(StorybookAutomationError::NoActiveStory)?,
    };
    find_scenario(&story, scenario_key)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpStream;

    #[test]
    fn native_response_deadline_retains_ownership_until_acknowledgment() {
        let endpoint = DeviceEndpoint::listen(0, "surface-1".to_owned()).unwrap();
        let mut stream = TcpStream::connect(("127.0.0.1", endpoint.port())).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        write_frame(
            &mut stream,
            &DeviceRequest::builder()
                .protocol_version(PROTOCOL_VERSION)
                .session("surface-1".to_owned())
                .request_id(17)
                .command(DeviceOperation::OpenStory {
                    key: "counter".to_owned(),
                })
                .build(),
        )
        .unwrap();
        let deadline = Instant::now() + Duration::from_secs(3);
        let admitted = loop {
            if let Some(request) = endpoint.try_recv() {
                break request;
            }
            assert!(Instant::now() < deadline, "native work was not admitted");
            std::thread::yield_now();
        };
        let permit = admitted.permit().unwrap();
        let now = Instant::now();
        let mut pending = PendingShell {
            admitted,
            deadline: now,
            reported: false,
            route: "counter".to_owned(),
            dark: false,
        };
        let mut shell = NativeShellSnapshot::builder()
            .route("counter".to_owned())
            .dark(false)
            .revision(1)
            .surface(1)
            .active(true)
            .geometry(
                SurfaceGeometry::builder()
                    .x(0)
                    .y(0)
                    .width(2)
                    .height(3)
                    .scale(1.)
                    .display_width(2)
                    .display_height(3)
                    .build(),
            )
            .build();
        assert!(matches!(
            pending.advance(&shell, false, now),
            NativeSettlement::Waiting
        ));
        assert!(matches!(
            read_frame::<DeviceResponse>(&mut stream)
                .unwrap()
                .into_outcome(),
            Err(StorybookAutomationError::OutcomeUnknown { request_id: 17, .. })
        ));
        drop(stream);
        assert!(endpoint.gate().busy());
        assert!(permit.is_current());
        assert!(matches!(
            endpoint.gate().acquire(),
            Err(StorybookAutomationError::AutomationBusy)
        ));
        shell.ack = 17;
        assert!(matches!(
            pending.advance(&shell, false, now),
            NativeSettlement::AlreadyReported
        ));
        drop(pending);
        assert!(!endpoint.gate().busy());
        assert!(!permit.is_current());
    }
}
