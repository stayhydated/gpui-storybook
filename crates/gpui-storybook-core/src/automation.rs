//! Gallery and dock implementation of the portable [`AutomationBackend`].
//!
//! Shared records and validation are owned by `gpui-storybook-automation`.
//! This module owns live story construction, GPUI dispatch, and desktop capture.
//!
//! [`StorybookAutomation`] serializes navigation, control mutation, capture,
//! and [`StoryInteractionRequest`] batches through one exclusive operation
//! guard. Story and current-route reads, control reads, and action discovery do
//! not acquire the guard; callers may use them while a mutation is active, but
//! they can observe an intermediate rendered state.
//!
//! Interaction requests are completely validated and their keystrokes and
//! registered actions are constructed before input dispatch. The shared
//! frame-aware executor resolves fresh story or substory capture bounds after
//! route preparation and story-region sizing, constrains pointer input to those bounds,
//! honors explicit rendered-frame waits, and performs an optional capture in
//! the same operation. Runtime failures after dispatch report partial progress
//! and must not be retried automatically.
//!
//! This controller uses the application's normal platform window. On Linux,
//! the MCP integration provides a Wayland compositor with Sway's wlroots
//! headless backend. macOS capture uses GPUI's native image renderer directly.

#[cfg(feature = "capture")]
use crate::capture_output::CaptureOutputStore;
pub(crate) mod interaction;

use crate::{
    capture_region::{
        capture_region_bounds, capture_route_story_key, scroll_capture_region_into_view,
    },
    story::StoryContainer,
};
use gpui_kit::{App, Entity, Global, Window, px};
#[cfg(feature = "capture")]
use gpui_kit::{Bounds, Pixels, point};

use std::{
    borrow::Borrow,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
};
use tokio::sync::{mpsc, oneshot, watch};

/// Shared automation handle used by live storybook views and MCP integrations.
pub type SharedStorybookAutomation = Arc<StorybookAutomation>;

/// Shared story navigation controller.
pub type SharedStoryController = SharedStorybookAutomation;

/// Shared story screenshot controller.
pub type SharedStoryCaptureController = SharedStorybookAutomation;

/// App-wide automation controller used by base storybook constructors.
///
/// When this global is installed, [`Gallery`](crate::gallery::Gallery) and
/// the dock workspace attach it from their base `view(...)` constructors.
#[derive(Clone)]
pub struct DefaultStorybookAutomation {
    automation: SharedStorybookAutomation,
}

impl Global for DefaultStorybookAutomation {}

impl DefaultStorybookAutomation {
    pub fn new(automation: SharedStorybookAutomation) -> Self {
        Self { automation }
    }

    pub fn automation(&self) -> SharedStorybookAutomation {
        self.automation.clone()
    }
}

pub fn set_default_storybook_automation(
    cx: &mut App,
    automation: SharedStorybookAutomation,
) -> SharedStorybookAutomation {
    cx.set_global(DefaultStorybookAutomation::new(automation.clone()));
    automation
}

pub fn default_storybook_automation(cx: &App) -> Option<SharedStorybookAutomation> {
    cx.try_global::<DefaultStorybookAutomation>()
        .map(DefaultStorybookAutomation::automation)
}

pub use gpui_storybook_automation::*;

mod backend;
pub(crate) mod capture;
mod controller;
mod host;

#[cfg(test)]
pub(crate) use capture::capture_exit_code;
pub use capture::{default_capture_output_path, story_snapshots_from_containers};
pub(crate) use capture::{schedule_story_capture, set_capture_target_size};
pub(crate) use controller::{
    AutomationOperationGuard, StorybookAutomationCommand, StorybookAutomationCommandReceiver,
};
pub use controller::{StorybookAutomation, story_snapshot_from_container};
pub(crate) use host::schedule_semantic_value_read;
use host::{receive_host_response, resolve_story_route};

#[cfg(test)]
mod tests;
