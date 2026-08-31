use super::*;

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

    pub(super) fn request_case(
        &self,
        request: CaptureRequest,
    ) -> Result<CaptureCase, StorybookTestError> {
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
