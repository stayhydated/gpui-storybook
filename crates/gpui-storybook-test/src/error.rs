use super::*;

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
