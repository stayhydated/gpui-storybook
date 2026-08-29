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

/// Static metadata and executable constructor for one inventory story.
#[derive(Clone)]
pub struct StoryDescriptor {
    entry: Option<&'static StoryEntry>,
    metadata: PortableStoryMetadata,
}

impl fmt::Debug for StoryDescriptor {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("StoryDescriptor")
            .field("metadata", &self.metadata)
            .field("has_constructor", &self.entry.is_some())
            .finish()
    }
}

/// Inventory metadata copied into a serializable discovery report.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PortableStoryMetadata {
    /// Globally stable story key.
    pub key: String,
    /// Registered Rust story name.
    pub name: String,
    /// Declared Storybook section.
    pub section: Option<String>,
    /// Source package.
    pub crate_name: String,
    /// Source manifest directory.
    pub crate_dir: String,
    /// Source file.
    pub source_file: String,
    /// Source line.
    pub source_line: u32,
    /// Rustdoc captured at registration time.
    pub docs: String,
}

impl StoryDescriptor {
    fn from_entry(entry: &'static StoryEntry) -> Self {
        let metadata = entry.metadata();
        Self {
            entry: Some(entry),
            metadata: PortableStoryMetadata {
                key: metadata.key().as_str().to_owned(),
                name: metadata.name().as_str().to_owned(),
                section: metadata
                    .section()
                    .map(|section| section.as_str().to_owned()),
                crate_name: metadata.crate_name().to_owned(),
                crate_dir: metadata.crate_dir().to_owned(),
                source_file: metadata.source_file().to_owned(),
                source_line: metadata.source_line(),
                docs: entry.autodoc().docs().to_owned(),
            },
        }
    }

    /// Returns the serializable discovery metadata.
    pub fn metadata(&self) -> &PortableStoryMetadata {
        &self.metadata
    }

    /// Returns the stable story key.
    pub fn key(&self) -> &str {
        &self.metadata.key
    }

    /// Returns the registered Rust story name.
    pub fn name(&self) -> &str {
        &self.metadata.name
    }

    /// Returns the source package.
    pub fn crate_name(&self) -> &str {
        &self.metadata.crate_name
    }

    /// Returns the executable inventory entry, when this descriptor came from
    /// [`discover_stories`].
    pub fn entry(&self) -> Option<&'static StoryEntry> {
        self.entry
    }

    #[cfg(test)]
    pub(crate) fn for_test(key: &str, name: &str) -> Self {
        Self {
            entry: None,
            metadata: PortableStoryMetadata {
                key: key.to_owned(),
                name: name.to_owned(),
                section: None,
                crate_name: "test".to_owned(),
                crate_dir: "/tmp/test".to_owned(),
                source_file: "test.rs".to_owned(),
                source_line: 1,
                docs: String::new(),
            },
        }
    }
}

/// Discovers inventory stories sorted by stable key and source location.
pub fn discover_stories() -> Vec<StoryDescriptor> {
    let mut stories = inventory::iter::<StoryEntry>()
        .map(StoryDescriptor::from_entry)
        .collect::<Vec<_>>();
    stories.sort_by(|left, right| {
        left.metadata
            .key
            .cmp(&right.metadata.key)
            .then_with(|| left.metadata.source_file.cmp(&right.metadata.source_file))
            .then_with(|| left.metadata.source_line.cmp(&right.metadata.source_line))
    });
    stories
}

/// Discovers stories and rejects duplicate global keys before execution.
pub fn discover_stories_checked() -> Result<Vec<StoryDescriptor>, StorybookTestError> {
    let stories = discover_stories();
    for duplicate in stories.windows(2) {
        if duplicate[0].key() == duplicate[1].key() {
            return Err(StorybookTestError::DuplicateStoryKey {
                key: duplicate[0].key().to_owned(),
                first: duplicate_location(&duplicate[0]),
                second: duplicate_location(&duplicate[1]),
            });
        }
    }
    Ok(stories)
}

fn duplicate_location(story: &StoryDescriptor) -> String {
    format!(
        "{}:{} ({})",
        story.metadata.source_file, story.metadata.source_line, story.metadata.crate_name
    )
}

/// A fresh executable story and its isolated headless app context.
pub struct PortableStory {
    context: HeadlessAppContext,
    window: WindowHandle<StoryContainer>,
    story: Entity<StoryContainer>,
    descriptor: StoryDescriptor,
    case: CaptureCase,
    route_capture: Option<RouteCapture>,
}

impl fmt::Debug for PortableStory {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PortableStory")
            .field("story_key", &self.case.story_key)
            .field("case_id", &self.case.id)
            .field("window", &self.window)
            .finish()
    }
}

impl PortableStory {
    /// Returns the descriptor used to create this story.
    pub fn descriptor(&self) -> &StoryDescriptor {
        &self.descriptor
    }

    /// Returns the fully expanded case.
    pub fn case(&self) -> &CaptureCase {
        &self.case
    }

    /// Returns the live story entity.
    pub fn story(&self) -> Entity<StoryContainer> {
        self.story.clone()
    }

    /// Returns the headless window handle.
    pub fn window(&self) -> WindowHandle<StoryContainer> {
        self.window
    }

    /// Runs all pending GPUI work until the context parks.
    pub fn run_until_parked(&self) {
        self.context.run_until_parked();
    }

    /// Advances the deterministic headless clock.
    pub fn advance_clock(&self, duration: std::time::Duration) {
        self.context.advance_clock(duration);
    }

    /// Executes a caller-owned app update in the isolated context.
    pub fn update<R>(&mut self, update: impl FnOnce(&mut App) -> R) -> R {
        self.context.update(update)
    }

    /// Applies one typed control value to the fresh story.
    pub fn set_control(
        &mut self,
        key: impl Into<String>,
        value: ControlValue,
    ) -> Result<(), StorybookTestError> {
        let key = key.into();
        let target = self.control_target()?;
        self.context
            .update(|app| target.set(&key, value, app))
            .map_err(StorybookTestError::from)
    }

    /// Applies a deterministic set of typed controls in key order.
    pub fn apply_controls(
        &mut self,
        controls: &BTreeMap<String, ControlValue>,
    ) -> Result<(), StorybookTestError> {
        for (key, value) in controls {
            self.set_control(key.clone(), value.clone())?;
        }
        Ok(())
    }

    /// Reads current control metadata and values from the live story.
    ///
    /// Stories without a typed control target return an empty snapshot. Applying
    /// a non-empty control map to such a story still returns
    /// [`StorybookTestError::ControlsUnavailable`].
    pub fn control_snapshots(&mut self) -> Result<Vec<ControlSnapshot>, StorybookTestError> {
        self.context.update(|app| {
            let target = self.story.read(app).control_target();
            read_control_snapshots(target, app)
        })
    }

    /// Reads the live story metadata after the first frame has rendered.
    pub fn story_snapshot(
        &mut self,
    ) -> Result<gpui_storybook_core::automation::StorySnapshot, StorybookTestError> {
        self.context
            .update(|app| {
                let story = self.story.read(app);
                let app_ref: &App = app;
                gpui_storybook_core::automation::StorySnapshot::from_container(story, &app_ref)
            })
            .ok_or_else(|| StorybookTestError::StoryMetadataUnavailable {
                key: self.case.story_key.clone(),
            })
    }

    /// Requests and processes `frames` redraws before a capture.
    pub fn settle(&mut self, frames: u32) -> Result<(), StorybookTestError> {
        let frames = frames.max(1);
        let window: AnyWindowHandle = self.window.into();
        if uses_core_route_registry(&self.case.route, self.route_capture.is_some()) {
            let route_id = self.case.route_id.clone();
            let rendered = self
                .context
                .update_window(window, |_, _, _| scroll_capture_region_into_view(&route_id))
                .map_err(headless_error)?;
            if !rendered {
                #[cfg(feature = "capture")]
                return Err(StorybookTestError::CaptureRegion(
                    CaptureRegionImageError::RouteNotRendered { route_id },
                ));
                #[cfg(not(feature = "capture"))]
                return Err(StorybookTestError::RouteCapture {
                    route_id,
                    message: "route was not rendered before settling".to_owned(),
                });
            }
        }
        for _ in 0..frames {
            self.context
                .update_window(window, |_, window, _| window.refresh())
                .map_err(headless_error)?;
            self.context.run_until_parked();
        }
        Ok(())
    }

    /// Captures the full window or invokes the configured substory route
    /// callback for a cropped image.
    pub fn capture_image(&mut self) -> Result<RgbaImage, StorybookTestError> {
        let window: AnyWindowHandle = self.window.into();
        let image = self
            .context
            .capture_screenshot(window)
            .map_err(headless_error)?;
        let window_size = self
            .context
            .update_window(window, |_, window, _| window.bounds().size)
            .map_err(headless_error)?;
        if matches!(&self.case.route, RouteCase::Root) {
            #[cfg(feature = "capture")]
            return crop_capture_region_image(&self.case.route_id, image, window_size)
                .map_err(StorybookTestError::CaptureRegion);
            #[cfg(not(feature = "capture"))]
            return Ok(image);
        }

        if let Some(route_capture) = self.configured_route_capture() {
            return route_capture(&self.case.route_id, &image, window_size).map_err(|message| {
                StorybookTestError::RouteCapture {
                    route_id: self.case.route_id.clone(),
                    message,
                }
            });
        }

        #[cfg(feature = "capture")]
        return crop_capture_region_image(&self.case.route_id, image, window_size)
            .map_err(StorybookTestError::CaptureRegion);
        #[cfg(not(feature = "capture"))]
        Err(StorybookTestError::RouteCaptureRequired {
            route_id: self.case.route_id.clone(),
        })
    }

    /// Captures and writes a PNG at `path`, returning the image as well.
    pub fn capture_png(&mut self, path: impl AsRef<Path>) -> Result<RgbaImage, StorybookTestError> {
        let image = self.capture_image()?;
        write_png(path.as_ref(), &image)?;
        Ok(image)
    }

    /// Returns the current GPUI profiler report when the `performance` feature
    /// was enabled for this crate.
    #[cfg(feature = "performance")]
    pub fn performance_report(&mut self) -> Result<PerformanceReport, StorybookTestError> {
        let window: AnyWindowHandle = self.window.into();
        self.context
            .update_window(window, |_, window, _| {
                PerformanceReport::from_window(window)
            })
            .map_err(headless_error)
    }

    fn control_target(&mut self) -> Result<Rc<dyn ControlTarget>, StorybookTestError> {
        self.context
            .update(|app| self.story.read(app).control_target())
            .ok_or_else(|| StorybookTestError::ControlsUnavailable {
                key: self.case.story_key.clone(),
            })
    }

    fn configured_route_capture(&self) -> Option<RouteCapture> {
        self.route_capture.clone()
    }
}

/// Headless story runner with fresh-context isolation for every case.
#[derive(Clone, Debug, Default)]
pub struct HeadlessStoryRunner {
    config: RunnerConfig,
}

impl HeadlessStoryRunner {
    /// Creates a runner with the default text fallback and settle count.
    pub fn new(config: RunnerConfig) -> Self {
        Self { config }
    }

    /// Returns the runner configuration.
    pub fn config(&self) -> &RunnerConfig {
        &self.config
    }

    /// Discovers and validates all registered stories.
    pub fn discover(&self) -> Result<Vec<StoryDescriptor>, StorybookTestError> {
        discover_stories_checked()
    }

    /// Opens a fresh story context without capturing it.
    pub fn open(&self, request: CaptureRequest) -> Result<PortableStory, StorybookTestError> {
        let case = self.request_case(request)?;
        self.open_case(case)
    }

    /// Captures one request without touching visual baselines.
    pub fn capture(&self, request: CaptureRequest) -> Result<CaptureReport, StorybookTestError> {
        self.capture_with_baseline(request, None, BaselinePolicy::Ignore)
    }

    /// Captures one request and applies an explicit visual baseline policy.
    pub fn capture_with_baseline(
        &self,
        request: CaptureRequest,
        baseline_store: Option<&BaselineStore>,
        baseline_policy: BaselinePolicy,
    ) -> Result<CaptureReport, StorybookTestError> {
        let case = self.request_case(request)?;
        self.capture_case(case, baseline_store, baseline_policy)
    }

    /// Expands and executes a matrix, preserving each case's failure as typed
    /// report data so one bad story does not hide adjacent cases.
    pub fn run_matrix(
        &self,
        matrix: &CaptureMatrix,
        baseline_store: Option<&BaselineStore>,
        baseline_policy: BaselinePolicy,
    ) -> Result<MatrixReport, StorybookTestError> {
        let discovered = self.discover()?;
        let cases = matrix.expand(&discovered)?;
        let mut reports = Vec::with_capacity(cases.len());
        for case in cases {
            let request = case.request();
            match self.capture_case(case.clone(), baseline_store, baseline_policy.clone()) {
                Ok(capture) => reports.push(CaseReport::passed(case.id, request, capture)),
                Err(error) => reports.push(CaseReport::failed_with_error(case.id, request, &error)),
            }
        }
        let passed = reports.iter().all(|report| {
            matches!(
                report.status,
                CaseStatus::Passed | CaseStatus::BaselineUpdated
            )
        });
        Ok(MatrixReport {
            cases: reports,
            passed,
        })
    }

    fn request_case(&self, request: CaptureRequest) -> Result<CaptureCase, StorybookTestError> {
        let route_id = request.validate()?;
        let id = request.id();
        let controls = ControlCase::new("request", request.controls.clone());
        controls.validate()?;
        Ok(CaptureCase {
            id,
            story_key: request.story_key,
            route_id,
            route: request.route,
            viewport: request.viewport,
            presentation: request.presentation,
            theme: request.theme,
            language: request.language,
            controls,
            output_path: request.output_path,
            settle_frames: request.settle_frames,
            performance: request.performance,
        })
    }

    fn open_case(&self, case: CaptureCase) -> Result<PortableStory, StorybookTestError> {
        let descriptor = self
            .discover()?
            .into_iter()
            .find(|story| story.key() == case.story_key)
            .ok_or_else(|| StorybookTestError::StoryNotFound {
                key: case.story_key.clone(),
            })?;
        let entry = descriptor
            .entry()
            .ok_or_else(|| StorybookTestError::StoryNotExecutable {
                key: case.story_key.clone(),
            })?;
        validate_case_configuration(&case, &self.config)?;

        #[cfg(not(feature = "performance"))]
        if case.performance.is_some() {
            return Err(StorybookTestError::PerformanceUnavailable);
        }

        reset_capture_regions_for_story(&case.story_key);

        let text_system: Arc<dyn PlatformTextSystem> =
            Arc::new(gpui_wgpu::CosmicTextSystem::new(&self.config.font_fallback));
        let mut context = HeadlessAppContext::with_platform(
            text_system,
            self.config.asset_source.clone(),
            gpui_platform::current_headless_renderer,
        );

        context.update(|app| {
            initialize_portable_story_app(app)?;
            if let Some(initializer) = &self.config.app_initializer {
                initializer(app);
            }
            Ok::<_, StorybookTestError>(())
        })?;

        let setup_error = Rc::new(RefCell::new(None));
        let setup_error_for_window = setup_error.clone();
        let configurator = self.config.case_configurator.clone();
        let route_capture = self.config.route_capture.clone();
        let section = descriptor.metadata.section.clone();
        let controls = case.controls.values.clone();
        let case_for_window = case.clone();
        let viewport = size(
            px(case.viewport.width as f32),
            px(case.viewport.height as f32),
        );
        let window = context
            .open_window(viewport, move |window, app| {
                let story = (entry.create_fn)(window, app);
                story.update(app, |story, cx| {
                    story.section = section.clone().map(Into::into);
                    story.set_registration_metadata(entry.metadata());
                    story.set_presentation(StoryPresentation {
                        viewport: viewport_preset(&case_for_window.viewport),
                        background: case_for_window.presentation.background,
                    });
                    cx.notify();
                });

                apply_builtin_theme(&case_for_window.theme, window, app);
                if let Err(error) = apply_controls_to_story(&story, &controls, app) {
                    *setup_error_for_window.borrow_mut() = Some(error);
                }

                if setup_error_for_window.borrow().is_none()
                    && let Some(configurator) = configurator
                    && let Err(message) = configurator(&case_for_window, &story, window, app)
                {
                    *setup_error_for_window.borrow_mut() =
                        Some(StorybookTestError::CaseConfiguration {
                            axis: configuration_axis(&case_for_window),
                            message,
                        });
                }
                story
            })
            .map_err(headless_error)?;
        if let Some(error) = setup_error.borrow_mut().take() {
            return Err(error);
        }
        let story = window.entity(&context).map_err(headless_error)?;
        Ok(PortableStory {
            context,
            window,
            story,
            descriptor,
            case,
            route_capture,
        })
    }

    fn capture_case(
        &self,
        case: CaptureCase,
        baseline_store: Option<&BaselineStore>,
        baseline_policy: BaselinePolicy,
    ) -> Result<CaptureReport, StorybookTestError> {
        let performance_options = case.performance.clone();
        let settle_frames = effective_settle_frames(
            case.settle_frames,
            self.config.settle_frames,
            performance_options.as_ref(),
        );
        let mut story = self.open_case(case.clone())?;
        story.settle(settle_frames)?;
        let image = story.capture_image()?;
        let output_path = case.output_path.clone();
        if let Some(path) = &output_path {
            write_png(path, &image)?;
        }
        let controls = story.control_snapshots()?;
        let story_snapshot = story.story_snapshot()?;
        #[cfg(feature = "performance")]
        let performance = performance_options
            .as_ref()
            .map(|_| story.performance_report())
            .transpose()?;
        #[cfg(not(feature = "performance"))]
        let performance: Option<PerformanceReport> = None;

        let baseline = match baseline_policy {
            BaselinePolicy::Ignore => None,
            BaselinePolicy::Check { tolerance } => {
                let store = baseline_store.ok_or(StorybookTestError::BaselineStoreRequired)?;
                Some(store.check(&case.id, &image, tolerance)?)
            },
            BaselinePolicy::Update => {
                let store = baseline_store.ok_or(StorybookTestError::BaselineStoreRequired)?;
                Some(store.update(&case.id, &image)?)
            },
        };

        let report = CaptureReport {
            id: case.id,
            story: story_snapshot,
            route_id: case.route_id,
            viewport: case.viewport,
            presentation: case.presentation,
            theme: case.theme,
            language: case.language,
            controls,
            output_path,
            width: image.width(),
            height: image.height(),
            baseline,
            performance,
        };

        if let (Some(options), Some(performance)) =
            (performance_options, report.performance.as_ref())
            && let Some(budget) = options.budget
            && let Err(failure) = performance.check(&budget)
        {
            return Err(StorybookTestError::PerformanceBudgetExceeded {
                failure: Box::new(failure),
                capture: Box::new(report),
            });
        }
        Ok(report)
    }
}

/// Structured output from one rendered request.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CaptureReport {
    /// Stable case ID.
    pub id: String,
    /// Runtime story metadata after construction.
    pub story: gpui_storybook_core::automation::StorySnapshot,
    /// Fully qualified captured route.
    pub route_id: String,
    /// Requested viewport.
    pub viewport: ViewportCase,
    /// Requested canvas presentation.
    pub presentation: PresentationCase,
    /// Requested theme case.
    pub theme: ThemeCase,
    /// Requested language case.
    pub language: LanguageCase,
    /// Runtime controls after applying the request.
    pub controls: Vec<ControlSnapshot>,
    /// Optional written PNG path.
    pub output_path: Option<PathBuf>,
    /// Captured image width in physical pixels.
    pub width: u32,
    /// Captured image height in physical pixels.
    pub height: u32,
    /// Explicit baseline comparison or update result.
    pub baseline: Option<BaselineReport>,
    /// Optional GPUI profiler report.
    pub performance: Option<PerformanceReport>,
}

impl CaptureReport {
    /// Returns whether the requested baseline policy was accepted.
    pub fn visual_match(&self) -> bool {
        self.baseline.as_ref().is_none_or(BaselineReport::matches)
    }
}

/// Errors emitted by planning, configuring, rendering, and checking cases.
#[derive(Debug, Error)]
pub enum StorybookTestError {
    /// No registered story matched a requested key.
    #[error("story `{key}` was not found in inventory")]
    StoryNotFound { key: String },
    /// Multiple inventory entries share one global story key.
    #[error("duplicate story key `{key}` at {first} and {second}")]
    DuplicateStoryKey {
        key: String,
        first: String,
        second: String,
    },
    /// A descriptor exists only for planning and has no executable constructor.
    #[error("story `{key}` has no executable inventory constructor")]
    StoryNotExecutable { key: String },
    /// A story was constructed without metadata readable by the report layer.
    #[error("runtime metadata for story `{key}` was unavailable")]
    StoryMetadataUnavailable { key: String },
    /// A viewport has invalid dimensions or identity.
    #[error("invalid viewport `{name}`: {message}")]
    InvalidViewport { name: String, message: String },
    /// A matrix or request field is invalid.
    #[error("invalid capture case field `{field}`: {message}")]
    InvalidCase { field: String, message: String },
    /// A requested theme or language needs a consumer callback.
    #[error("capture case requires a case configurator for `{axis}`")]
    CaseConfigurationRequired { axis: String },
    /// A configured case callback rejected a case.
    #[error("case configurator failed for `{axis}`: {message}")]
    CaseConfiguration { axis: String, message: String },
    /// A substory route needs a crop-and-verify callback.
    #[error("capture route `{route_id}` requires a route capture callback")]
    RouteCaptureRequired { route_id: String },
    /// A route callback could not verify or crop a route.
    #[error("capture route `{route_id}` failed: {message}")]
    RouteCapture { route_id: String, message: String },
    /// The core capture-region registry could not resolve a rendered route.
    #[cfg(feature = "capture")]
    #[error("capture region operation failed: {0}")]
    CaptureRegion(#[from] CaptureRegionImageError),
    /// Core Storybook app initialization failed.
    #[error("Storybook runtime initialization failed: {message}")]
    RuntimeInitialization { message: String },
    /// A typed story control operation failed.
    #[error("story control operation failed: {0}")]
    Control(#[from] ControlError),
    /// No controls target was available on a story that was asked to expose controls.
    #[error("story `{key}` exposes no controls target")]
    ControlsUnavailable { key: String },
    /// A headless GPUI operation failed.
    #[error("headless GPUI operation failed: {message}")]
    Headless { message: String },
    /// A requested PNG could not be written.
    #[error("failed to write PNG {}: {message}", path.display())]
    Output { path: PathBuf, message: String },
    /// A baseline comparison or update failed.
    #[error("visual baseline operation failed: {0}")]
    Baseline(#[from] BaselineError),
    /// A check or update policy omitted its store.
    #[error("a baseline store is required for the selected baseline policy")]
    BaselineStoreRequired,
    /// Performance was requested without compiling this crate's feature.
    #[error("performance capture requires the `performance` crate feature")]
    PerformanceUnavailable,
    /// A typed performance budget failed; the capture report is retained.
    #[error("{failure}")]
    PerformanceBudgetExceeded {
        failure: Box<PerformanceBudgetFailure>,
        capture: Box<CaptureReport>,
    },
}

impl StorybookTestError {
    /// Returns a stable category suitable for matrix JSON reports.
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::StoryNotFound { .. } => "story_not_found",
            Self::DuplicateStoryKey { .. } => "duplicate_story_key",
            Self::StoryNotExecutable { .. } => "story_not_executable",
            Self::StoryMetadataUnavailable { .. } => "story_metadata_unavailable",
            Self::InvalidViewport { .. } => "invalid_viewport",
            Self::InvalidCase { .. } => "invalid_case",
            Self::CaseConfigurationRequired { .. } => "case_configuration_required",
            Self::CaseConfiguration { .. } => "case_configuration",
            Self::RouteCaptureRequired { .. } => "route_capture_required",
            Self::RouteCapture { .. } => "route_capture",
            #[cfg(feature = "capture")]
            Self::CaptureRegion(_) => "capture_region",
            Self::RuntimeInitialization { .. } => "runtime_initialization",
            Self::Control(_) => "control",
            Self::ControlsUnavailable { .. } => "controls_unavailable",
            Self::Headless { .. } => "headless",
            Self::Output { .. } => "output",
            Self::Baseline(_) => "baseline",
            Self::BaselineStoreRequired => "baseline_store_required",
            Self::PerformanceUnavailable => "performance_unavailable",
            Self::PerformanceBudgetExceeded { .. } => "performance_budget_exceeded",
        }
    }

    /// Returns a retained capture when a performance budget failed.
    pub fn capture_report(&self) -> Option<&CaptureReport> {
        match self {
            Self::PerformanceBudgetExceeded { capture, .. } => Some(capture),
            _ => None,
        }
    }

    /// Returns the typed performance failure when present.
    pub fn performance_failure(&self) -> Option<&PerformanceBudgetFailure> {
        match self {
            Self::PerformanceBudgetExceeded { failure, .. } => Some(failure),
            _ => None,
        }
    }
}

fn effective_settle_frames(
    requested: u32,
    configured: u32,
    performance: Option<&PerformanceOptions>,
) -> u32 {
    let settled = if requested == 0 {
        configured
    } else {
        requested
    };
    settled.max(performance.map_or(0, |options| options.measured_frames))
}

fn headless_error(error: anyhow::Error) -> StorybookTestError {
    StorybookTestError::Headless {
        message: error.to_string(),
    }
}

fn write_png(path: &Path, image: &RgbaImage) -> Result<(), StorybookTestError> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent).map_err(|error| StorybookTestError::Output {
            path: parent.to_path_buf(),
            message: error.to_string(),
        })?;
    }
    image
        .save_with_format(path, image::ImageFormat::Png)
        .map_err(|error| StorybookTestError::Output {
            path: path.to_path_buf(),
            message: error.to_string(),
        })
}

fn validate_case_configuration(
    case: &CaptureCase,
    config: &RunnerConfig,
) -> Result<(), StorybookTestError> {
    if case
        .theme
        .theme
        .as_deref()
        .is_some_and(|theme| builtin_theme_mode(theme).is_none())
        && config.case_configurator.is_none()
    {
        return Err(StorybookTestError::CaseConfigurationRequired {
            axis: "theme".to_owned(),
        });
    }
    if case.language.language.is_some() && config.case_configurator.is_none() {
        return Err(StorybookTestError::CaseConfigurationRequired {
            axis: "language".to_owned(),
        });
    }
    Ok(())
}

fn configuration_axis(case: &CaptureCase) -> String {
    if case.theme.theme.is_some() {
        "theme".to_owned()
    } else if case.language.language.is_some() {
        "language".to_owned()
    } else {
        "presentation".to_owned()
    }
}

fn builtin_theme_mode(theme: &str) -> Option<ThemeMode> {
    match theme.trim().to_ascii_lowercase().as_str() {
        "light" | "default light" => Some(ThemeMode::Light),
        "dark" | "default dark" => Some(ThemeMode::Dark),
        _ => None,
    }
}

fn apply_builtin_theme(theme: &ThemeCase, window: &mut Window, app: &mut App) {
    if let Some(theme) = theme.theme.as_deref()
        && let Some(mode) = builtin_theme_mode(theme)
    {
        Theme::change(mode, Some(window), app);
    }
}

fn initialize_portable_story_app(app: &mut App) -> Result<(), StorybookTestError> {
    #[cfg(not(target_family = "wasm"))]
    gpui_tokio::init(app);
    init_story_runtime(app).map_err(|error| StorybookTestError::RuntimeInitialization {
        message: error.to_string(),
    })?;
    for init in inventory::iter::<InitEntry>() {
        (init.init_fn)(app);
    }
    Ok(())
}

fn apply_controls_to_story(
    story: &Entity<StoryContainer>,
    controls: &BTreeMap<String, ControlValue>,
    app: &mut App,
) -> Result<(), StorybookTestError> {
    if controls.is_empty() {
        return Ok(());
    }
    let target = {
        let story = story.read(app);
        story.control_target()
    }
    .ok_or_else(|| StorybookTestError::ControlsUnavailable {
        key: "capture".to_owned(),
    })?;
    for (key, value) in controls {
        target.set(key, value.clone(), app)?;
    }
    Ok(())
}

fn read_control_snapshots(
    target: Option<Rc<dyn ControlTarget>>,
    app: &mut App,
) -> Result<Vec<ControlSnapshot>, StorybookTestError> {
    match target {
        Some(target) => target.snapshots(app).map_err(StorybookTestError::from),
        None => Ok(Vec::new()),
    }
}

fn uses_core_route_registry(route: &RouteCase, has_custom_route_capture: bool) -> bool {
    matches!(route, RouteCase::Substory { .. }) && !has_custom_route_capture
}

fn viewport_preset(viewport: &ViewportCase) -> StoryViewportPreset {
    match (viewport.width, viewport.height) {
        (390, 844) => StoryViewportPreset::Mobile,
        (768, 1024) => StoryViewportPreset::Tablet,
        (1440, 900) => StoryViewportPreset::Desktop,
        _ => StoryViewportPreset::Responsive,
    }
}

pub(crate) fn encode_id_fragment(value: &str) -> String {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";

    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'_' | b'.')
        {
            encoded.push(char::from(byte));
        } else {
            encoded.push('%');
            encoded.push(char::from(HEX[usize::from(byte >> 4)]));
            encoded.push(char::from(HEX[usize::from(byte & 0x0f)]));
        }
    }
    encoded
}

/// Encodes a case ID as one deterministic PNG filename component.
pub(crate) fn case_file_name(id: &str) -> String {
    format!("id-{}", encode_id_fragment(id))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(not(target_family = "wasm"))]
    use std::sync::atomic::{AtomicBool, Ordering};

    #[cfg(not(target_family = "wasm"))]
    static TOKIO_STORY_INIT_RAN: AtomicBool = AtomicBool::new(false);

    #[cfg(not(target_family = "wasm"))]
    fn tokio_story_init(app: &mut App) {
        let _handle = gpui_tokio::Tokio::handle(app);
        TOKIO_STORY_INIT_RAN.store(true, Ordering::SeqCst);
    }

    #[cfg(not(target_family = "wasm"))]
    inventory::submit! {
        InitEntry {
            init_fn: tokio_story_init,
            fn_name: "tokio_story_init",
            file: file!(),
            line: line!(),
        }
    }

    #[cfg(not(target_family = "wasm"))]
    #[gpui::test]
    fn portable_runtime_installs_tokio_before_story_init_hooks(cx: &mut App) {
        TOKIO_STORY_INIT_RAN.store(false, Ordering::SeqCst);

        initialize_portable_story_app(cx).expect("portable runtime should initialize");

        assert!(TOKIO_STORY_INIT_RAN.load(Ordering::SeqCst));
    }

    #[gpui::test]
    fn story_without_control_target_has_an_empty_snapshot(cx: &mut App) {
        let snapshots = read_control_snapshots(None, cx).expect("missing controls should be empty");

        assert!(snapshots.is_empty());
    }

    #[test]
    fn custom_substory_capture_owns_route_verification() {
        let route = RouteCase::Substory {
            key: "application-surface".to_owned(),
        };

        assert!(!uses_core_route_registry(&route, true));
        assert!(uses_core_route_registry(&route, false));
        assert!(!uses_core_route_registry(&RouteCase::Root, false));
    }

    #[test]
    fn case_file_name_is_stable_and_safe() {
        assert_eq!(
            case_file_name("crate/Button root"),
            "id-crate%2F%42utton%20root"
        );
        assert_eq!(case_file_name(""), "id-");
        assert_ne!(case_file_name("a b"), case_file_name("a?b"));
        assert_ne!(
            case_file_name("A").to_ascii_lowercase(),
            case_file_name("a").to_ascii_lowercase()
        );
    }

    #[test]
    fn explicit_settle_frames_override_the_runner_default() {
        let runner = HeadlessStoryRunner::new(RunnerConfig::default().settle_frames(5));
        let mut request = CaptureRequest::new("crate-Button");
        request.settle_frames = 2;
        let case = runner.request_case(request).unwrap();

        assert_eq!(effective_settle_frames(case.settle_frames, 5, None), 2);
        assert_eq!(effective_settle_frames(0, 5, None), 5);
    }

    #[test]
    fn performance_frames_remain_a_minimum_after_settle_resolution() {
        let performance = PerformanceOptions::new().measured_frames(4);

        assert_eq!(effective_settle_frames(2, 5, Some(&performance)), 4);
        assert_eq!(effective_settle_frames(6, 5, Some(&performance)), 6);
    }

    #[test]
    fn named_theme_and_language_require_a_configurator() {
        let request = CaptureRequest::new("crate-Button");
        let mut case = HeadlessStoryRunner::default()
            .request_case(request)
            .unwrap();
        case.theme = ThemeCase::named("Consumer Theme");
        let error = validate_case_configuration(&case, &RunnerConfig::default()).unwrap_err();
        assert!(matches!(
            error,
            StorybookTestError::CaseConfigurationRequired { axis } if axis == "theme"
        ));
    }

    #[test]
    fn default_matrix_axes_do_not_require_callbacks() {
        let case = HeadlessStoryRunner::default()
            .request_case(CaptureRequest::new("crate-Button"))
            .unwrap();
        validate_case_configuration(&case, &RunnerConfig::default()).unwrap();
    }

    #[test]
    fn built_in_theme_modes_do_not_require_callbacks() {
        let mut case = HeadlessStoryRunner::default()
            .request_case(CaptureRequest::new("crate-Button"))
            .unwrap();
        for theme in ["light", "Default Dark"] {
            case.theme = ThemeCase::named(theme);
            validate_case_configuration(&case, &RunnerConfig::default()).unwrap();
        }
    }
}
