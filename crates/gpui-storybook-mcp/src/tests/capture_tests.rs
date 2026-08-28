use super::*;

#[test]
fn capture_output_schema_accepts_runtime_snapshot_shape() {
    let snapshot = StoryCaptureSnapshot {
        request_id: 7,
        path: PathBuf::from("target/storybook-captures/button.png"),
        pixel_width: 900,
        pixel_height: 700,
        story: sample_story(),
    };
    let definition = component_shape_mcp::tool_definition(
        "capture_schema_test",
        None,
        None,
        McpSchema::object().with_additional_properties(false),
        Some(capture_story_output_schema()),
    )
    .expect("capture schema should define a valid tool");
    let mut server = McpServer::new("capture-schema-test", "0.0.0");
    server
        .add_tool(definition, move |_| tool_structured_result(json!(snapshot)))
        .expect("schema test tool should register");

    let result = serde_json::to_value(server.call_tool("capture_schema_test", Some(json!({}))))
        .expect("result should serialize");
    assert_eq!(result["isError"], false);
    assert_eq!(result["structuredContent"]["pixel_width"], 900);
}

#[test]
fn capture_launch_env_returns_wgpu_env_and_command() {
    let automation = StorybookAutomation::with_stories(Vec::new());
    let server = server(automation).expect("server should build");

    let result = server.call_tool(
        TOOL_CAPTURE_LAUNCH_ENV,
        Some(json!({
            "story_key": "gpui-storybook-example-story-ButtonStory",
            "output_path": "target/storybook-captures/button.png",
            "width": 900,
            "height": 700,
            "package": "gpui-storybook-example-story",
            "bin": "story",
            "features": ["mcp"],
        })),
    );
    let result = serde_json::to_value(result).unwrap();
    let structured =
        tool_call_structured_content(&result).expect("tool should return structured content");

    assert_eq!(
        structured["env"]["WGPU_CAPTURE_ROUTE"],
        "gpui-storybook-example-story-ButtonStory"
    );
    assert_eq!(
        structured["env"]["WGPU_CAPTURE_PATH"],
        "target/storybook-captures/button.png"
    );
    assert_eq!(structured["env"]["WGPU_CAPTURE_WIDTH"], "900");
    assert_eq!(structured["env"]["WGPU_CAPTURE_HEIGHT"], "700");
    assert_eq!(structured["env"][STDIO_ENV_VAR], "1");
    #[cfg(target_os = "linux")]
    assert_eq!(
        structured["command"],
        json!([
            "gpui-storybook-launch",
            "--",
            "cargo",
            "run",
            "-p",
            "gpui-storybook-example-story",
            "--features",
            "mcp",
            "--bin",
            "story"
        ])
    );
    #[cfg(target_os = "macos")]
    assert_eq!(
        structured["command"],
        json!([
            "cargo",
            "run",
            "-p",
            "gpui-storybook-example-story",
            "--features",
            "mcp",
            "--bin",
            "story"
        ])
    );
}

#[test]
fn platform_launch_commands_wrap_only_linux() {
    let cargo_args = [
        "run".to_string(),
        "--features".to_string(),
        "mcp".to_string(),
    ];

    assert_eq!(
        cargo_launch_command_for(&["gpui-storybook-launch", "--", "cargo"], &cargo_args),
        [
            "gpui-storybook-launch",
            "--",
            "cargo",
            "run",
            "--features",
            "mcp",
        ]
    );
    assert_eq!(
        cargo_launch_command_for(&["cargo"], &cargo_args),
        ["cargo", "run", "--features", "mcp"]
    );
}

#[test]
fn read_capture_session_reads_wgpu_env() {
    let _lock = ENV_LOCK.lock().expect("env lock should not be poisoned");
    let _env = EnvGuard::set(&[
        (
            "WGPU_CAPTURE_ROUTE",
            "gpui-storybook-example-story-ButtonStory",
        ),
        ("WGPU_CAPTURE_PATH", "target/storybook-captures/button.png"),
        ("WGPU_CAPTURE_WIDTH", "900"),
        ("WGPU_CAPTURE_HEIGHT", "700"),
    ]);

    let session = read_capture_session("fallback-story").unwrap();
    let capture = session.capture.expect("capture config should be read");

    assert_eq!(
        session.story_key,
        "gpui-storybook-example-story-ButtonStory"
    );
    assert_eq!(
        capture.path,
        PathBuf::from("target/storybook-captures/button.png")
    );
    assert_eq!(capture.size.width, 900);
    assert_eq!(capture.size.height, 700);
}

#[test]
fn capture_launch_env_rejects_invalid_frame_capture_values() {
    let error = build_capture_launch_env(CaptureLaunchEnvInput {
        story_key: "gpui-storybook-example-story-ButtonStory".to_string(),
        output_path: Some(PathBuf::from("target/storybook-captures/button.png")),
        frame: Some(0),
        width: None,
        height: None,
        viewport: None,
        package: None,
        bin: None,
        features: None,
        stdio: None,
    })
    .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("capture frame must be greater than zero")
    );

    let error = build_capture_launch_env(CaptureLaunchEnvInput {
        story_key: "gpui-storybook-example-story-ButtonStory".to_string(),
        output_path: None,
        frame: None,
        width: Some(900),
        height: None,
        viewport: None,
        package: None,
        bin: None,
        features: None,
        stdio: None,
    })
    .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("set both capture width and height")
    );
}

#[test]
fn stdio_flag_requires_the_explicit_enabled_value() {
    let _lock = ENV_LOCK.lock().expect("env lock should not be poisoned");
    let _unset = EnvGuard::remove(&[STDIO_ENV_VAR]);
    assert!(!stdio_requested());

    {
        let _disabled = EnvGuard::set(&[(STDIO_ENV_VAR, "0")]);
        assert!(!stdio_requested());
    }
    {
        let _enabled = EnvGuard::set(&[(STDIO_ENV_VAR, "1")]);
        assert!(stdio_requested());
    }
}

#[test]
fn capture_request_uses_the_frame_capture_route_and_path_contract() {
    let _lock = ENV_LOCK.lock().expect("env lock should not be poisoned");
    let capture_env = storybook_capture_env();
    let route_var = capture_env.route_var().to_owned();
    let path_var = capture_env.path_var().to_owned();
    let _unset = EnvGuard::remove(&[&route_var, &path_var]);

    assert!(!capture_requested());

    {
        let _route = EnvGuard::set(&[(&route_var, "example-ButtonStory")]);
        assert!(capture_requested());
    }
    {
        let _path = EnvGuard::set(&[(&path_var, "target/storybook-captures/button.png")]);
        assert!(capture_requested());
    }
}

#[test]
fn capture_catalog_exposes_route_metadata_only() {
    let story = sample_story();
    assert_eq!(
        capture_catalog(std::slice::from_ref(&story)),
        json!({
            "routes": [{
                "id": story.capture_route_id,
                "title": story.title,
                "default_size": story.default_size,
            }]
        })
    );
    assert_eq!(capture_catalog(&[]), json!({ "routes": [] }));
}

#[test]
fn capture_session_defaults_without_capture_environment() {
    let _lock = ENV_LOCK.lock().expect("env lock should not be poisoned");
    let _env = EnvGuard::remove(&[
        "WGPU_CAPTURE_ROUTE",
        "WGPU_CAPTURE_PATH",
        "WGPU_CAPTURE_FRAME",
        "WGPU_CAPTURE_WIDTH",
        "WGPU_CAPTURE_HEIGHT",
    ]);

    let session = read_capture_session("fallback-story").expect("fallback should be valid");
    assert_eq!(
        session,
        StorybookCaptureSession {
            story_key: "fallback-story".to_string(),
            capture: None,
        }
    );

    let error = read_capture_session("").expect_err("blank fallback route should fail");
    assert!(matches!(
        error,
        StorybookMcpError::InvalidDefaultStoryKey { key, .. } if key.is_empty()
    ));
}

#[test]
fn capture_launch_env_supports_minimal_non_stdio_commands() {
    let launch = build_capture_launch_env(CaptureLaunchEnvInput {
        story_key: "example-ButtonStory".to_string(),
        output_path: None,
        frame: None,
        width: None,
        height: None,
        viewport: None,
        package: None,
        bin: None,
        features: Some(Vec::new()),
        stdio: Some(false),
    })
    .expect("minimal launch environment should build");

    assert_eq!(launch.cargo_args, vec!["run"]);
    #[cfg(target_os = "linux")]
    assert_eq!(
        launch.command,
        vec!["gpui-storybook-launch", "--", "cargo", "run",]
    );
    #[cfg(target_os = "macos")]
    assert_eq!(launch.command, vec!["cargo", "run"]);
    assert!(!launch.env.contains_key(STDIO_ENV_VAR));
    assert_eq!(launch.env["WGPU_CAPTURE_ROUTE"], "example-ButtonStory");

    let mobile = build_capture_launch_env(CaptureLaunchEnvInput {
        story_key: "example-ButtonStory".to_string(),
        output_path: Some(PathBuf::from("mobile.png")),
        frame: None,
        width: None,
        height: None,
        viewport: Some(SchemarsValue(StoryViewportPreset::Mobile)),
        package: None,
        bin: None,
        features: None,
        stdio: Some(false),
    })
    .expect("mobile viewport should build");
    assert_eq!(mobile.env["WGPU_CAPTURE_WIDTH"], "390");
    assert_eq!(mobile.env["WGPU_CAPTURE_HEIGHT"], "844");
}

#[test]
fn capture_session_thread_reports_missing_live_host() {
    let automation = StorybookAutomation::with_stories(vec![sample_story()]);
    let handle = start_capture_session(
        automation,
        StorybookCaptureSession {
            story_key: "example-ButtonStory".to_string(),
            capture: None,
        },
        false,
    )
    .expect("capture session thread should start");

    let error = handle
        .join()
        .expect("capture session thread should not panic")
        .expect_err("a detached automation host should fail");
    assert!(matches!(
        error,
        StorybookMcpError::Automation(StorybookAutomationError::NoLiveHost)
    ));
}

#[test]
fn capture_session_from_env_handles_absent_and_late_story_registration() {
    let _lock = ENV_LOCK.lock().expect("env lock should not be poisoned");
    let _clean = EnvGuard::remove(&["WGPU_CAPTURE_ROUTE", "WGPU_CAPTURE_PATH"]);
    assert!(
        start_capture_session_from_env(StorybookAutomation::new())
            .expect("absent capture env should not fail")
            .is_none()
    );

    let _route = EnvGuard::set(&[("WGPU_CAPTURE_ROUTE", "example-ButtonStory")]);
    let automation = StorybookAutomation::new();
    let handle = start_capture_session_from_env(automation.clone())
        .expect("capture waiter should start")
        .expect("capture route should request a session");
    automation.set_stories(vec![sample_story()]);

    let error = handle
        .join()
        .expect("capture waiter should not panic")
        .expect_err("a detached automation host should fail");
    assert!(matches!(
        error,
        StorybookMcpError::Automation(StorybookAutomationError::NoLiveHost)
    ));
}
