use super::*;

/// A fresh executable story and its isolated headless app context.
pub struct PortableStory {
    pub(super) context: HeadlessAppContext,
    pub(super) window: WindowHandle<StoryContainer>,
    pub(super) story: Entity<StoryContainer>,
    pub(super) descriptor: StoryDescriptor,
    pub(super) case: CaptureCase,
    pub(super) route_capture: Option<RouteCapture>,
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
