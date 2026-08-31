use super::*;

pub(crate) fn schedule_story_capture(
    request_id: u64,
    request: StoryScreenshotRequest,
    story: StorySnapshot,
    response: oneshot::Sender<Result<StoryCaptureSnapshot, StorybookAutomationError>>,
    operation: AutomationOperationGuard,
    quit_after_capture: bool,
    window: &mut Window,
) {
    if response.is_closed() {
        return;
    }
    window.on_next_frame(move |window, cx| {
        if response.is_closed() {
            return;
        }
        let resized = match ensure_capture_target_visible(&story.capture_route_id, window) {
            Ok(resized) => resized,
            Err(error) => {
                let result = Err(error);
                let exit_code = capture_exit_code(&result);
                let _ = response.send(result);
                if quit_after_capture {
                    exit_after_capture(exit_code, cx);
                }
                return;
            },
        };
        if resized {
            window.refresh();
            window.on_next_frame(move |window, _cx| {
                prepare_story_capture(
                    request_id,
                    request,
                    story,
                    response,
                    operation,
                    quit_after_capture,
                    window,
                )
            });
        } else {
            prepare_story_capture(
                request_id,
                request,
                story,
                response,
                operation,
                quit_after_capture,
                window,
            );
        }
    });
}

fn prepare_story_capture(
    request_id: u64,
    request: StoryScreenshotRequest,
    story: StorySnapshot,
    response: oneshot::Sender<Result<StoryCaptureSnapshot, StorybookAutomationError>>,
    operation: AutomationOperationGuard,
    quit_after_capture: bool,
    window: &mut Window,
) {
    if response.is_closed() {
        return;
    }
    if !scroll_capture_region_into_view(&story.capture_route_id) {
        let result = Err(StorybookAutomationError::CaptureUnavailable {
            message: format!(
                "capture route `{}` was not rendered by the current story view",
                story.capture_route_id
            ),
        });
        let exit_code = capture_exit_code(&result);
        let _ = response.send(result);
        if quit_after_capture {
            std::process::exit(exit_code);
        }
        return;
    }

    window.refresh();
    window.on_next_frame(move |window, cx| {
        let _operation = operation;
        let result = render_story_capture(request_id, request, story, window);
        let exit_code = capture_exit_code(&result);
        let _ = response.send(result);
        if quit_after_capture {
            exit_after_capture(exit_code, cx);
        }
    });
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
    stories: &[gpui::Entity<StoryContainer>],
    cx: &impl Borrow<App>,
) -> Vec<StorySnapshot> {
    fn collect(
        story: &gpui::Entity<StoryContainer>,
        snapshots: &mut Vec<StorySnapshot>,
        cx: &impl Borrow<App>,
    ) {
        let (snapshot, members) = {
            let story = story.read(cx.borrow());
            (
                StorySnapshot::from_container(story, cx),
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

pub(crate) fn validate_capture_target_size(
    request: &StoryScreenshotRequest,
) -> Result<Option<(u32, u32)>, StorybookAutomationError> {
    match (request.width, request.height) {
        (Some(width), Some(height)) if width > 0 && height > 0 => Ok(Some((width, height))),
        (Some(_), Some(_)) => Err(StorybookAutomationError::InvalidCaptureRequest {
            message: "capture width and height must be greater than zero".to_string(),
        }),
        (None, None) => Ok(request.viewport.and_then(StoryViewportPreset::dimensions)),
        _ => Err(StorybookAutomationError::InvalidCaptureRequest {
            message: "capture width and height must be provided together".to_string(),
        }),
    }
}

pub(crate) fn set_capture_target_size(
    story: &Entity<StoryContainer>,
    window: &Window,
    target_size: Option<(u32, u32)>,
    cx: &mut App,
) {
    let scale_factor = window.scale_factor().max(f32::EPSILON);
    let size = target_size.map(|(width, height)| {
        gpui::size(
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
) -> Result<bool, StorybookAutomationError> {
    let story_key = capture_route_story_key(route_id);
    let region = capture_region_bounds(story_key).ok_or_else(|| {
        StorybookAutomationError::CaptureUnavailable {
            message: format!(
                "capture route `{story_key}` was not rendered before validating its target size"
            ),
        }
    })?;
    let Some(target_window_size) = expanded_window_size(window.bounds().size, region.bounds) else {
        return Ok(false);
    };
    window.resize(target_window_size);
    Ok(true)
}

pub(super) fn expanded_window_size(
    window_size: gpui::Size<gpui::Pixels>,
    story_region: gpui::Bounds<gpui::Pixels>,
) -> Option<gpui::Size<gpui::Pixels>> {
    let required_width =
        (f32::from(story_region.origin.x) + f32::from(story_region.size.width)).max(0.0);
    let required_height =
        (f32::from(story_region.origin.y) + f32::from(story_region.size.height)).max(0.0);
    let width = f32::from(window_size.width).max(required_width);
    let height = f32::from(window_size.height).max(required_height);
    if width == f32::from(window_size.width) && height == f32::from(window_size.height) {
        None
    } else {
        Some(gpui::size(px(width), px(height)))
    }
}

pub(crate) fn render_story_capture(
    request_id: u64,
    request: StoryScreenshotRequest,
    story: StorySnapshot,
    window: &mut Window,
) -> Result<StoryCaptureSnapshot, StorybookAutomationError> {
    #[cfg(feature = "capture")]
    {
        let image = window.render_to_image().map_err(|error| {
            StorybookAutomationError::CaptureUnavailable {
                message: format!("failed to render current story to image: {error}"),
            }
        })?;
        let image = crop_story_capture_image(image, &story, window)?;
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
        })
    }

    #[cfg(not(feature = "capture"))]
    {
        let _ = (request_id, request, story, window);
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
) -> Result<image::RgbaImage, StorybookAutomationError> {
    let region = capture_region_bounds(&story.capture_route_id).ok_or_else(|| {
        StorybookAutomationError::CaptureUnavailable {
            message: format!(
                "capture route `{}` was not rendered by the current story view",
                story.capture_route_id
            ),
        }
    })?;
    let window_size = window.bounds().size;
    let window_bounds = Bounds {
        origin: point(px(0.), px(0.)),
        size: window_size,
    };
    let bounds = region.bounds.intersect(&window_bounds);

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
    window_size: gpui::Size<Pixels>,
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
