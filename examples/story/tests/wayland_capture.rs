//! Verifies the application window capture path under a private Wayland compositor.
//! Run with `just wayland-capture-test` using the workspace's Git-pinned GPUI backends.

#![cfg(all(target_os = "linux", feature = "mcp"))]

use gpui_storybook_launch::{LaunchCommand, LaunchOptions};

#[test]
#[ignore = "requires Sway and a working Wgpu adapter"]
fn startup_capture_renders_a_story_under_wayland() {
    let directory = tempfile::tempdir().expect("create capture output directory");
    for (width, height) in [(390, 844), (801, 601)] {
        let path = directory
            .path()
            .join(format!("button-{width}x{height}.png"));
        let command = LaunchCommand::new(
            "timeout",
            [
                "45s".to_owned(),
                "env".to_owned(),
                "-u".to_owned(),
                "GPUI_STORYBOOK_MCP_STDIO".to_owned(),
                "-u".to_owned(),
                "WGPU_CAPTURE_FRAME".to_owned(),
                "WGPU_CAPTURE_ROUTE=gpui-storybook-example-story-ButtonStory".to_owned(),
                format!("WGPU_CAPTURE_PATH={}", path.display()),
                format!("WGPU_CAPTURE_WIDTH={width}"),
                format!("WGPU_CAPTURE_HEIGHT={height}"),
                env!("CARGO_BIN_EXE_gpui-storybook-example-story").to_owned(),
            ],
        );
        let status = gpui_storybook_launch::run(&command, &LaunchOptions::default())
            .expect("launch Storybook under private Sway");
        assert!(status.success(), "startup capture failed: {status}");
        let image = image::open(&path)
            .expect("decode captured PNG")
            .into_rgba8();
        assert_eq!(image.dimensions(), (width, height));
        assert!(
            image
                .pixels()
                .any(|pixel| pixel[0] < 64 && pixel[1] < 64 && pixel[2] < 64),
            "capture must contain the story's text and buttons"
        );
        assert!(
            image
                .pixels()
                .any(|pixel| pixel[0] > 220 && pixel[1] > 220 && pixel[2] > 220),
            "capture must contain the story's light background"
        );
        assert!(
            image.pixels().any(|pixel| pixel[0].abs_diff(pixel[1]) > 40),
            "capture must include the colored button variants"
        );
    }
}
