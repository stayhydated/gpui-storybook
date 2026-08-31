//! Explicit PNG baseline storage and comparison.

use image::RgbaImage;
use serde::{Deserialize, Serialize};
use std::{
    fs, io,
    path::{Path, PathBuf},
};
use thiserror::Error;

/// The action a capture runner should take for a visual baseline.
///
/// Baselines are never updated implicitly. Use [`BaselinePolicy::Update`] only
/// for an intentional acceptance run, and use [`BaselinePolicy::Check`] for
/// verification or CI.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "snake_case", tag = "action")]
pub enum BaselinePolicy {
    /// Do not read or write a baseline.
    #[default]
    Ignore,
    /// Compare the capture with the baseline at the store's case path.
    Check {
        /// Pixel comparison tolerance.
        tolerance: BaselineTolerance,
    },
    /// Replace the baseline with the newly captured image.
    Update,
}

impl BaselinePolicy {
    /// Creates a comparison policy using `tolerance`.
    pub const fn check(tolerance: BaselineTolerance) -> Self {
        Self::Check { tolerance }
    }
}

/// Tolerance used when comparing two RGBA PNG images.
///
/// `per_channel_tolerance` is applied before a pixel is counted as different.
/// The mean absolute channel error is still reported for review and is bounded
/// independently by `max_mean_absolute_error`.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BaselineTolerance {
    /// Maximum absolute difference for each channel that is ignored.
    pub per_channel_tolerance: u8,
    /// Maximum number of pixels that may differ after channel tolerance.
    pub max_different_pixels: u64,
    /// Maximum mean absolute error per channel, in the range `0..=255`.
    pub max_mean_absolute_error: f64,
}

impl Default for BaselineTolerance {
    fn default() -> Self {
        Self {
            per_channel_tolerance: 0,
            max_different_pixels: 0,
            max_mean_absolute_error: 0.0,
        }
    }
}

impl BaselineTolerance {
    /// Creates an exact pixel comparison policy.
    pub const fn exact() -> Self {
        Self {
            per_channel_tolerance: 0,
            max_different_pixels: 0,
            max_mean_absolute_error: 0.0,
        }
    }

    /// Validates values that cannot be represented by an image comparison.
    pub fn validate(self) -> Result<Self, BaselineError> {
        if self.max_mean_absolute_error.is_finite()
            && (0.0..=255.0).contains(&self.max_mean_absolute_error)
        {
            Ok(self)
        } else {
            Err(BaselineError::InvalidTolerance {
                message: "max_mean_absolute_error must be finite and within 0..=255".to_owned(),
            })
        }
    }
}

/// The result of comparing or updating one baseline image.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BaselineReport {
    /// Baseline path selected by the store.
    pub path: PathBuf,
    /// Comparison or update outcome.
    pub status: BaselineStatus,
    /// Dimensions of the newly captured image.
    pub actual_width: u32,
    /// Dimensions of the newly captured image.
    pub actual_height: u32,
    /// Dimensions of the existing baseline, if one was available.
    pub expected_width: Option<u32>,
    /// Dimensions of the existing baseline, if one was available.
    pub expected_height: Option<u32>,
    /// Number of pixels whose channels exceeded the per-channel tolerance.
    pub different_pixels: u64,
    /// Largest absolute channel delta observed.
    pub max_channel_delta: u8,
    /// Mean absolute error across all channels.
    pub mean_absolute_error: f64,
}

impl BaselineReport {
    /// Returns whether this report represents an accepted comparison.
    pub const fn matches(&self) -> bool {
        matches!(self.status, BaselineStatus::Match | BaselineStatus::Updated)
    }
}

/// Outcome recorded in a [`BaselineReport`].
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BaselineStatus {
    /// No baseline exists at the selected path.
    Missing,
    /// The capture is within the requested tolerance.
    Match,
    /// The baseline exists but the capture is outside the requested tolerance.
    Mismatch,
    /// The baseline was intentionally replaced by the capture.
    Updated,
}

/// Filesystem-backed storage for visual baselines.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BaselineStore {
    root: PathBuf,
}

impl BaselineStore {
    /// Creates a store rooted at `root`.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// Returns the root directory without creating it.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Returns the deterministic PNG path for a case ID.
    ///
    /// Each slash-separated case-ID component is encoded before being joined
    /// below the store root. This keeps generated IDs useful as directories
    /// while preventing absolute paths or parent traversal from escaping the
    /// store. The `.png` suffix is appended to the final encoded component so
    /// dots already present in a case ID remain part of its one-to-one path.
    pub fn path_for(&self, case_id: &str) -> PathBuf {
        let mut path = self.root.clone();
        let components = case_id
            .split('/')
            .filter(|component| !component.is_empty())
            .map(encode_component)
            .collect::<Vec<_>>();

        if let Some((file_name, directories)) = components.split_last() {
            for component in directories {
                path.push(component);
            }
            path.push(format!("{file_name}.png"));
        } else {
            path.push("unnamed.png");
        }
        path
    }

    /// Compares `actual` with the baseline at `case_id`.
    ///
    /// A missing baseline is a normal report outcome, while malformed or
    /// unreadable existing files are returned as errors so CI cannot silently
    /// accept a broken baseline directory.
    pub fn check(
        &self,
        case_id: &str,
        actual: &RgbaImage,
        tolerance: BaselineTolerance,
    ) -> Result<BaselineReport, BaselineError> {
        let tolerance = tolerance.validate()?;
        let path = self.path_for(case_id);
        let expected = match image::open(&path) {
            Ok(image) => image.into_rgba8(),
            Err(image::ImageError::IoError(error)) if error.kind() == io::ErrorKind::NotFound => {
                return Ok(BaselineReport {
                    path,
                    status: BaselineStatus::Missing,
                    actual_width: actual.width(),
                    actual_height: actual.height(),
                    expected_width: None,
                    expected_height: None,
                    different_pixels: 0,
                    max_channel_delta: 0,
                    mean_absolute_error: 0.0,
                });
            },
            Err(error) => {
                return Err(BaselineError::Decode {
                    path,
                    source: error,
                });
            },
        };

        let metrics = image_metrics(actual, &expected, tolerance.per_channel_tolerance);
        let status = if actual.dimensions() == expected.dimensions()
            && metrics.different_pixels <= tolerance.max_different_pixels
            && metrics.mean_absolute_error <= tolerance.max_mean_absolute_error
        {
            BaselineStatus::Match
        } else {
            BaselineStatus::Mismatch
        };

        Ok(BaselineReport {
            path,
            status,
            actual_width: actual.width(),
            actual_height: actual.height(),
            expected_width: Some(expected.width()),
            expected_height: Some(expected.height()),
            different_pixels: metrics.different_pixels,
            max_channel_delta: metrics.max_channel_delta,
            mean_absolute_error: metrics.mean_absolute_error,
        })
    }

    /// Intentionally replaces the baseline at `case_id` with `actual`.
    pub fn update(
        &self,
        case_id: &str,
        actual: &RgbaImage,
    ) -> Result<BaselineReport, BaselineError> {
        let path = self.path_for(case_id);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|source| BaselineError::Io {
                path: parent.to_path_buf(),
                source,
            })?;
        }

        actual
            .save_with_format(&path, image::ImageFormat::Png)
            .map_err(|source| BaselineError::Encode {
                path: path.clone(),
                source,
            })?;

        Ok(BaselineReport {
            path,
            status: BaselineStatus::Updated,
            actual_width: actual.width(),
            actual_height: actual.height(),
            expected_width: Some(actual.width()),
            expected_height: Some(actual.height()),
            different_pixels: 0,
            max_channel_delta: 0,
            mean_absolute_error: 0.0,
        })
    }
}

/// Errors produced while reading, comparing, or updating a baseline.
#[derive(Debug, Error)]
pub enum BaselineError {
    /// A baseline directory operation failed.
    #[error("baseline filesystem operation for {} failed: {source}", path.display())]
    Io {
        /// Path involved in the operation.
        path: PathBuf,
        /// Underlying filesystem error.
        #[source]
        source: io::Error,
    },
    /// An existing baseline could not be decoded as an image.
    #[error("failed to decode baseline {}: {source}", path.display())]
    Decode {
        /// Path involved in the operation.
        path: PathBuf,
        /// Underlying image error.
        #[source]
        source: image::ImageError,
    },
    /// A capture could not be written as a PNG.
    #[error("failed to encode baseline {}: {source}", path.display())]
    Encode {
        /// Path involved in the operation.
        path: PathBuf,
        /// Underlying image error.
        #[source]
        source: image::ImageError,
    },
    /// A comparison tolerance was invalid.
    #[error("invalid baseline tolerance: {message}")]
    InvalidTolerance {
        /// Validation detail.
        message: String,
    },
}

#[derive(Clone, Copy, Debug, Default)]
struct ImageMetrics {
    different_pixels: u64,
    max_channel_delta: u8,
    mean_absolute_error: f64,
}

fn image_metrics(actual: &RgbaImage, expected: &RgbaImage, channel_tolerance: u8) -> ImageMetrics {
    if actual.dimensions() != expected.dimensions() {
        let actual_pixels = u64::from(actual.width()) * u64::from(actual.height());
        let expected_pixels = u64::from(expected.width()) * u64::from(expected.height());
        return ImageMetrics {
            different_pixels: actual_pixels.max(expected_pixels),
            max_channel_delta: u8::MAX,
            mean_absolute_error: f64::from(u8::MAX),
        };
    }

    let mut metrics = ImageMetrics::default();
    let mut total_error = 0u64;
    for (actual, expected) in actual.pixels().zip(expected.pixels()) {
        let mut different = false;
        for (actual, expected) in actual.0.into_iter().zip(expected.0) {
            let delta = actual.abs_diff(expected);
            metrics.max_channel_delta = metrics.max_channel_delta.max(delta);
            total_error += u64::from(delta);
            different |= delta > channel_tolerance;
        }
        if different {
            metrics.different_pixels += 1;
        }
    }

    let channel_count = u64::from(actual.width())
        .saturating_mul(u64::from(actual.height()))
        .saturating_mul(4);
    metrics.mean_absolute_error = if channel_count == 0 {
        0.0
    } else {
        total_error as f64 / channel_count as f64
    };
    metrics
}

fn encode_component(component: &str) -> String {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";

    let escape_dots = matches!(component, "." | "..");
    let mut encoded = String::with_capacity(component.len());
    for byte in component.bytes() {
        if (byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
            && !(escape_dots && byte == b'.')
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

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;

    #[test]
    fn baseline_paths_are_encoded_below_store_root() {
        let store = BaselineStore::new("target/baselines");
        let path = store.path_for("../outside//story key");
        assert_eq!(
            path,
            PathBuf::from("target/baselines/%2E%2E/outside/story%20key.png")
        );
        assert!(path.starts_with(store.root()));
    }

    #[test]
    fn dotted_case_ids_keep_distinct_png_paths() {
        let store = BaselineStore::new("target/baselines");
        let first = store.path_for("controls/v1.0");
        let second = store.path_for("controls/v1.1");

        assert_eq!(first, PathBuf::from("target/baselines/controls/v1.0.png"));
        assert_eq!(second, PathBuf::from("target/baselines/controls/v1.1.png"));
        assert_ne!(first, second);
        assert_ne!(
            store.path_for("controls/a b"),
            store.path_for("controls/a?b")
        );
    }

    #[test]
    fn exact_comparison_reports_match_and_mismatch() {
        let directory = tempfile::tempdir().expect("temporary baseline directory");
        let store = BaselineStore::new(directory.path());
        let image = RgbaImage::from_pixel(2, 1, Rgba([10, 20, 30, 255]));
        store.update("button/default", &image).unwrap();

        let matching = store
            .check("button/default", &image, BaselineTolerance::exact())
            .unwrap();
        assert_eq!(matching.status, BaselineStatus::Match);
        assert_eq!(matching.different_pixels, 0);

        let changed = RgbaImage::from_pixel(2, 1, Rgba([11, 20, 30, 255]));
        let mismatching = store
            .check("button/default", &changed, BaselineTolerance::exact())
            .unwrap();
        assert_eq!(mismatching.status, BaselineStatus::Mismatch);
        assert_eq!(mismatching.different_pixels, 2);
        assert_eq!(mismatching.max_channel_delta, 1);
    }

    #[test]
    fn missing_baseline_is_reported_without_creating_it() {
        let directory = tempfile::tempdir().expect("temporary baseline directory");
        let store = BaselineStore::new(directory.path());
        let image = RgbaImage::from_pixel(1, 1, Rgba([0, 0, 0, 0]));

        let report = store
            .check("missing", &image, BaselineTolerance::default())
            .unwrap();
        assert_eq!(report.status, BaselineStatus::Missing);
        assert!(!store.path_for("missing").exists());
    }

    #[test]
    fn tolerance_can_accept_channel_deltas() {
        let directory = tempfile::tempdir().expect("temporary baseline directory");
        let store = BaselineStore::new(directory.path());
        let image = RgbaImage::from_pixel(1, 1, Rgba([10, 20, 30, 255]));
        store.update("tolerated", &image).unwrap();

        let changed = RgbaImage::from_pixel(1, 1, Rgba([11, 20, 30, 255]));
        let tolerance = BaselineTolerance {
            per_channel_tolerance: 1,
            max_different_pixels: 0,
            max_mean_absolute_error: 1.0,
        };
        let report = store.check("tolerated", &changed, tolerance).unwrap();
        assert_eq!(report.status, BaselineStatus::Match);
        assert_eq!(report.mean_absolute_error, 0.25);
    }
}
