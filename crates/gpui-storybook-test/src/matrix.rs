//! Capture request and matrix planning types.

use crate::{PerformanceOptions, StoryDescriptor, StorybookTestError, baseline::BaselineStatus};
use gpui_storybook_core::{
    capture_region::capture_substory_route_id_with_key, controls::ControlValue,
    presentation::StoryCanvasBackground,
};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::PathBuf};

/// A named logical viewport used for one capture case.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ViewportCase {
    /// Stable case label used in reports and paths.
    pub name: String,
    /// Window width in logical pixels.
    pub width: u32,
    /// Window height in logical pixels.
    pub height: u32,
}

impl ViewportCase {
    /// Creates a named viewport. Positive dimensions are validated when a case
    /// is expanded or executed.
    pub fn new(name: impl Into<String>, width: u32, height: u32) -> Self {
        Self {
            name: name.into(),
            width,
            height,
        }
    }

    /// The default responsive capture region.
    pub fn responsive() -> Self {
        Self::new("responsive", 1280, 720)
    }

    /// A compact mobile capture region.
    pub fn mobile() -> Self {
        Self::new("mobile", 390, 844)
    }

    /// A tablet capture region.
    pub fn tablet() -> Self {
        Self::new("tablet", 768, 1024)
    }

    /// A desktop capture region.
    pub fn desktop() -> Self {
        Self::new("desktop", 1440, 900)
    }

    pub(crate) fn validate(&self) -> Result<(), StorybookTestError> {
        if self.name.trim().is_empty() {
            return Err(StorybookTestError::InvalidViewport {
                name: self.name.clone(),
                message: "viewport name must not be empty".to_owned(),
            });
        }
        if self.width == 0 || self.height == 0 {
            return Err(StorybookTestError::InvalidViewport {
                name: self.name.clone(),
                message: "viewport width and height must be greater than zero".to_owned(),
            });
        }
        Ok(())
    }
}

impl Default for ViewportCase {
    fn default() -> Self {
        Self::responsive()
    }
}

/// A canvas background selected for one capture case.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PresentationCase {
    /// Stable case label used in reports and paths.
    pub name: String,
    /// Story canvas background.
    pub background: StoryCanvasBackground,
}

impl PresentationCase {
    /// Creates a named presentation case.
    pub fn new(name: impl Into<String>, background: StoryCanvasBackground) -> Self {
        Self {
            name: name.into(),
            background,
        }
    }

    /// Uses the Storybook theme background.
    pub fn theme() -> Self {
        Self::new("theme", StoryCanvasBackground::Theme)
    }

    /// Uses a light canvas background.
    pub fn light() -> Self {
        Self::new("light", StoryCanvasBackground::Light)
    }

    /// Uses a dark canvas background.
    pub fn dark() -> Self {
        Self::new("dark", StoryCanvasBackground::Dark)
    }

    /// Uses a transparent canvas background.
    pub fn transparent() -> Self {
        Self::new("transparent", StoryCanvasBackground::Transparent)
    }

    pub(crate) fn validate(&self) -> Result<(), StorybookTestError> {
        if self.name.trim().is_empty() {
            Err(StorybookTestError::InvalidCase {
                field: "presentation.name".to_owned(),
                message: "presentation name must not be empty".to_owned(),
            })
        } else {
            Ok(())
        }
    }
}

impl Default for PresentationCase {
    fn default() -> Self {
        Self::theme()
    }
}

/// A named theme selection for one capture case.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ThemeCase {
    /// Stable case label used in reports and paths.
    pub name: String,
    /// Consumer-registered theme name passed to the case configurator.
    pub theme: Option<String>,
}

impl ThemeCase {
    /// Keeps the app's already-selected theme.
    pub fn current() -> Self {
        Self {
            name: "current".to_owned(),
            theme: None,
        }
    }

    /// Selects a consumer-registered GPUI Component theme by name.
    pub fn named(theme: impl Into<String>) -> Self {
        let theme = theme.into();
        Self {
            name: theme.clone(),
            theme: Some(theme),
        }
    }

    pub(crate) fn validate(&self) -> Result<(), StorybookTestError> {
        if self.name.trim().is_empty() {
            return Err(StorybookTestError::InvalidCase {
                field: "theme.name".to_owned(),
                message: "theme name must not be empty".to_owned(),
            });
        }
        if self
            .theme
            .as_ref()
            .is_some_and(|theme| theme.trim().is_empty())
        {
            return Err(StorybookTestError::InvalidCase {
                field: "theme.theme".to_owned(),
                message: "theme selection must not be empty".to_owned(),
            });
        }
        Ok(())
    }
}

impl Default for ThemeCase {
    fn default() -> Self {
        Self::current()
    }
}

/// A named language selection for one capture case.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LanguageCase {
    /// Stable case label used in reports and paths.
    pub name: String,
    /// BCP-47 language tag passed to the case configurator.
    pub language: Option<String>,
}

impl LanguageCase {
    /// Keeps the app's already-selected language.
    pub fn current() -> Self {
        Self {
            name: "current".to_owned(),
            language: None,
        }
    }

    /// Selects a language tag through the case configurator.
    pub fn named(language: impl Into<String>) -> Self {
        let language = language.into();
        Self {
            name: language.clone(),
            language: Some(language),
        }
    }

    pub(crate) fn validate(&self) -> Result<(), StorybookTestError> {
        if self.name.trim().is_empty() {
            return Err(StorybookTestError::InvalidCase {
                field: "language.name".to_owned(),
                message: "language name must not be empty".to_owned(),
            });
        }
        if self
            .language
            .as_ref()
            .is_some_and(|language| language.trim().is_empty())
        {
            return Err(StorybookTestError::InvalidCase {
                field: "language.language".to_owned(),
                message: "language selection must not be empty".to_owned(),
            });
        }
        Ok(())
    }
}

impl Default for LanguageCase {
    fn default() -> Self {
        Self::current()
    }
}

/// A route within a story. Root captures use the story route; sub-story
/// captures use the stable key supplied to `capture_substory_with_key` or the
/// slug generated by `capture_substory`.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum RouteCase {
    /// Capture the complete story region.
    #[default]
    Root,
    /// Capture one story-local section.
    Substory {
        /// Stable section key, without the parent story key.
        key: String,
    },
}

impl RouteCase {
    /// Creates a root route case.
    pub const fn root() -> Self {
        Self::Root
    }

    /// Creates a sub-story route case from its stable section key.
    pub fn substory(key: impl Into<String>) -> Self {
        Self::Substory { key: key.into() }
    }

    /// Builds the fully qualified route for `story_key`.
    pub fn route_id(&self, story_key: &str) -> Result<String, StorybookTestError> {
        match self {
            Self::Root => Ok(story_key.to_owned()),
            Self::Substory { key } if key.trim().is_empty() => {
                Err(StorybookTestError::InvalidCase {
                    field: "route.key".to_owned(),
                    message: "substory key must not be empty".to_owned(),
                })
            },
            Self::Substory { key } => Ok(capture_substory_route_id_with_key(story_key, key)),
        }
    }

    pub(crate) fn label(&self) -> &str {
        match self {
            Self::Root => "root",
            Self::Substory { key } => key,
        }
    }
}

/// A named set of typed control values.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ControlCase {
    /// Stable case label used in reports and paths.
    pub name: String,
    /// Values applied before the first settled frame.
    #[serde(default)]
    pub values: BTreeMap<String, ControlValue>,
}

impl ControlCase {
    /// Creates a named control case.
    pub fn new(name: impl Into<String>, values: BTreeMap<String, ControlValue>) -> Self {
        Self {
            name: name.into(),
            values,
        }
    }

    /// Creates the empty/default control case.
    pub fn defaults() -> Self {
        Self::new("default", BTreeMap::new())
    }

    pub(crate) fn validate(&self) -> Result<(), StorybookTestError> {
        if self.name.trim().is_empty() {
            Err(StorybookTestError::InvalidCase {
                field: "controls.name".to_owned(),
                message: "control case name must not be empty".to_owned(),
            })
        } else {
            Ok(())
        }
    }
}

/// One fully expanded, executable capture case.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CaptureCase {
    /// Stable case ID used for output and baseline paths.
    pub id: String,
    /// Registered story key.
    pub story_key: String,
    /// Fully qualified story or sub-story route.
    pub route_id: String,
    /// Route selector used to produce `route_id`.
    pub route: RouteCase,
    /// Viewport dimensions.
    pub viewport: ViewportCase,
    /// Canvas presentation.
    pub presentation: PresentationCase,
    /// Theme selection.
    pub theme: ThemeCase,
    /// Language selection.
    pub language: LanguageCase,
    /// Typed control values.
    pub controls: ControlCase,
    /// Optional PNG output path.
    pub output_path: Option<PathBuf>,
    /// Number of settled frames; zero means the runner default.
    pub settle_frames: u32,
    /// Optional profiler collection and budget.
    pub performance: Option<PerformanceOptions>,
}

impl CaptureCase {
    /// Converts this case into the request executed by a runner.
    pub fn request(&self) -> CaptureRequest {
        CaptureRequest {
            case_id: Some(self.id.clone()),
            story_key: self.story_key.clone(),
            route: self.route.clone(),
            viewport: self.viewport.clone(),
            presentation: self.presentation.clone(),
            theme: self.theme.clone(),
            language: self.language.clone(),
            controls: self.controls.values.clone(),
            output_path: self.output_path.clone(),
            settle_frames: self.settle_frames,
            performance: self.performance.clone(),
        }
    }
}

/// A single portable story capture request.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CaptureRequest {
    /// Optional explicit case ID. Matrix cases always set one.
    pub case_id: Option<String>,
    /// Registered story key.
    pub story_key: String,
    /// Story or sub-story route.
    #[serde(default)]
    pub route: RouteCase,
    /// Viewport dimensions.
    #[serde(default)]
    pub viewport: ViewportCase,
    /// Canvas presentation.
    #[serde(default)]
    pub presentation: PresentationCase,
    /// Theme selection.
    #[serde(default)]
    pub theme: ThemeCase,
    /// Language selection.
    #[serde(default)]
    pub language: LanguageCase,
    /// Typed control values applied before rendering.
    #[serde(default)]
    pub controls: BTreeMap<String, ControlValue>,
    /// Optional PNG output path.
    pub output_path: Option<PathBuf>,
    /// Number of settled frames; zero means the runner default.
    #[serde(default)]
    pub settle_frames: u32,
    /// Optional profiler collection and budget.
    pub performance: Option<PerformanceOptions>,
}

impl CaptureRequest {
    /// Creates a root capture request using the default responsive viewport.
    pub fn new(story_key: impl Into<String>) -> Self {
        Self {
            case_id: None,
            story_key: story_key.into(),
            route: RouteCase::default(),
            viewport: ViewportCase::default(),
            presentation: PresentationCase::default(),
            theme: ThemeCase::default(),
            language: LanguageCase::default(),
            controls: BTreeMap::new(),
            output_path: None,
            settle_frames: 0,
            performance: None,
        }
    }

    /// Returns the stable ID used when no explicit matrix ID was supplied.
    pub fn id(&self) -> String {
        if let Some(case_id) = &self.case_id {
            return case_id.clone();
        }
        let controls = if self.controls.is_empty() {
            "default".to_owned()
        } else {
            let serialized = serde_json::to_string(&self.controls)
                .expect("string-keyed ControlValue maps serialize to JSON");
            crate::encode_id_fragment(&serialized)
        };
        format!(
            "{}/{}/{}/{}/{}/{}/{}",
            self.story_key,
            self.route.label(),
            self.viewport.name,
            self.presentation.name,
            self.theme.name,
            self.language.name,
            controls,
        )
    }

    pub(crate) fn validate(&self) -> Result<String, StorybookTestError> {
        if self.story_key.trim().is_empty() {
            return Err(StorybookTestError::InvalidCase {
                field: "story_key".to_owned(),
                message: "story key must not be empty".to_owned(),
            });
        }
        self.viewport.validate()?;
        self.presentation.validate()?;
        self.theme.validate()?;
        self.language.validate()?;
        let route_id = self.route.route_id(&self.story_key)?;
        if let Some(case_id) = &self.case_id
            && case_id.trim().is_empty()
        {
            return Err(StorybookTestError::InvalidCase {
                field: "case_id".to_owned(),
                message: "case ID must not be empty".to_owned(),
            });
        }
        Ok(route_id)
    }
}

/// A Cartesian-product capture plan.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CaptureMatrix {
    /// Selected story keys. Empty selects every discovered story.
    #[serde(default)]
    pub story_keys: Vec<String>,
    /// Route cases. Empty selects the root route.
    #[serde(default)]
    pub routes: Vec<RouteCase>,
    /// Viewports. Empty selects the default responsive viewport.
    #[serde(default)]
    pub viewports: Vec<ViewportCase>,
    /// Canvas presentation cases. Empty selects the theme background.
    #[serde(default)]
    pub presentations: Vec<PresentationCase>,
    /// Theme cases. Empty keeps the current theme.
    #[serde(default)]
    pub themes: Vec<ThemeCase>,
    /// Language cases. Empty keeps the current language.
    #[serde(default)]
    pub languages: Vec<LanguageCase>,
    /// Named control sets. Empty selects the default values.
    #[serde(default)]
    pub control_cases: Vec<ControlCase>,
    /// Optional output directory for generated PNGs.
    pub output_dir: Option<PathBuf>,
    /// Optional per-case frame settle override.
    pub settle_frames: Option<u32>,
    /// Optional profiler collection and budget applied to every case.
    pub performance: Option<PerformanceOptions>,
}

impl CaptureMatrix {
    /// Creates an empty matrix whose dimensions resolve to deterministic defaults.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds one story key to the plan.
    pub fn story(mut self, key: impl Into<String>) -> Self {
        self.story_keys.push(key.into());
        self
    }

    /// Adds one route case to the plan.
    pub fn route(mut self, route: RouteCase) -> Self {
        self.routes.push(route);
        self
    }

    /// Adds one viewport case to the plan.
    pub fn viewport(mut self, viewport: ViewportCase) -> Self {
        self.viewports.push(viewport);
        self
    }

    /// Adds one presentation case to the plan.
    pub fn presentation(mut self, presentation: PresentationCase) -> Self {
        self.presentations.push(presentation);
        self
    }

    /// Adds one theme case to the plan.
    pub fn theme(mut self, theme: ThemeCase) -> Self {
        self.themes.push(theme);
        self
    }

    /// Adds one language case to the plan.
    pub fn language(mut self, language: LanguageCase) -> Self {
        self.languages.push(language);
        self
    }

    /// Adds one named control set to the plan.
    pub fn controls(mut self, controls: ControlCase) -> Self {
        self.control_cases.push(controls);
        self
    }

    /// Sets the generated PNG output directory.
    pub fn output_dir(mut self, output_dir: impl Into<PathBuf>) -> Self {
        self.output_dir = Some(output_dir.into());
        self
    }

    /// Sets the number of settled frames for each case.
    pub const fn settle_frames(mut self, settle_frames: u32) -> Self {
        self.settle_frames = Some(settle_frames);
        self
    }

    /// Applies one profiler configuration to every expanded case.
    pub fn performance(mut self, performance: PerformanceOptions) -> Self {
        self.performance = Some(performance);
        self
    }

    /// Expands this plan against discovered story descriptors.
    pub fn expand(
        &self,
        discovered: &[StoryDescriptor],
    ) -> Result<Vec<CaptureCase>, StorybookTestError> {
        let stories = if self.story_keys.is_empty() {
            discovered.to_vec()
        } else {
            self.story_keys
                .iter()
                .map(|key| {
                    discovered
                        .iter()
                        .find(|story| story.key() == key)
                        .cloned()
                        .ok_or_else(|| StorybookTestError::StoryNotFound { key: key.clone() })
                })
                .collect::<Result<Vec<_>, _>>()?
        };
        let routes = if self.routes.is_empty() {
            vec![RouteCase::Root]
        } else {
            self.routes.clone()
        };
        let viewports = if self.viewports.is_empty() {
            vec![ViewportCase::default()]
        } else {
            self.viewports.clone()
        };
        let presentations = if self.presentations.is_empty() {
            vec![PresentationCase::default()]
        } else {
            self.presentations.clone()
        };
        let themes = if self.themes.is_empty() {
            vec![ThemeCase::default()]
        } else {
            self.themes.clone()
        };
        let languages = if self.languages.is_empty() {
            vec![LanguageCase::default()]
        } else {
            self.languages.clone()
        };
        let control_cases = if self.control_cases.is_empty() {
            vec![ControlCase::defaults()]
        } else {
            self.control_cases.clone()
        };

        let mut cases = Vec::new();
        for story in stories {
            for route in &routes {
                let route_id = route.route_id(story.key())?;
                for viewport in &viewports {
                    viewport.validate()?;
                    for presentation in &presentations {
                        presentation.validate()?;
                        for theme in &themes {
                            theme.validate()?;
                            for language in &languages {
                                language.validate()?;
                                for controls in &control_cases {
                                    controls.validate()?;
                                    let id = format!(
                                        "{}/{}/{}/{}/{}/{}/{}/{}/{}",
                                        story.key(),
                                        route.label(),
                                        viewport.name,
                                        viewport.width,
                                        viewport.height,
                                        presentation.name,
                                        theme.name,
                                        language.name,
                                        controls.name,
                                    );
                                    let output_path = self.output_dir.as_ref().map(|directory| {
                                        directory
                                            .join(format!("{}.png", crate::case_file_name(&id)))
                                    });
                                    cases.push(CaptureCase {
                                        id,
                                        story_key: story.key().to_owned(),
                                        route_id: route_id.clone(),
                                        route: route.clone(),
                                        viewport: viewport.clone(),
                                        presentation: presentation.clone(),
                                        theme: theme.clone(),
                                        language: language.clone(),
                                        controls: controls.clone(),
                                        output_path,
                                        settle_frames: self.settle_frames.unwrap_or_default(),
                                        performance: self.performance.clone(),
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }
        cases.sort_by(|left, right| left.id.cmp(&right.id));
        Ok(cases)
    }
}

/// Outcome of one matrix case.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CaseStatus {
    /// Capture completed without a baseline failure.
    Passed,
    /// Capture completed but the requested baseline does not exist.
    BaselineMissing,
    /// Capture completed but differs from its baseline.
    BaselineMismatch,
    /// Capture completed and intentionally updated its baseline.
    BaselineUpdated,
    /// The case could not be rendered, configured, or checked.
    Failed,
}

/// Serializable error information for one failed matrix case.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CaseFailure {
    /// Stable error category.
    pub kind: String,
    /// Human-readable detail.
    pub message: String,
    /// Typed performance failure, when a performance budget rejected a capture.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub performance: Option<crate::PerformanceBudgetFailure>,
}

/// Structured result for one matrix case.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CaseReport {
    /// Stable case ID.
    pub id: String,
    /// Case status.
    pub status: CaseStatus,
    /// Executed request.
    pub request: CaptureRequest,
    /// Capture and baseline details when execution reached that stage.
    pub capture: Option<crate::CaptureReport>,
    /// Failure details when execution did not pass.
    pub error: Option<CaseFailure>,
}

impl CaseReport {
    pub(crate) fn passed(
        id: String,
        request: CaptureRequest,
        capture: crate::CaptureReport,
    ) -> Self {
        let status = match capture.baseline.as_ref().map(|baseline| baseline.status) {
            Some(BaselineStatus::Missing) => CaseStatus::BaselineMissing,
            Some(BaselineStatus::Mismatch) => CaseStatus::BaselineMismatch,
            Some(BaselineStatus::Updated) => CaseStatus::BaselineUpdated,
            Some(BaselineStatus::Match) | None => CaseStatus::Passed,
        };
        Self {
            id,
            status,
            request,
            capture: Some(capture),
            error: None,
        }
    }

    pub(crate) fn failed_with_error(
        id: String,
        request: CaptureRequest,
        error: &StorybookTestError,
    ) -> Self {
        Self {
            id,
            status: CaseStatus::Failed,
            request,
            capture: error.capture_report().cloned(),
            error: Some(CaseFailure {
                kind: error.kind().to_owned(),
                message: error.to_string(),
                performance: error.performance_failure().cloned(),
            }),
        }
    }
}

/// Structured result for an entire capture matrix.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MatrixReport {
    /// Every expanded case in deterministic order.
    pub cases: Vec<CaseReport>,
    /// Whether every case passed its requested capture and baseline policy.
    pub passed: bool,
}

impl MatrixReport {
    /// Returns the number of expanded cases.
    pub fn len(&self) -> usize {
        self.cases.len()
    }

    /// Returns whether no cases were expanded.
    pub fn is_empty(&self) -> bool {
        self.cases.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui_storybook_core::controls::ControlValue;

    fn descriptors() -> Vec<StoryDescriptor> {
        vec![StoryDescriptor::for_test("crate-Button", "Button")]
    }

    #[test]
    fn matrix_expands_all_requested_dimensions_deterministically() {
        let mut values = BTreeMap::new();
        values.insert("enabled".to_owned(), ControlValue::Boolean(true));
        let matrix = CaptureMatrix::new()
            .story("crate-Button")
            .route(RouteCase::root())
            .route(RouteCase::substory("states"))
            .viewport(ViewportCase::mobile())
            .viewport(ViewportCase::desktop())
            .presentation(PresentationCase::theme())
            .presentation(PresentationCase::dark())
            .theme(ThemeCase::current())
            .theme(ThemeCase::named("Default Dark"))
            .language(LanguageCase::current())
            .language(LanguageCase::named("fr-FR"))
            .controls(ControlCase::new("enabled", values));

        let cases = matrix.expand(&descriptors()).unwrap();
        assert_eq!(cases.len(), 2 * 2 * 2 * 2 * 2);
        assert_eq!(cases[0].route_id, "crate-Button");
        assert!(
            cases
                .iter()
                .any(|case| case.route_id == "crate-Button/states")
        );
        assert!(cases.windows(2).all(|window| window[0].id < window[1].id));
    }

    #[test]
    fn unknown_selected_story_is_a_planning_error() {
        let matrix = CaptureMatrix::new().story("missing");
        let error = matrix
            .expand(&descriptors())
            .expect_err("story should be unknown");
        assert!(matches!(error, StorybookTestError::StoryNotFound { .. }));
    }

    #[test]
    fn request_ids_include_controls_when_not_explicit() {
        let mut request = CaptureRequest::new("crate-Button");
        request
            .controls
            .insert("enabled".to_owned(), ControlValue::Boolean(true));
        let id = request.id();
        assert!(id.starts_with("crate-Button/root/responsive/theme/current/current/"));
        assert!(id.contains("enabled"));
    }

    #[test]
    fn request_ids_preserve_distinct_control_values() {
        let request_id = |value: &str| {
            let mut request = CaptureRequest::new("crate-Button");
            request
                .controls
                .insert("label".to_owned(), ControlValue::Text(value.to_owned()));
            request.id()
        };

        assert_ne!(request_id("a-b"), request_id("a_b"));
    }

    #[test]
    fn matrix_output_paths_preserve_distinct_case_labels() {
        let matrix = CaptureMatrix::new()
            .story("crate-Button")
            .controls(ControlCase::new("a b", BTreeMap::new()))
            .controls(ControlCase::new("a?b", BTreeMap::new()))
            .output_dir("target/captures");

        let cases = matrix.expand(&descriptors()).unwrap();
        let first = cases[0]
            .output_path
            .as_ref()
            .expect("matrix output path should be generated");
        let second = cases[1]
            .output_path
            .as_ref()
            .expect("matrix output path should be generated");

        assert_ne!(first, second);
    }

    #[test]
    fn route_case_builds_stable_substory_route() {
        assert_eq!(
            RouteCase::substory("With Icon")
                .route_id("crate-Button")
                .unwrap(),
            "crate-Button/with-icon"
        );
    }

    #[test]
    fn matrix_settle_override_wins_over_a_larger_runner_default() {
        let cases = CaptureMatrix::new()
            .story("crate-Button")
            .settle_frames(2)
            .expand(&descriptors())
            .unwrap();

        assert_eq!(cases[0].settle_frames, 2);
        assert_eq!(
            crate::effective_settle_frames(cases[0].settle_frames, 5, None),
            2
        );
    }
}
