//! Bind automation to a window and root supplied by the application.

use crate::{interaction::*, regions::*, snapshot::*};
use gpui::{App, Entity, Render, WeakEntity, Window};
use std::{
    cell::Cell,
    collections::{BTreeMap, BTreeSet},
    rc::Rc,
    sync::{Arc, atomic::AtomicUsize},
};
use tokio::sync::oneshot;

/// Application-owned route, control, and fixture policy for an embedded root.
/// Native shell adapters acknowledge their own state before selecting the GPUI
/// route. Ad-hoc selection preserves state; fixture recreation is explicit.
pub trait EmbeddedRoot: Render + 'static {
    /// Application-owned availability of this public surface. Keep this a local
    /// state check; authentication, loading, and permissions belong to the app.
    fn readiness(
        &self,
        _cx: &App,
    ) -> Result<(), gpui_storybook_automation::wire::HostReadinessIssue> {
        Ok(())
    }
    fn active_route(&self, cx: &App) -> String;
    /// Monotonic public state revision, including user-driven changes.
    fn revision(&self, cx: &App) -> u64;
    fn select_route(
        &mut self,
        route: &str,
        window: &mut Window,
        cx: &mut gpui::Context<Self>,
    ) -> Result<(), StorybookAutomationError>;
    /// Actions available on this route. The default exposes no actions.
    fn action_scope(&self, _route: &str, _cx: &App) -> BTreeSet<String> {
        BTreeSet::new()
    }
    /// Typed controls available on this route. The default exposes no controls.
    fn control_catalog(&self, _route: &str, _cx: &App) -> Vec<ControlSpec> {
        Vec::new()
    }
    fn read_control(&self, key: &str, _cx: &App) -> Result<ControlValue, ControlError> {
        Err(ControlError::UnknownControl {
            key: key.to_owned(),
        })
    }
    fn set_control(
        &mut self,
        key: &str,
        _value: ControlValue,
        _window: &mut Window,
        _cx: &mut gpui::Context<Self>,
    ) -> Result<(), ControlError> {
        Err(ControlError::UnknownControl {
            key: key.to_owned(),
        })
    }
    fn reset_control(
        &mut self,
        key: Option<&str>,
        _window: &mut Window,
        _cx: &mut gpui::Context<Self>,
    ) -> Result<(), ControlError> {
        Err(ControlError::UnknownControl {
            key: key.unwrap_or_default().to_owned(),
        })
    }
    fn apply_presentation(
        &mut self,
        _presentation: StoryPresentation,
        _window: &mut Window,
        _cx: &mut gpui::Context<Self>,
    ) -> Result<(), StorybookAutomationError> {
        Err(StorybookAutomationError::UnsupportedCapability {
            capability: AutomationCapability::Presentation,
        })
    }
    fn recreate_fixture(
        &mut self,
        _route: &str,
        _window: &mut Window,
        _cx: &mut gpui::Context<Self>,
    ) -> Result<(), StorybookAutomationError> {
        Err(StorybookAutomationError::UnsupportedCapability {
            capability: AutomationCapability::FreshScenarios,
        })
    }
    /// Apply an exact native appearance. Override this for application schemes
    /// such as OLED; the default maps the standard light/dark IDs to a responsive canvas.
    fn apply_host_appearance(
        &mut self,
        appearance: &gpui_storybook_automation::wire::HostAppearance,
        window: &mut Window,
        cx: &mut gpui::Context<Self>,
    ) -> Result<(), StorybookAutomationError> {
        if !matches!(appearance.id(), "light" | "dark") {
            return Err(StorybookAutomationError::UnsupportedCapability {
                capability: AutomationCapability::Presentation,
            });
        }
        self.apply_presentation(
            StoryPresentation {
                background: if appearance.is_dark() {
                    StoryCanvasBackground::Dark
                } else {
                    StoryCanvasBackground::Light
                },
                viewport: StoryViewportPreset::Responsive,
            },
            window,
            cx,
        )
    }
}

#[cfg(test)]
mod tests;

/// A batch admitted by the host's exclusive operation owner.
/// Its lease moves into frame callbacks and survives response cancellation.
#[derive(bon::Builder)]
pub struct AttachedInteraction<Lease: 'static> {
    pub(crate) request_id: u64,
    pub(crate) request: StoryInteractionRequest,
    pub(crate) fresh_fixture: bool,
    pub(crate) response:
        oneshot::Sender<Result<StoryInteractionSnapshot, StorybookAutomationError>>,
    pub(crate) progress: Arc<AtomicUsize>,
    pub(crate) lease: Lease,
    /// Check a platform admission permit before preparation and at each deferred
    /// execution boundary. The retained lease owns work; this check revokes it
    /// when the native surface owner invalidates its host.
    pub(crate) admission_current: Option<Rc<dyn Fn() -> bool>>,
}

/// Owning-thread attachment to an existing root; it creates no GPUI application.
/// Hold this with the surface lifecycle and call [`Self::invalidate`] before
/// surface replacement. A replacement gets a fresh attachment and catalog.
pub struct GpuiHostAttachment<R: EmbeddedRoot> {
    root: WeakEntity<R>,
    window: gpui::AnyWindowHandle,
    catalog: BTreeMap<String, StorySnapshot>,
    capabilities: AutomationCapabilities,
    provider: Rc<dyn InteractionCaptureProvider>,
    revision: Cell<u64>,
    valid: Rc<Cell<bool>>,
}

#[derive(Debug, thiserror::Error)]
pub enum AttachmentError {
    #[error("route `{0}` is registered more than once")]
    DuplicateRoute(String),
    #[error("route keys must be nonempty and equal their root capture route")]
    InvalidRoute,
    #[error(
        "embedded attachment uses observed window geometry; sizing belongs to the platform owner"
    )]
    StorySizing,
}

impl<R: EmbeddedRoot> GpuiHostAttachment<R> {
    pub fn attach(
        root: &Entity<R>,
        catalog: Vec<StorySnapshot>,
        capabilities: AutomationCapabilities,
        provider: Rc<dyn InteractionCaptureProvider>,
        window: &Window,
        cx: &mut App,
    ) -> Result<Self, AttachmentError> {
        if capabilities.contains(AutomationCapability::StorySizing) {
            return Err(AttachmentError::StorySizing);
        }
        let mut routes = BTreeMap::new();
        for story in catalog {
            if story.key.is_empty() || story.key != story.capture_route_id {
                return Err(AttachmentError::InvalidRoute);
            }
            let key = story.key.clone();
            if routes.insert(key.clone(), story).is_some() {
                return Err(AttachmentError::DuplicateRoute(key));
            }
        }
        invalidate_window_regions(window, cx);
        let valid = Rc::new(Cell::new(true));
        let provider = Rc::new(AttachedProvider {
            valid: valid.clone(),
            root: root.downgrade(),
            window: window.window_handle(),
            inner: provider,
        });
        Ok(Self {
            root: root.downgrade(),
            window: window.window_handle(),
            catalog: routes,
            capabilities,
            provider,
            revision: Cell::new(0),
            valid,
        })
    }

    pub fn capabilities(&self) -> &AutomationCapabilities {
        &self.capabilities
    }
    /// Check application availability before dispatching through a custom host.
    pub fn check_readiness(
        &self,
        window: &Window,
        cx: &App,
    ) -> Result<(), StorybookAutomationError> {
        self.root(window)?
            .read(cx)
            .readiness(cx)
            .map_err(|issue| StorybookAutomationError::HostNotReady { issue })
    }
    pub fn stories(&self) -> Vec<StorySnapshot> {
        self.catalog.values().cloned().collect()
    }
    pub fn get_story(&self, route: &str) -> Result<StorySnapshot, StorybookAutomationError> {
        self.catalog
            .get(route)
            .cloned()
            .ok_or_else(|| StorybookAutomationError::StoryNotFound {
                key: route.to_owned(),
            })
    }
    fn root(&self, window: &Window) -> Result<Entity<R>, StorybookAutomationError> {
        if !self.valid.get() || self.window != window.window_handle() {
            return Err(StorybookAutomationError::NoLiveHost);
        }
        self.root
            .upgrade()
            .ok_or(StorybookAutomationError::NoLiveHost)
    }
    pub fn invalidate(&self, window: &Window, cx: &mut App) {
        self.valid.set(false);
        if self.window == window.window_handle() {
            invalidate_window_regions(window, cx);
        }
    }
    pub fn current_story(
        &self,
        window: &Window,
        cx: &App,
    ) -> Result<StoryCurrentSnapshot, StorybookAutomationError> {
        let root = self.root(window)?;
        let route = root.read(cx).active_route(cx);
        Ok(StoryCurrentSnapshot {
            story: Some(self.get_story(&route)?),
            revision: self
                .revision
                .get()
                .saturating_add(root.read(cx).revision(cx)),
        })
    }
    pub fn open_story(
        &self,
        route: &str,
        window: &mut Window,
        cx: &mut App,
    ) -> Result<StoryCurrentSnapshot, StorybookAutomationError> {
        self.capabilities
            .require(AutomationCapability::Navigation)?;
        self.get_story(route)?;
        let root = self.root(window)?;
        root.update(cx, |root, cx| root.select_route(route, window, cx))?;
        let actual = root.read(cx).active_route(cx);
        if actual != route {
            return Err(StorybookAutomationError::HostDisconnected {
                message: format!("application selected `{actual}` while `{route}` was requested"),
                steps_dispatched: 0,
            });
        }
        invalidate_window_regions(window, cx);
        self.revision.set(self.revision.get().saturating_add(1));
        window.refresh();
        self.current_story(window, cx)
    }
    pub fn read_controls(
        &self,
        window: &Window,
        cx: &App,
    ) -> Result<StoryControlsSnapshot, StorybookAutomationError> {
        self.capabilities.require(AutomationCapability::Controls)?;
        let story = self
            .current_story(window, cx)?
            .story
            .ok_or(StorybookAutomationError::NoActiveStory)?;
        let root = self.root(window)?;
        let controls = root
            .read(cx)
            .control_catalog(&story.key, cx)
            .into_iter()
            .map(|spec| {
                let value = root
                    .read(cx)
                    .read_control(&spec.key, cx)
                    .map_err(control_error)?;
                Ok(ControlSnapshot { spec, value })
            })
            .collect::<Result<_, StorybookAutomationError>>()?;
        Ok(StoryControlsSnapshot { story, controls })
    }
    pub fn set_control(
        &self,
        key: &str,
        value: ControlValue,
        window: &mut Window,
        cx: &mut App,
    ) -> Result<StoryControlsSnapshot, StorybookAutomationError> {
        self.capabilities
            .require(AutomationCapability::ControlMutation)?;
        let snapshot = self.read_controls(window, cx)?;
        let spec = snapshot
            .controls
            .iter()
            .find(|control| control.spec.key == key)
            .ok_or_else(|| {
                control_error(ControlError::UnknownControl {
                    key: key.to_owned(),
                })
            })?;
        validate_control_value(&spec.spec, &value).map_err(control_error)?;
        self.root(window)?
            .update(cx, |root, cx| root.set_control(key, value, window, cx))
            .map_err(control_error)?;
        window.refresh();
        self.read_controls(window, cx)
    }
    pub fn reset_control(
        &self,
        key: Option<&str>,
        window: &mut Window,
        cx: &mut App,
    ) -> Result<StoryControlsSnapshot, StorybookAutomationError> {
        self.capabilities
            .require(AutomationCapability::ControlMutation)?;
        self.root(window)?
            .update(cx, |root, cx| root.reset_control(key, window, cx))
            .map_err(control_error)?;
        window.refresh();
        self.read_controls(window, cx)
    }
    pub fn list_actions(
        &self,
        window: &Window,
        cx: &App,
    ) -> Result<Vec<StoryActionSnapshot>, StorybookAutomationError> {
        self.capabilities.require(AutomationCapability::Actions)?;
        let root = self.root(window)?;
        let route = root.read(cx).active_route(cx);
        let scope = root.read(cx).action_scope(&route, cx);
        Ok(list_registered_actions(cx)
            .into_iter()
            .filter(|action| scope.contains(&action.name))
            .collect())
    }
    pub fn read_values(
        &self,
        response: oneshot::Sender<Result<StorySemanticValuesSnapshot, StorybookAutomationError>>,
        window: &mut Window,
        cx: &App,
    ) -> Result<(), StorybookAutomationError> {
        self.capabilities
            .require(AutomationCapability::SemanticValues)?;
        let story = self
            .current_story(window, cx)?
            .story
            .ok_or(StorybookAutomationError::NoActiveStory)?;
        let provider = self.provider.clone();
        window.refresh();
        window.on_next_frame(move |window, cx| {
            let result = provider
                .validate_host(&story.capture_route_id, window, cx)
                .and_then(|()| rendered_semantic_values(story, window, cx));
            let _ = response.send(result);
        });
        Ok(())
    }
    pub fn list_targets(
        &self,
        response: oneshot::Sender<
            Result<StoryInteractionTargetsSnapshot, StorybookAutomationError>,
        >,
        window: &mut Window,
        cx: &App,
    ) -> Result<(), StorybookAutomationError> {
        self.capabilities
            .require(AutomationCapability::SemanticTargets)?;
        let story = self
            .current_story(window, cx)?
            .story
            .ok_or(StorybookAutomationError::NoActiveStory)?;
        schedule_interaction_target_listing(story, response, self.provider.clone(), window);
        Ok(())
    }

    /// Preflight the whole batch, then execute on the attached window. The
    /// caller owns admission and passes a lease covering native and GPUI work.
    pub fn validate_steps(
        &self,
        request: &StoryInteractionRequest,
        fresh_fixture: bool,
        window: &Window,
        cx: &App,
    ) -> Result<(), StorybookAutomationError> {
        self.capabilities.validate_interaction(request)?;
        if fresh_fixture {
            self.capabilities
                .require(AutomationCapability::FreshScenarios)?;
        }
        let root = self.root(window)?;
        let route = request
            .story_key
            .clone()
            .unwrap_or_else(|| root.read(cx).active_route(cx));
        self.get_story(&route)?;
        prepare_interaction_steps(&request.steps, &root.read(cx).action_scope(&route, cx), cx)?;
        let specs = root.read(cx).control_catalog(&route, cx);
        for (key, value) in &request.controls {
            let spec = specs
                .iter()
                .find(|spec| spec.key == *key)
                .ok_or_else(|| control_error(ControlError::UnknownControl { key: key.clone() }))?;
            validate_control_value(spec, value).map_err(control_error)?;
        }
        Ok(())
    }

    pub fn run_steps<Lease: 'static>(
        &self,
        admitted: AttachedInteraction<Lease>,
        window: &mut Window,
        cx: &mut App,
    ) {
        let AttachedInteraction {
            request_id,
            request,
            fresh_fixture,
            response,
            progress,
            lease,
            admission_current,
        } = admitted;
        let prepare = (|| {
            if admission_current.as_ref().is_some_and(|current| !current()) {
                return Err(StorybookAutomationError::StaleHost {
                    message: "platform operation admission was revoked".to_owned(),
                });
            }
            self.capabilities.validate_interaction(&request)?;
            if fresh_fixture {
                self.capabilities
                    .require(AutomationCapability::FreshScenarios)?;
            }
            let root = self.root(window)?;
            let route = request
                .story_key
                .clone()
                .unwrap_or_else(|| root.read(cx).active_route(cx));
            let story = self.get_story(&route)?;
            let scope = root.read(cx).action_scope(&route, cx);
            let steps = prepare_interaction_steps(&request.steps, &scope, cx)?;
            let specs = root.read(cx).control_catalog(&route, cx);
            for (key, value) in &request.controls {
                let spec = specs.iter().find(|spec| spec.key == *key).ok_or_else(|| {
                    control_error(ControlError::UnknownControl { key: key.clone() })
                })?;
                validate_control_value(spec, value).map_err(control_error)?;
            }
            if request.story_key.is_some() {
                self.open_story(&route, window, cx)?;
            }
            if fresh_fixture {
                root.update(cx, |root, cx| root.recreate_fixture(&route, window, cx))?;
                invalidate_window_regions(window, cx);
            }
            if let Some(presentation) = request.presentation {
                root.update(cx, |root, cx| {
                    root.apply_presentation(presentation, window, cx)
                })?;
            }
            for (key, value) in &request.controls {
                root.update(cx, |root, cx| {
                    root.set_control(key, value.clone(), window, cx)
                })
                .map_err(control_error)?;
            }
            Ok((story, steps))
        })();
        match prepare {
            Ok((story, steps)) => {
                window.refresh();
                // GPUI invokes next-frame callbacks before that frame's layout.
                // Allow the newly selected/recreated root to prepaint before the
                // executor reads its region registry on the following frame.
                let provider: Rc<dyn InteractionCaptureProvider> = match admission_current {
                    Some(current) => Rc::new(AdmissionProvider {
                        current,
                        inner: self.provider.clone(),
                    }),
                    None => self.provider.clone(),
                };
                window.on_next_frame(move |window, _| {
                    schedule_story_interaction(
                        PreparedStoryInteraction::builder()
                            .provider(provider)
                            .request_id(request_id)
                            .story(story)
                            .steps(steps)
                            .postconditions(request.postconditions)
                            .maybe_capture(request.capture)
                            .response(response)
                            .progress(progress)
                            .operation(lease)
                            .build(),
                        window,
                    );
                });
            },
            Err(error) => {
                let _ = response.send(Err(error));
            },
        }
    }
}

struct AdmissionProvider {
    current: Rc<dyn Fn() -> bool>,
    inner: Rc<dyn InteractionCaptureProvider>,
}
impl InteractionCaptureProvider for AdmissionProvider {
    fn validate_host(
        &self,
        route: &str,
        window: &Window,
        cx: &App,
    ) -> Result<(), StorybookAutomationError> {
        if !(self.current)() {
            return Err(StorybookAutomationError::StaleHost {
                message: "platform operation admission was revoked".to_owned(),
            });
        }
        self.inner.validate_host(route, window, cx)
    }
    fn ensure_visible(
        &self,
        route: &str,
        window: &mut Window,
        cx: &App,
    ) -> Result<bool, StorybookAutomationError> {
        self.validate_host(route, window, cx)?;
        self.inner.ensure_visible(route, window, cx)
    }
    fn capture(
        &self,
        id: u64,
        request: StoryScreenshotRequest,
        story: StorySnapshot,
        window: &mut Window,
        cx: &App,
    ) -> Result<StoryCaptureSnapshot, StorybookAutomationError> {
        self.validate_host(&story.capture_route_id, window, cx)?;
        self.inner.capture(id, request, story, window, cx)
    }
}

struct AttachedProvider<R: EmbeddedRoot> {
    valid: Rc<Cell<bool>>,
    root: WeakEntity<R>,
    window: gpui::AnyWindowHandle,
    inner: Rc<dyn InteractionCaptureProvider>,
}
impl<R: EmbeddedRoot> InteractionCaptureProvider for AttachedProvider<R> {
    fn validate_host(
        &self,
        route: &str,
        window: &Window,
        cx: &App,
    ) -> Result<(), StorybookAutomationError> {
        if !self.valid.get()
            || self.window != window.window_handle()
            || !self
                .root
                .upgrade()
                .is_some_and(|root| root.read(cx).active_route(cx) == route)
        {
            return Err(StorybookAutomationError::StaleHost {
                message: "attached root, surface, or route was replaced".to_owned(),
            });
        }
        self.inner.validate_host(route, window, cx)
    }
    fn ensure_visible(
        &self,
        route: &str,
        window: &mut Window,
        cx: &App,
    ) -> Result<bool, StorybookAutomationError> {
        self.validate_host(route, window, cx)?;
        self.inner.ensure_visible(route, window, cx)
    }
    fn capture(
        &self,
        request_id: u64,
        request: StoryScreenshotRequest,
        story: StorySnapshot,
        window: &mut Window,
        cx: &App,
    ) -> Result<StoryCaptureSnapshot, StorybookAutomationError> {
        self.validate_host(&story.capture_route_id, window, cx)?;
        self.inner.capture(request_id, request, story, window, cx)
    }
}

fn control_error(error: ControlError) -> StorybookAutomationError {
    StorybookAutomationError::ControlOperationFailed {
        message: error.to_string(),
    }
}
