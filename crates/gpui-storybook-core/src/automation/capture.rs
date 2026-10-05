use super::*;
use std::time::{Duration, Instant};

const CAPTURE_LAYOUT_TIMEOUT: Duration = Duration::from_secs(5);

pub(crate) fn schedule_story_capture(
    request_id: u64,
    request: StoryScreenshotRequest,
    story: StorySnapshot,
    response: oneshot::Sender<Result<StoryCaptureSnapshot, StorybookAutomationError>>,
    operation: AutomationOperationGuard,
    quit_after_capture: bool,
    window: &mut Window,
) {
    PendingStoryCapture {
        request_id,
        request,
        story,
        response,
        operation,
        quit_after_capture,
        deadline: Instant::now() + CAPTURE_LAYOUT_TIMEOUT,
    }
    .schedule(window);
}

struct PendingStoryCapture {
    request_id: u64,
    request: StoryScreenshotRequest,
    story: StorySnapshot,
    response: oneshot::Sender<Result<StoryCaptureSnapshot, StorybookAutomationError>>,
    operation: AutomationOperationGuard,
    quit_after_capture: bool,
    deadline: Instant,
}

impl PendingStoryCapture {
    fn is_ready(&self, window: &mut Window, cx: &App) -> Result<bool, StorybookAutomationError> {
        if let Some((width, height)) = validate_capture_target_size(&self.request)? {
            let scale = window.scale_factor().max(f32::EPSILON);
            let expected = gpui_kit::size(px(width as f32 / scale), px(height as f32 / scale));
            let story_key = capture_route_story_key(&self.story.capture_route_id);
            if capture_region_bounds(story_key, window, cx)
                .is_none_or(|region| region.bounds.size != expected)
            {
                return Ok(false);
            }
        }
        ensure_capture_target_visible(&self.story.capture_route_id, window, cx)
            .map(|resized| !resized)
    }

    fn schedule(self, window: &mut Window) {
        if self.response.is_closed() {
            return;
        }
        window.refresh();
        window.on_next_frame(move |window, cx| {
            if self.response.is_closed() {
                return;
            }
            match self.is_ready(window, cx) {
                Ok(true) => self.prepare(window, cx),
                Ok(false) => self.retry(window, cx),
                Err(error) => self.fail(error, cx),
            }
        });
    }

    fn retry(self, window: &mut Window, cx: &mut App) {
        if Instant::now() < self.deadline {
            self.schedule(window);
        } else {
            let error = StorybookAutomationError::CaptureUnavailable {
                message: format!(
                    "capture route `{}` did not fit its visible story pane within 5 seconds",
                    self.story.capture_route_id,
                ),
            };
            self.fail(error, cx);
        }
    }

    fn fail(self, error: StorybookAutomationError, cx: &mut App) {
        let result = Err(error);
        let exit_code = capture_exit_code(&result);
        let _ = self.response.send(result);
        if self.quit_after_capture {
            exit_after_capture(exit_code, cx);
        }
    }

    fn prepare(self, window: &mut Window, cx: &mut App) {
        if !scroll_capture_region_into_view(&self.story.capture_route_id, window, cx) {
            let error = StorybookAutomationError::CaptureUnavailable {
                message: format!(
                    "capture route `{}` was not rendered by the current story view",
                    self.story.capture_route_id,
                ),
            };
            self.fail(error, cx);
            return;
        }

        window.refresh();
        window.on_next_frame(move |window, cx| {
            if self.response.is_closed() {
                return;
            }
            // Scrolling and resizable panels can change the clip on this frame.
            // Only capture after the final rendered geometry still fits.
            match self.is_ready(window, cx) {
                Ok(true) => {
                    let _operation = self.operation;
                    let result =
                        render_story_capture(self.request_id, self.request, self.story, window, cx);
                    let exit_code = capture_exit_code(&result);
                    let _ = self.response.send(result);
                    if self.quit_after_capture {
                        exit_after_capture(exit_code, cx);
                    }
                },
                Ok(false) => self.retry(window, cx),
                Err(error) => self.fail(error, cx),
            }
        });
    }
}

#[cfg(unix)]
fn exit_after_capture(exit_code: i32, _cx: &mut App) -> ! {
    // SAFETY: startup capture owns the process and has completed its output
    // write. `_exit` avoids native platform teardown callbacks after that point.
    unsafe { libc::_exit(exit_code) }
}

#[cfg(not(unix))]
fn exit_after_capture(exit_code: i32, cx: &mut App) {
    if exit_code == 0 {
        cx.quit();
    } else {
        std::process::exit(exit_code);
    }
}

pub fn story_snapshots_from_containers(
    stories: &[gpui_kit::Entity<StoryContainer>],
    cx: &impl Borrow<App>,
) -> Vec<StorySnapshot> {
    fn collect(
        story: &gpui_kit::Entity<StoryContainer>,
        snapshots: &mut Vec<StorySnapshot>,
        cx: &impl Borrow<App>,
    ) {
        let (snapshot, members) = {
            let story = story.read(cx.borrow());
            (
                crate::automation::story_snapshot_from_container(story, cx),
                story.variants.clone(),
            )
        };

        if let Some(snapshot) = snapshot {
            snapshots.push(snapshot);
        }

        for member in members {
            collect(&member, snapshots, cx);
        }
    }

    let mut snapshots = Vec::new();
    for story in stories {
        collect(story, &mut snapshots, cx);
    }
    snapshots
}

pub fn default_capture_output_path(story: &StorySnapshot) -> PathBuf {
    PathBuf::from("target")
        .join("storybook-captures")
        .join(format!("{}.png", story.capture_route_id))
}

pub(crate) fn set_capture_target_size(
    story: &Entity<StoryContainer>,
    window: &Window,
    target_size: Option<(u32, u32)>,
    cx: &mut App,
) {
    let scale_factor = window.scale_factor().max(f32::EPSILON);
    let size = target_size.map(|(width, height)| {
        gpui_kit::size(
            px(width as f32 / scale_factor),
            px(height as f32 / scale_factor),
        )
    });
    story.update(cx, |story, cx| {
        story.set_automation_size(size);
        cx.notify();
    });
}

pub(crate) fn ensure_capture_target_visible(
    route_id: &str,
    window: &mut Window,
    cx: &App,
) -> Result<bool, StorybookAutomationError> {
    let story_key = capture_route_story_key(route_id);
    let region = capture_region_bounds(story_key, window, cx).ok_or_else(|| {
        StorybookAutomationError::CaptureUnavailable {
            message: format!(
                "capture route `{story_key}` was not rendered before validating its target size"
            ),
        }
    })?;
    if region.window_size != window.viewport_size() {
        // A platform configure can arrive before the corresponding redraw.
        // Re-read the pane after that frame instead of accumulating resizes
        // against bounds from the previous window size.
        return Ok(true);
    }
    let Some(target_window_size) = expanded_window_size(
        window.viewport_size(),
        region.bounds,
        region.viewport_bounds,
    ) else {
        return Ok(false);
    };
    window.resize(target_window_size);
    Ok(true)
}

pub(super) fn expanded_window_size(
    window_size: gpui_kit::Size<gpui_kit::Pixels>,
    story_region: gpui_kit::Bounds<gpui_kit::Pixels>,
    viewport: gpui_kit::Bounds<gpui_kit::Pixels>,
) -> Option<gpui_kit::Size<gpui_kit::Pixels>> {
    let width = window_size.width + (story_region.size.width - viewport.size.width).max(px(0.));
    let height = window_size.height + (story_region.size.height - viewport.size.height).max(px(0.));
    if width == window_size.width && height == window_size.height {
        None
    } else {
        Some(gpui_kit::size(width, height))
    }
}

pub(crate) fn render_story_capture(
    request_id: u64,
    request: StoryScreenshotRequest,
    story: StorySnapshot,
    window: &mut Window,
    cx: &App,
) -> Result<StoryCaptureSnapshot, StorybookAutomationError> {
    #[cfg(feature = "capture")]
    {
        let image = window.render_to_image().map_err(|error| {
            StorybookAutomationError::CaptureUnavailable {
                message: format!("failed to render current story to image: {error}"),
            }
        })?;
        let image = crop_story_capture_image(image, &story, window, cx)?;
        let path = request
            .output_path
            .unwrap_or_else(|| default_capture_output_path(&story));

        CaptureOutputStore::create_parent(&path).map_err(|error| {
            StorybookAutomationError::CaptureUnavailable {
                message: format!("failed to create capture output directory: {error}"),
            }
        })?;
        CaptureOutputStore::save_png(&image, &path).map_err(|error| {
            StorybookAutomationError::CaptureUnavailable {
                message: format!(
                    "failed to save story capture to {}: {error}",
                    path.display()
                ),
            }
        })?;

        Ok(StoryCaptureSnapshot {
            request_id,
            path,
            pixel_width: image.width(),
            pixel_height: image.height(),
            story,
            observation: None,
        })
    }

    #[cfg(not(feature = "capture"))]
    {
        let _ = (request_id, request, story, window, cx);
        Err(StorybookAutomationError::CaptureUnavailable {
            message: "story capture requires the gpui-storybook-core `capture` feature".to_string(),
        })
    }
}

#[cfg(feature = "capture")]
fn crop_story_capture_image(
    image: image::RgbaImage,
    story: &StorySnapshot,
    window: &Window,
    cx: &App,
) -> Result<image::RgbaImage, StorybookAutomationError> {
    let region = capture_region_bounds(&story.capture_route_id, window, cx).ok_or_else(|| {
        StorybookAutomationError::CaptureUnavailable {
            message: format!(
                "capture route `{}` was not rendered by the current story view",
                story.capture_route_id
            ),
        }
    })?;
    let window_size = window.viewport_size();
    let window_bounds = Bounds {
        origin: point(px(0.), px(0.)),
        size: window_size,
    };
    let bounds = region
        .bounds
        .intersect(&region.viewport_bounds)
        .intersect(&window_bounds);

    let Some((x, y, width, height)) = image_crop_rect(bounds, window_size, &image) else {
        return Err(StorybookAutomationError::CaptureUnavailable {
            message: format!(
                "capture route `{}` is outside the rendered story view",
                story.capture_route_id
            ),
        });
    };

    Ok(image::imageops::crop_imm(&image, x, y, width, height).to_image())
}

#[cfg(feature = "capture")]
pub(super) fn image_crop_rect(
    bounds: Bounds<Pixels>,
    window_size: gpui_kit::Size<Pixels>,
    image: &image::RgbaImage,
) -> Option<(u32, u32, u32, u32)> {
    let window_width = f32::from(window_size.width);
    let window_height = f32::from(window_size.height);
    if window_width <= 0. || window_height <= 0. || image.width() == 0 || image.height() == 0 {
        return None;
    }

    let x_scale = image.width() as f32 / window_width;
    let y_scale = image.height() as f32 / window_height;
    let left = (f32::from(bounds.origin.x) * x_scale)
        .floor()
        .clamp(0., image.width() as f32) as u32;
    let top = (f32::from(bounds.origin.y) * y_scale)
        .floor()
        .clamp(0., image.height() as f32) as u32;
    let right = ((f32::from(bounds.origin.x) + f32::from(bounds.size.width)) * x_scale)
        .ceil()
        .clamp(0., image.width() as f32) as u32;
    let bottom = ((f32::from(bounds.origin.y) + f32::from(bounds.size.height)) * y_scale)
        .ceil()
        .clamp(0., image.height() as f32) as u32;

    let width = right.checked_sub(left)?;
    let height = bottom.checked_sub(top)?;
    if width == 0 || height == 0 {
        return None;
    }

    Some((left, top, width, height))
}

pub(crate) fn capture_exit_code(
    result: &Result<StoryCaptureSnapshot, StorybookAutomationError>,
) -> i32 {
    if let Err(error) = result {
        eprintln!("gpui-storybook capture session failed: {error}");
        1
    } else {
        0
    }
}

/// Gallery policy for desktop resizing and image rendering.
pub(crate) struct DesktopCaptureProvider;
impl gpui_storybook_automation_gpui::interaction::InteractionCaptureProvider
    for DesktopCaptureProvider
{
    fn ensure_visible(
        &self,
        route: &str,
        window: &mut Window,
        cx: &App,
    ) -> Result<bool, StorybookAutomationError> {
        ensure_capture_target_visible(route, window, cx)
    }
    fn capture(
        &self,
        request_id: u64,
        request: StoryScreenshotRequest,
        story: StorySnapshot,
        window: &mut Window,
        cx: &App,
    ) -> Result<StoryCaptureSnapshot, StorybookAutomationError> {
        render_story_capture(request_id, request, story, window, cx)
    }
}
