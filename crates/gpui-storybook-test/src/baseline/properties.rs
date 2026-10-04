use super::*;
use image::Rgba;
use proptest::prelude::*;

fn image_pair() -> impl Strategy<Value = (RgbaImage, RgbaImage)> {
    (0u32..17, 0u32..17).prop_flat_map(|(width, height)| {
        let length = (width * height * 4) as usize;
        (
            prop::collection::vec(any::<u8>(), length),
            prop::collection::vec(any::<u8>(), length),
        )
            .prop_map(move |(actual, expected)| {
                (
                    RgbaImage::from_raw(width, height, actual).unwrap(),
                    RgbaImage::from_raw(width, height, expected).unwrap(),
                )
            })
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    #[test]
    fn metrics_are_symmetric_and_tolerance_only_changes_pixel_count(
        (actual, expected) in image_pair(),
        first in any::<u8>(),
        second in any::<u8>(),
    ) {
        let lower = image_metrics(&actual, &expected, first.min(second));
        let higher = image_metrics(&actual, &expected, first.max(second));
        let swapped = image_metrics(&expected, &actual, first.min(second));
        prop_assert_eq!(lower.different_pixels, swapped.different_pixels);
        prop_assert_eq!(lower.max_channel_delta, swapped.max_channel_delta);
        prop_assert_eq!(lower.mean_absolute_error, swapped.mean_absolute_error);
        prop_assert!(higher.different_pixels <= lower.different_pixels);
        prop_assert_eq!(lower.max_channel_delta, higher.max_channel_delta);
        prop_assert_eq!(lower.mean_absolute_error, higher.mean_absolute_error);
        prop_assert!(lower.different_pixels <= u64::from(actual.width()) * u64::from(actual.height()));
        prop_assert!((0.0..=255.0).contains(&lower.mean_absolute_error));
        let identical = image_metrics(&actual, &actual, first);
        prop_assert_eq!(identical.different_pixels, 0);
        prop_assert_eq!(identical.max_channel_delta, 0);
        prop_assert_eq!(identical.mean_absolute_error, 0.0);
    }

    #[test]
    fn known_channel_changes_have_exact_counts_and_mean_error(
        width in 1u32..17,
        height in 1u32..17,
        changed_seed in 0u32..257,
        channel in 0usize..4,
        delta in 1u8..=255,
        tolerance in any::<u8>(),
    ) {
        let pixel_count = width * height;
        let changed_count = changed_seed.min(pixel_count);
        let expected = RgbaImage::from_pixel(width, height, Rgba([0; 4]));
        let mut actual = expected.clone();
        for pixel in actual.pixels_mut().take(changed_count as usize) {
            pixel.0[channel] = delta;
        }
        let metrics = image_metrics(&actual, &expected, tolerance);
        let expected_differences = if delta > tolerance { u64::from(changed_count) } else { 0 };
        prop_assert_eq!(metrics.different_pixels, expected_differences);
        prop_assert_eq!(metrics.max_channel_delta, if changed_count == 0 { 0 } else { delta });
        // A controlled one-channel perturbation supplies a scalar oracle that
        // does not reproduce the production nested pixel/channel loop.
        let expected_mean = f64::from(changed_count) * f64::from(delta) / (4.0 * f64::from(pixel_count));
        prop_assert_eq!(metrics.mean_absolute_error, expected_mean);
        let at_threshold = image_metrics(&actual, &expected, delta);
        prop_assert_eq!(at_threshold.different_pixels, 0);
        prop_assert_eq!(at_threshold.mean_absolute_error, expected_mean);
    }
}
