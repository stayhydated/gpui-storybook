#[cfg(feature = "capture")]
use super::capture_region_bounds;
#[cfg(feature = "capture")]
use gpui::{Bounds, Pixels, Size, point, px};

/// Failure to crop a rendered full-window image to one registered story route.
#[derive(Clone, Debug, Eq, thiserror::Error, PartialEq)]
pub enum CaptureRegionImageError {
    /// The requested route did not register bounds during the latest frame.
    #[error("capture route `{route_id}` was not rendered")]
    RouteNotRendered {
        /// Fully qualified story or substory route.
        route_id: String,
    },
    /// The registered route does not overlap a non-empty portion of the image.
    #[error("capture route `{route_id}` is outside the rendered window image")]
    RouteOutsideImage {
        /// Fully qualified story or substory route.
        route_id: String,
    },
}

/// Crops a full-window image to bounds registered by a rendered story route.
///
/// `window_size` uses GPUI logical pixels; the image may use a different
/// physical pixel size. The conversion scales each axis independently and
/// clips the route to the window before cropping. This is the portable visual
/// runner's built-in root and substory crop contract.
#[cfg(feature = "capture")]
pub fn crop_capture_region_image(
    route_id: &str,
    image: image::RgbaImage,
    window_size: Size<Pixels>,
) -> Result<image::RgbaImage, CaptureRegionImageError> {
    let region = capture_region_bounds(route_id).ok_or_else(|| {
        CaptureRegionImageError::RouteNotRendered {
            route_id: route_id.to_owned(),
        }
    })?;
    let window_bounds = Bounds {
        origin: point(px(0.), px(0.)),
        size: window_size,
    };
    let bounds = region.bounds.intersect(&window_bounds);
    let window_width = f32::from(window_size.width);
    let window_height = f32::from(window_size.height);
    if window_width <= 0. || window_height <= 0. || image.width() == 0 || image.height() == 0 {
        return Err(CaptureRegionImageError::RouteOutsideImage {
            route_id: route_id.to_owned(),
        });
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
    let Some(width) = right.checked_sub(left) else {
        return Err(CaptureRegionImageError::RouteOutsideImage {
            route_id: route_id.to_owned(),
        });
    };
    let Some(height) = bottom.checked_sub(top) else {
        return Err(CaptureRegionImageError::RouteOutsideImage {
            route_id: route_id.to_owned(),
        });
    };
    if width == 0 || height == 0 {
        return Err(CaptureRegionImageError::RouteOutsideImage {
            route_id: route_id.to_owned(),
        });
    }

    Ok(image::imageops::crop_imm(&image, left, top, width, height).to_image())
}
