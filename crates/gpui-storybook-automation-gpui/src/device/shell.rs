//! Native lifecycle and committed-frame state shared by platform adapters.

use super::*;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

/// One committed native frame. The application supplies its public state;
/// the adapter owns revisions, geometry generations, and acknowledgment.
#[derive(Clone, Debug, Deserialize, Serialize, bon::Builder)]
#[serde(deny_unknown_fields)]
pub struct NativeObservation {
    route: String,
    appearance: String,
    geometry: Option<SurfaceGeometry>,
    #[builder(default)]
    actions: Vec<HostActionDescriptor>,
    #[builder(default)]
    values: Vec<StorySemanticValueSnapshot>,
}

/// Messages sent by a native lifecycle owner. `Frame` is published after native
/// composition and display commitment, never merely after state assignment.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "event", rename_all = "snake_case", deny_unknown_fields)]
pub enum NativeShellEvent {
    Active {
        active: bool,
    },
    SurfaceReleased {},
    Frame {
        observation: NativeObservation,
        #[serde(with = "native_request_id")]
        request_id: u64,
    },
    Settled {
        #[serde(with = "native_request_id")]
        request_id: u64,
        applied: bool,
    },
}

/// One admitted selection dispatched to the native owner. Application actions
/// remain typed public JSON; this payload contains no transport permit.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NativeDispatch {
    route: String,
    appearance: String,
    #[serde(with = "native_request_id")]
    request_id: u64,
    action: Option<HostAction>,
}
impl NativeDispatch {
    pub fn route(&self) -> &str {
        &self.route
    }
    pub fn appearance(&self) -> &str {
        &self.appearance
    }
    pub fn request_id(&self) -> u64 {
        self.request_id
    }
    pub fn action(&self) -> Option<&HostAction> {
        self.action.as_ref()
    }
}

struct ShellState {
    snapshot: NativeShellSnapshot,
    pending: Option<NativeSelection>,
}

/// Thread-safe native shell state. Clone into JNI callbacks; retain the device
/// coordinator on the GPUI owner. Surface release revokes permits immediately.
#[derive(Clone)]
pub struct NativeShellHandle {
    gate: OperationGate,
    state: Arc<Mutex<ShellState>>,
}
impl NativeShellHandle {
    pub fn new(
        gate: OperationGate,
        appearances: Vec<HostAppearance>,
    ) -> Result<Self, StorybookAutomationError> {
        if appearances.is_empty()
            || appearances.len() > 32
            || appearances.iter().any(|choice| !choice.validate())
            || appearances
                .iter()
                .map(HostAppearance::id)
                .collect::<std::collections::BTreeSet<_>>()
                .len()
                != appearances.len()
        {
            return Err(StorybookAutomationError::ControlOperationFailed {
                message: "native appearance catalog must contain 1–32 valid unique IDs".to_owned(),
            });
        }
        Ok(Self {
            gate,
            state: Arc::new(Mutex::new(ShellState {
                snapshot: NativeShellSnapshot::builder()
                    .route(String::new())
                    .appearance(appearances[0].clone())
                    .appearances(appearances)
                    .active(false)
                    .revision(0)
                    .surface(0)
                    .build(),
                pending: None,
            })),
        })
    }

    pub fn snapshot(&self) -> NativeShellSnapshot {
        self.state
            .lock()
            .expect("native shell state")
            .snapshot
            .clone()
    }

    /// Native input and remote work share the same exclusive operation owner.
    pub fn is_input_allowed(&self) -> bool {
        let state = self.state.lock().expect("native shell state");
        state.snapshot.active && state.snapshot.geometry.is_some() && !self.gate.busy()
    }

    /// Check immediately before native dispatch, on the lifecycle owner thread.
    pub fn is_current(&self, request: u64) -> bool {
        let state = self.state.lock().expect("native shell state");
        state.snapshot.active
            && state.snapshot.geometry.is_some()
            && state.pending.as_ref().is_some_and(|selection| {
                selection.request_id() == request
                    && selection.surface() == state.snapshot.surface
                    && selection.permit().is_current()
            })
    }

    /// Decode a bounded native event. Applications forward one JNI callback here.
    pub fn publish_json(&self, event: &str) -> Result<(), StorybookAutomationError> {
        if event.len() > 16 * 1024 {
            return Err(invalid("native shell event exceeds 16 KiB"));
        }
        let event = serde_json::from_str(event).map_err(|error| invalid(error.to_string()))?;
        self.publish(event)
    }

    pub fn publish(&self, event: NativeShellEvent) -> Result<(), StorybookAutomationError> {
        let mut state = self.state.lock().expect("native shell state");
        match event {
            NativeShellEvent::Active { active } => {
                if state.snapshot.active != active {
                    state.snapshot.active = active;
                    state.snapshot.revision = state.snapshot.revision.saturating_add(1);
                }
                if !active && let Some(selection) = state.pending.take() {
                    state.snapshot.ack = selection.request_id();
                    state.snapshot.applied = false;
                }
            },
            NativeShellEvent::SurfaceReleased {} => {
                self.gate.suspend();
                state.snapshot.surface = state.snapshot.surface.saturating_add(1);
                state.snapshot.geometry = None;
                state.snapshot.ack = 0;
                state.pending = None;
            },
            NativeShellEvent::Settled {
                request_id,
                applied,
            } => {
                if state
                    .pending
                    .as_ref()
                    .is_some_and(|selection| selection.request_id() == request_id)
                {
                    state.pending = None;
                    state.snapshot.ack = request_id;
                    state.snapshot.applied = applied;
                }
            },
            NativeShellEvent::Frame {
                observation,
                request_id,
            } => {
                if observation.route.trim().is_empty()
                    || observation.route.len() > 256
                    || observation
                        .geometry
                        .as_ref()
                        .is_some_and(|geometry| !geometry.validate())
                {
                    return Err(invalid("native frame has an invalid route or geometry"));
                }
                let appearance = state
                    .snapshot
                    .appearances
                    .iter()
                    .find(|choice| choice.id() == observation.appearance)
                    .cloned()
                    .ok_or_else(|| invalid("native frame has an unadvertised appearance"))?;
                let display_changed = match (&state.snapshot.geometry, &observation.geometry) {
                    (Some(previous), Some(current)) => {
                        previous.display_width() != current.display_width()
                            || previous.display_height() != current.display_height()
                    },
                    _ => false,
                };
                if display_changed {
                    self.gate.suspend();
                    state.snapshot.surface = state.snapshot.surface.saturating_add(1);
                    state.snapshot.ack = 0;
                    state.pending = None;
                }
                let snapshot = &mut state.snapshot;
                if snapshot.route != observation.route
                    || snapshot.appearance != appearance
                    || snapshot.geometry != observation.geometry
                    || snapshot.actions != observation.actions
                    || snapshot.values != observation.values
                {
                    snapshot.route = observation.route;
                    snapshot.appearance = appearance;
                    snapshot.geometry = observation.geometry;
                    snapshot.actions = observation.actions;
                    snapshot.values = observation.values;
                    snapshot.revision = snapshot.revision.saturating_add(1);
                }
                // Geometry replacement revoked the selection; it cannot acknowledge
                // work against the new surface even if the native callback arrived late.
                if !display_changed
                    && request_id != 0
                    && state.pending.as_ref().is_some_and(|selection| {
                        selection.request_id() == request_id
                            && selection.surface() == state.snapshot.surface
                            && selection.permit().is_current()
                    })
                {
                    state.snapshot.ack = request_id;
                    state.snapshot.applied = true;
                }
            },
        }
        Ok(())
    }

    /// Retain the permit before enqueueing exactly once. Only a known rejection
    /// settles locally; an unknown submission remains owned until acknowledgment.
    pub fn submit(
        &self,
        selection: NativeSelection,
        dispatch: impl FnOnce(NativeDispatch) -> Result<(), NativeSubmissionError>,
    ) -> Result<(), NativeSubmissionError> {
        let request = selection.request_id();
        let payload = NativeDispatch {
            route: selection.route().to_owned(),
            appearance: selection.appearance().id().to_owned(),
            request_id: request,
            action: selection.action().cloned(),
        };
        {
            let mut state = self.state.lock().expect("native shell state");
            if state.pending.is_some()
                || !state.snapshot.active
                || state.snapshot.geometry.is_none()
                || selection.surface() != state.snapshot.surface
                || !selection.permit().is_current()
            {
                return Err(NativeSubmissionError::Rejected(
                    "native owner or selection is not current".to_owned(),
                ));
            }
            state.pending = Some(selection);
        }
        let result = dispatch(payload);
        if matches!(result, Err(NativeSubmissionError::Rejected(_))) {
            let _ = self.publish(NativeShellEvent::Settled {
                request_id: request,
                applied: false,
            });
        }
        result
    }
}

fn invalid(message: impl Into<String>) -> StorybookAutomationError {
    StorybookAutomationError::ControlOperationFailed {
        message: message.into(),
    }
}

// JNI/Kotlin carries a signed Long; preserve every unsigned wire ID's bits
// without routing it through a floating-point JSON number.
mod native_request_id {
    use serde::{Deserialize as _, Deserializer, Serializer};
    pub fn serialize<S: Serializer>(value: &u64, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i64(*value as i64)
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<u64, D::Error> {
        i64::deserialize(deserializer).map(|value| value as u64)
    }
}

#[cfg(test)]
mod tests;
