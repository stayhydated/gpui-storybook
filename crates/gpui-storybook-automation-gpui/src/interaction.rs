//! Frame-aware input execution on the application's owning GPUI thread.
use crate::regions::{capture_region_bounds, scroll_capture_region_into_view};
use crate::snapshot::{rendered_interaction_targets, rendered_semantic_values};
use gpui::{
    Action, App, Keystroke, Modifiers, MouseDownEvent, MouseMoveEvent, MouseUpEvent, PlatformInput,
    ScrollDelta, ScrollWheelEvent, TouchPhase, Window, point, px,
};
#[cfg(test)]
use serde::Deserialize;
use serde_json::Value;
use std::{
    collections::VecDeque,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};
use tokio::sync::oneshot;

pub use gpui_storybook_automation::*;
use std::rc::Rc;

/// Platform provider called after rendered readiness and under the host lease.
/// Device providers must use observed geometry; desktop sizing stays in core.
pub trait InteractionCaptureProvider {
    /// Validate the still-attached host before each deferred execution boundary.
    fn validate_host(&self, _: &str, _: &Window, _: &App) -> Result<(), StorybookAutomationError> {
        Ok(())
    }
    fn ensure_visible(
        &self,
        route: &str,
        window: &mut Window,
        cx: &App,
    ) -> Result<bool, StorybookAutomationError>;
    fn capture(
        &self,
        request_id: u64,
        request: StoryScreenshotRequest,
        story: StorySnapshot,
        window: &mut Window,
        cx: &App,
    ) -> Result<StoryCaptureSnapshot, StorybookAutomationError>;
}

/// Region readiness for hosts that advertise interaction without image capture.
pub struct NoCaptureProvider;
impl InteractionCaptureProvider for NoCaptureProvider {
    fn ensure_visible(
        &self,
        route: &str,
        window: &mut Window,
        cx: &App,
    ) -> Result<bool, StorybookAutomationError> {
        capture_region_bounds(route, window, cx)
            .map(|_| false)
            .ok_or_else(|| StorybookAutomationError::CaptureUnavailable {
                message: format!("capture route `{route}` was not rendered"),
            })
    }
    fn capture(
        &self,
        _: u64,
        _: StoryScreenshotRequest,
        _: StorySnapshot,
        _: &mut Window,
        _: &App,
    ) -> Result<StoryCaptureSnapshot, StorybookAutomationError> {
        Err(StorybookAutomationError::UnsupportedCapability {
            capability: AutomationCapability::StoryCapture,
        })
    }
}

mod request;
mod runner;

pub use request::{PreparedInteractionStep, list_registered_actions, prepare_interaction_steps};
pub use runner::{
    PreparedStoryInteraction, interaction_target_size, schedule_interaction_target_listing,
    schedule_story_interaction,
};

#[cfg(test)]
mod tests;
