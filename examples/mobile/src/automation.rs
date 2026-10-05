//! Example route owner: native acknowledgment, GPUI frame, and device lease.

use super::android::ShellState;
use gpui_kit::{App, Entity, Window};
use gpui_storybook_automation::{wire::*, *};
use gpui_storybook_automation_gpui::{
    AttachedInteraction, GpuiHostAttachment, regions::capture_region_bounds,
};
use gpui_storybook_example_embedded::DemoRoot;
use gpui_storybook_mobile::{AdmittedRequest, DEVICE_PORT, DeviceEndpoint, MutationLease};
use std::{
    cell::Cell,
    collections::BTreeMap,
    rc::Rc,
    sync::{Arc, atomic::AtomicUsize, mpsc::SyncSender},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tokio::sync::oneshot;

type Reply = SyncSender<Result<DeviceResult, StorybookAutomationError>>;
static NATIVE_GATE: std::sync::OnceLock<gpui_storybook_mobile::OperationGate> =
    std::sync::OnceLock::new();
pub(super) fn native_input_allowed() -> bool {
    NATIVE_GATE.get().is_none_or(|gate| !gate.busy())
}
struct PendingShell {
    admitted: AdmittedRequest,
    deadline: Instant,
    route: String,
    dark: bool,
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

pub(super) struct AutomationHost {
    endpoint: DeviceEndpoint,
    process_session: String,
    shell: Option<PendingShell>,
    captures: BTreeMap<u64, CaptureTicket>,
    frames: Vec<PendingFrame>,
}
impl AutomationHost {
    pub(super) fn new() -> Self {
        let process_session = format!(
            "{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        );
        let endpoint = DeviceEndpoint::listen(DEVICE_PORT, process_session.clone())
            .expect("opted-in loopback endpoint");
        let _ = NATIVE_GATE.set(endpoint.gate());
        Self {
            endpoint,
            process_session,
            shell: None,
            captures: BTreeMap::new(),
            frames: Vec::new(),
        }
    }
    pub(super) fn surface_replaced(&mut self, surface: u64) {
        self.endpoint
            .replace_session(format!("{}-{surface}", self.process_session));
        if let Some(pending) = self.shell.take() {
            let (_, response, _) = pending.admitted.into_parts();
            let _ = response.send(Err(stale()));
        }
        self.captures.clear();
        for frame in self.frames.drain(..) {
            let _ = frame.response.send(Err(stale()));
        }
    }
    fn descriptor(
        &self,
        attachment: &GpuiHostAttachment<DemoRoot>,
        shell: &ShellState,
        window: &Window,
        cx: &App,
    ) -> Result<HostDescriptor, StorybookAutomationError> {
        let current = attachment.current_story(window, cx)?;
        let story = current
            .story
            .ok_or(StorybookAutomationError::NoActiveStory)?;
        let capabilities = gpui_storybook_example_embedded::capabilities();
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
    pub(super) fn poll(
        &mut self,
        attachment: &GpuiHostAttachment<DemoRoot>,
        _root: &Entity<DemoRoot>,
        shell: &ShellState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let mut waiting = Vec::new();
        for frame in std::mem::take(&mut self.frames) {
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
            let _ = frame.response.send(outcome);
        }
        self.frames = waiting;
        self.captures
            .retain(|_, ticket| ticket.deadline > Instant::now());
        if let Some(pending) = self.shell.take() {
            if shell.ack() == pending.admitted.request().request_id()
                && shell.route() == pending.route
                && shell.dark() == pending.dark
                && self.descriptor(attachment, shell, window, cx).is_ok()
            {
                self.execute(pending.admitted, attachment, shell, window, cx);
            } else if pending.deadline <= Instant::now() {
                let id = pending.admitted.request().request_id();
                let (_, response, _) = pending.admitted.into_parts();
                let _ = response.send(Err(StorybookAutomationError::OutcomeUnknown {
                    request_id: id,
                    message: "native shell acknowledgment timed out".to_owned(),
                }));
            } else {
                self.shell = Some(pending);
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
                let _ = response.send(Err(error));
                continue;
            }
            let command = admitted.request().command();
            let native = match command {
                DeviceOperation::OpenStory { key } => attachment
                    .get_story(key)
                    .map(|_| Some((key.clone(), shell.dark()))),
                DeviceOperation::DispatchHostAction {
                    action: HostAction::SetAppearance { dark },
                } => Ok(Some((shell.route().to_owned(), *dark))),
                DeviceOperation::RunSteps { request } => attachment
                    .validate_steps(request, false, window, cx)
                    .map(|_| request.story_key.clone().map(|key| (key, shell.dark()))),
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
                            shell.dark(),
                        )))
                    }),
                _ => Ok(None),
            };
            match native {
                Err(error) => {
                    let (_, response, _) = admitted.into_parts();
                    let _ = response.send(Err(error));
                },
                Ok(Some((route, dark))) => {
                    if let Err(message) =
                        select_native(&route, dark, admitted.request().request_id())
                    {
                        let (_, response, _) = admitted.into_parts();
                        let _ = response.send(Err(StorybookAutomationError::CaptureUnavailable {
                            message,
                        }));
                    } else {
                        self.shell = Some(PendingShell {
                            admitted,
                            deadline: Instant::now() + Duration::from_secs(5),
                            route,
                            dark,
                        });
                    }
                },
                Ok(None) => self.execute(admitted, attachment, shell, window, cx),
            }
        }
    }
    fn execute(
        &mut self,
        admitted: AdmittedRequest,
        attachment: &GpuiHostAttachment<DemoRoot>,
        shell: &ShellState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let (request, response, lease) = admitted.into_parts();
        let id = request.request_id();
        let command = request.into_command();
        if let DeviceOperation::FinishCapture { ticket } = command {
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
            let _ = response.send(outcome);
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
                    let (send, receive) = oneshot::channel();
                    attachment.run_steps(
                        AttachedInteraction::builder()
                            .request_id(id)
                            .request(request)
                            .fresh_fixture(false)
                            .response(send)
                            .progress(Arc::new(AtomicUsize::new(0)))
                            .lease(lease)
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
                let _ = response.send(Ok(result));
            },
            Err(error) => {
                let _ = response.send(Err(error));
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
        if let Err(error) = &result {
            log::warn!("automation owner settled: {error}");
        }
        let _ = response.send(result.map(map));
        drop(lease);
    })
    .detach();
}
fn resolve_scenario(
    attachment: &GpuiHostAttachment<DemoRoot>,
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
fn select_native(route: &str, dark: bool, request: u64) -> Result<(), String> {
    gpui_mobile::android::jni::with_env(|env| {
        let activity = gpui_mobile::android::jni::activity(env)?;
        let route = env.new_string(route).map_err(|error| error.to_string())?;
        env.call_method(
            &activity,
            jni::jni_str!("gpuiSelect"),
            jni::jni_sig!("(Ljava/lang/String;ZJ)V"),
            &[
                jni::objects::JValue::Object(&route),
                jni::objects::JValue::Bool(dark),
                jni::objects::JValue::Long(request as i64),
            ],
        )
        .map_err(|error| error.to_string())?;
        Ok(())
    })
}
