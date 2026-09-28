//! End-to-end headless capture smoke test for the portable runner.
//!
//! The runner renders through `gpui_platform::current_headless_renderer`, which
//! needs no display server or compositor: Linux uses the Wgpu platform renderer
//! (hardware or Mesa software adapter) and macOS uses the Metal headless
//! renderer. Windows has no headless renderer in the pinned `gpui-pre` stack,
//! so this test only runs on the supported capture platforms.

#![cfg(any(target_os = "linux", target_os = "macos"))]

use std::{path::PathBuf, sync::Arc};

use gpui_storybook_test::{CaptureRequest, HeadlessStoryRunner, RunnerConfig};

// Keep the story-bearing library linked so inventory discovery sees its
// registrations.
use gpui_storybook_example_story as _;

const STORY_KEY: &str = "gpui-storybook-example-story-ButtonStory";

#[test]
fn portable_runner_captures_a_registered_story() {
    let runner = HeadlessStoryRunner::new(
        RunnerConfig::default().asset_source(Arc::new(gpui_storybook::Assets)),
    );
    let output = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("portable-captures")
        .join("button.png");
    let mut request = CaptureRequest::new(STORY_KEY);
    request.output_path = Some(output.clone());

    let report = runner
        .capture(request)
        .expect("the portable runner should capture a registered story");

    assert_eq!(report.story.key, STORY_KEY);
    assert!(
        report.width > 0 && report.height > 0,
        "capture should have rendered dimensions"
    );
    let bytes =
        std::fs::read(&output).expect("the capture should write a PNG at the requested path");
    assert!(
        bytes.starts_with(b"\x89PNG\r\n\x1a\n"),
        "the capture output should be a PNG"
    );
}
