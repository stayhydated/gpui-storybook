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
| `appium-client` | 0.2.2 | Native selectors, touch, PNG capture through an owned Appium server |
| AndroidX UI Automator | 2.4.0 | Native selectors, input, lifecycle, PNG capture |

Run on an exclusively owned disposable API 36 Pixel 7 emulator with AOSP en-US
LatinIME. The probes install the example, restart it, and manage UI Automator
helper processes on that serial. `adb` must be on `PATH`. Build the opted-in
example with the [mobile example commands](../README.md).

```sh
cargo build --manifest-path examples/mobile/tools/Cargo.toml --locked
python3 examples/mobile/tools/run.py \
  --serial emulator-5580 --apk target/mobile-example/x86_64/storybook.apk
```

The AndroidX qualification is maintained under [the native harness](../native/README.md).
The root mobile host adopts bounded `adbutils-rs` connection primitives. The Rust
UI Automator probes retain their single-submission and replay-fault evidence.
The Appium probe uses an isolated loopback server and explicit device selection:

```sh
python3 examples/mobile/tools/appium.py --serial emulator-5580
```

Its report checks native selectors, lost-click replies, leased PNG capture,
awaited session closure, asynchronous Drop, and forwarding cleanup. Server inputs
and reproduction commands are recorded in the qualification evidence.

Generated reports, hierarchy XML, captured images, and process logs live under
`target/android-tools-evidence`. [Evidence and integration decisions](EVIDENCE.md)
describe the observed limitations and required host safeguards.
