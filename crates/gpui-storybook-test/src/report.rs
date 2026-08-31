use super::*;

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
