//! Portable headless story execution, visual capture, and matrix testing.
//!
//! The runner deliberately owns one [`gpui::HeadlessAppContext`] per case. A
//! context contains the app, renderer, window, entities, and GPUI globals, so
//! constructing it inside each operation keeps story state isolated while
//! still allowing the real story registration functions to run.

mod baseline;
mod matrix;
mod performance;

pub use baseline::{
    BaselineError, BaselinePolicy, BaselineReport, BaselineStatus, BaselineStore, BaselineTolerance,
};
pub use matrix::{
    CaptureCase, CaptureMatrix, CaptureRequest, CaseFailure, CaseReport, CaseStatus, ControlCase,
    LanguageCase, MatrixReport, PresentationCase, RouteCase, ThemeCase, ViewportCase,
};
pub use performance::{
    PerformanceBudget, PerformanceBudgetFailure, PerformanceMetric, PerformanceMetricSummary,
    PerformanceReport, PerformanceStatistic, PerformanceViolation,
};

use gpui::{
    AnyWindowHandle, App, AssetSource, Entity, HeadlessAppContext, PlatformTextSystem, Size,
    Window, WindowHandle, px, size,
};
use gpui_component::{Theme, ThemeMode};
#[cfg(feature = "capture")]
use gpui_storybook_core::capture_region::{CaptureRegionImageError, crop_capture_region_image};
use gpui_storybook_core::capture_region::{
    reset_capture_regions_for_story, scroll_capture_region_into_view,
};
use gpui_storybook_core::{
    controls::{ControlError, ControlSnapshot, ControlTarget, ControlValue},
    presentation::{StoryPresentation, StoryViewportPreset},
    registry::{InitEntry, StoryEntry},
    story::{StoryContainer, init as init_story_runtime},
};
use image::RgbaImage;
use serde::{Deserialize, Serialize};
use std::{
    cell::RefCell,
    collections::BTreeMap,
    fmt,
    path::{Path, PathBuf},
    rc::Rc,
    sync::Arc,
};
use thiserror::Error;

/// A callback that configures a fresh case before its first draw.
///
/// The callback is called after the core Storybook runtime and all registered
/// [`#[gpui_storybook::story_init]`](https://docs.rs/gpui-storybook) hooks have
/// run, and after the runner has applied the requested viewport and canvas
/// background. It is the seam for application-owned theme and language
/// adapters. The callback receives the live story entity so an integration can
/// call `set_presentation` for a custom presentation or set any other
/// per-case state through GPUI globals.
///
/// A callback is invoked before `HeadlessAppContext::open_window` performs the
/// initial draw. Returning an error makes the case fail; the runner never
/// emits a capture carrying only an unapplied theme or language label.
pub type CaseConfigurator = Rc<
    dyn Fn(&CaptureCase, &Entity<StoryContainer>, &mut Window, &mut App) -> Result<(), String>
        + 'static,
>;

/// A callback that turns a full-window image into a requested story route.
///
/// Root routes bypass this callback. For a [`RouteCase::Substory`], the
/// callback must verify that `route_id` was rendered and return the cropped
/// route image. The core capture-region registry is intentionally kept behind
/// this callback so applications can choose their own crop policy while the
/// matrix runner still treats substory routing as executable work.
pub type RouteCapture =
    Rc<dyn Fn(&str, &RgbaImage, Size<gpui::Pixels>) -> Result<RgbaImage, String> + 'static>;

/// A callback for application-owned global initialization.
pub type AppInitializer = Rc<dyn Fn(&mut App) + 'static>;

/// Opt-in profiler collection for one capture.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PerformanceOptions {
    /// Number of redraws requested before the image and profiler snapshot are
    /// read. The initial `open_window` draw is not counted in this value.
    #[serde(default = "default_measured_frames")]
    pub measured_frames: u32,
    /// Optional assertions applied to the collected GPUI histograms.
    pub budget: Option<PerformanceBudget>,
}

const fn default_measured_frames() -> u32 {
    3
}

impl Default for PerformanceOptions {
    fn default() -> Self {
        Self {
            measured_frames: default_measured_frames(),
            budget: None,
        }
    }
}

impl PerformanceOptions {
    /// Creates profiler options with three measured redraws.
    pub const fn new() -> Self {
        Self {
            measured_frames: default_measured_frames(),
            budget: None,
        }
    }

    /// Sets the number of redraws requested for the profiler sample.
    pub const fn measured_frames(mut self, measured_frames: u32) -> Self {
        self.measured_frames = measured_frames;
        self
    }

    /// Adds a typed draw and dirty-to-present budget.
    pub const fn budget(mut self, budget: PerformanceBudget) -> Self {
        self.budget = Some(budget);
        self
    }
}

/// Configuration shared by fresh portable-story contexts.
#[derive(Clone)]
pub struct RunnerConfig {
    /// Fallback family passed to `gpui_wgpu::CosmicTextSystem`.
    pub font_fallback: String,
    /// Assets available to stories while they render in the headless context.
    ///
    /// The default empty source is enough for stories that draw only GPUI
    /// primitives. Applications with embedded fonts, icons, or image assets
    /// should provide their normal `AssetSource` here.
    pub asset_source: Arc<dyn AssetSource>,
    /// Number of redraws requested when a request does not provide a settle
    /// override.
    pub settle_frames: u32,
    /// Applies theme, language, or application-specific presentation state.
    pub case_configurator: Option<CaseConfigurator>,
    /// Crops and verifies a rendered substory route.
    pub route_capture: Option<RouteCapture>,
    /// Installs consumer-owned globals before the first story is constructed.
    pub app_initializer: Option<AppInitializer>,
}

impl fmt::Debug for RunnerConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RunnerConfig")
            .field("font_fallback", &self.font_fallback)
            .field("asset_source", &"configured")
            .field("settle_frames", &self.settle_frames)
            .field("case_configurator", &self.case_configurator.is_some())
            .field("route_capture", &self.route_capture.is_some())
            .field("app_initializer", &self.app_initializer.is_some())
            .finish()
    }
}

impl Default for RunnerConfig {
    fn default() -> Self {
        Self {
            font_fallback: "IBM Plex Sans".to_owned(),
            asset_source: Arc::new(()),
            settle_frames: 1,
            case_configurator: None,
            route_capture: None,
            app_initializer: None,
        }
    }
}

impl RunnerConfig {
    /// Sets the font fallback used by the real text shaper.
    pub fn font_fallback(mut self, font_fallback: impl Into<String>) -> Self {
        self.font_fallback = font_fallback.into();
        self
    }

    /// Sets the asset source used by every fresh context.
    pub fn asset_source(mut self, asset_source: Arc<dyn AssetSource>) -> Self {
        self.asset_source = asset_source;
        self
    }

    /// Sets the default redraw count used to settle a capture.
    pub const fn settle_frames(mut self, settle_frames: u32) -> Self {
        self.settle_frames = settle_frames;
        self
    }

    /// Installs the pre-draw theme/language/presentation configurator.
    pub fn case_configurator(mut self, configurator: CaseConfigurator) -> Self {
        self.case_configurator = Some(configurator);
        self
    }

    /// Installs the substory image crop and route-verification callback.
    pub fn route_capture(mut self, route_capture: RouteCapture) -> Self {
        self.route_capture = Some(route_capture);
        self
    }

    /// Installs an application-specific initialization hook.
    pub fn app_initializer(mut self, initializer: AppInitializer) -> Self {
        self.app_initializer = Some(initializer);
        self
    }
}

mod discovery;
mod error;
mod portable;
mod report;
mod runner;
mod support;

pub use discovery::{
    PortableStoryMetadata, StoryDescriptor, discover_stories, discover_stories_checked,
};
pub use error::StorybookTestError;
pub use portable::PortableStory;
pub use report::CaptureReport;
pub use runner::HeadlessStoryRunner;
use support::*;

#[cfg(test)]
mod tests;
