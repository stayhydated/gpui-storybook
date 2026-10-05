//! Run against an explicitly selected disposable emulator after APK installation.
use gpui_storybook_mobile_host::{AdbTransport, AdbTransportOptions};

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
