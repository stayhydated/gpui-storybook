# Android automation probes

An isolated, unpublished Cargo workspace compares Android libraries against
the embedded Counter/Notes example. A separate AndroidX instrumentation APK
tests native UI behavior. Production dependencies stay in the root workspace.

| Candidate | Version | Role |
| --- | --- | --- |
| `adb_client` | 3.2.3 | Blocking ADB shell, installation, forwarding, capture |
| `adbutils-rs` | 0.1.0 | Async ADB and direct device sockets |
| `uiautomator` | 1.0.2 | Rust JSON-RPC client and native service bootstrap |
| `uiautomator2-rs` | 0.1.2 | Rust native service lifecycle and JSON-RPC transport |
| AndroidX UI Automator | 2.4.0 | Native selectors, input, lifecycle, PNG capture |

Run on an exclusively owned disposable API 36 Pixel 7 emulator with AOSP en-US
LatinIME. The probes install the example, restart it, and manage UI Automator
helper processes on that serial. `adb` must be on `PATH`. Build the opted-in
example with the [mobile example commands](../README.md).

```sh
cargo build --manifest-path examples/mobile/tools/Cargo.toml --locked
python3 examples/mobile/tools/run.py \
  --serial emulator-5580 --apk target/mobile-example/x86_64/storybook.apk
python3 examples/mobile/tools/build_androidx.py --sdk "$ANDROID_HOME"
python3 examples/mobile/tools/run_androidx.py --serial emulator-5580
```

The AndroidX builder uses JDK 21, Android platform 36, and build-tools 36.0.0.
It verifies the artifact hashes in `androidx-dependencies.json` and signs a
test-only APK under `target/android-tools-apk`. Android SDK boot classes and
lambda stubs constrain its Java compilation to the Android API.

Generated reports, hierarchy XML, captured images, and process logs live under
`target/android-tools-evidence`. [Evidence and integration decisions](EVIDENCE.md)
describe the observed limitations and required host safeguards.
