//! Run against an explicitly selected disposable emulator after APK installation.
use gpui_storybook_mobile_host::{AdbTransport, AdbTransportOptions};

#[tokio::test]
#[ignore = "requires STORYBOOK_ANDROID_SERIAL and an exclusively owned opted-in emulator"]
async fn canceled_smoke_response_retains_captures_and_lifecycle_restoration() {
    use gpui_storybook_automation::AutomationBackend as _;
    use gpui_storybook_mobile_host::{
        AndroidLaunch, ReadinessOptions, RemoteBackend, SmokeOptions, SmokePlan, SmokeRunner,
    };
    let serial = std::env::var("STORYBOOK_ANDROID_SERIAL").unwrap();
    assert!(serial.starts_with("emulator-"));
    let transport =
        AdbTransport::new(AdbTransportOptions::builder().serial(serial).build()).unwrap();
    let app = AndroidLaunch::load(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../examples/mobile/storybook.toml"),
    )
    .unwrap();
    let port = app.application().device_port;
    let initial =
        RemoteBackend::attach_when_ready(transport.clone(), port, ReadinessOptions::default())
            .await
            .unwrap();
    let orientation = initial.host().await.unwrap().orientation();
    initial.shutdown().await;
    let runner = SmokeRunner::new(transport.clone(), port, Some(app.application().clone()));
    let output = tempfile::tempdir().unwrap();
    let worker = runner.clone();
    let path = output.path().to_owned();
    let response = tokio::spawn(async move {
        worker
            .run(
                SmokePlan::default(),
                SmokeOptions::builder()
                    .output(path)
                    .allow_interaction(true)
                    .lifecycle(true)
                    .build(),
            )
            .await
    });
    while !runner.is_running() {
        tokio::task::yield_now().await;
    }
    response.abort();
    assert!(response.await.unwrap_err().is_cancelled());
    runner.shutdown().await;
    for name in [
        "initial-display.png",
        "initial-gpui.png",
        "rotated-display.png",
        "rotated-gpui.png",
    ] {
        let (width, height) = image::image_dimensions(output.path().join(name)).unwrap();
        assert!(
            width > 0 && height > 0,
            "owned smoke work finished after response cancellation"
        );
    }
    let restored = RemoteBackend::attach_when_ready(
        transport.clone(),
        port,
        ReadinessOptions::builder().orientation(orientation).build(),
    )
    .await
    .unwrap();
    assert_eq!(restored.host().await.unwrap().orientation(), orientation);
    restored.shutdown().await;
    transport.shutdown().await;
}

#[tokio::test]
#[ignore = "requires STORYBOOK_ANDROID_SERIAL and the opted-in example APK"]
async fn native_shell_status_and_failed_install_preserve_the_package() {
    let serial = std::env::var("STORYBOOK_ANDROID_SERIAL").expect("explicit emulator serial");
    assert!(serial.starts_with("emulator-"));
    let transport =
        AdbTransport::new(AdbTransportOptions::builder().serial(serial).build()).unwrap();
    let result = transport
        .shell(&["sh", "-c", "printf output; printf error >&2; exit 7"])
        .await
        .unwrap();
    assert_eq!(
        (result.stdout(), result.stderr(), result.status()),
        (b"output".as_slice(), b"error".as_slice(), 7)
    );
    let before = transport
        .shell(&["pm", "path", "dev.storybook.mobile"])
        .await
        .unwrap()
        .require_success()
        .unwrap();
    let files = transport
        .shell(&["ls", "-1", "/data/local/tmp"])
        .await
        .unwrap()
        .require_success()
        .unwrap();
    let directory = tempfile::tempdir().unwrap();
    let apk = directory.path().join("invalid.apk");
    std::fs::write(&apk, b"not an APK").unwrap();
    assert!(transport.install(apk).await.is_err());
    let after = transport
        .shell(&["pm", "path", "dev.storybook.mobile"])
        .await
        .unwrap()
        .require_success()
        .unwrap();
    assert_eq!(before.stdout(), after.stdout());
    let after_files = transport
        .shell(&["ls", "-1", "/data/local/tmp"])
        .await
        .unwrap()
        .require_success()
        .unwrap();
    assert_eq!(
        files.stdout(),
        after_files.stdout(),
        "failed install removed its owned artifact"
    );
}
