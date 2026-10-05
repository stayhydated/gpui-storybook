# Android embedded automation example

A native Android Activity owns Counter/Notes tabs, appearance, system/IME insets,
and a `SurfaceView`. One retained GPUI Mobile runtime renders the production
views from `examples/embedded`. Automation requires the Cargo `automation`
feature and the `storybook_automation=true` launch extra.

The workspace pins GPUI Mobile revision
`9075e3aa3eea812127f2c60ed66f0cd5798ff245` with published `gpui-pre =0.3.7`
and `gpui-kit 0.7.0`. Platform service features stay explicitly selected.

```sh
python3 examples/mobile/build.py --automation \
  --sdk "$ANDROID_HOME" --ndk "$ANDROID_NDK_HOME"
cargo build -p gpui-storybook-mobile-host --locked
cargo run -p gpui-storybook-mobile-host -- \
  --serial emulator-5580 --allow-interaction \
  --install target/mobile-example/x86_64/storybook.apk --launch-example
```

Build inputs: Rust 1.99.0, JDK 21, Python 3, Android platform 36/build-tools
36.0.0, NDK 27.1.12297006, and Android API 31 or newer. `--abi arm64-v8a`
selects aarch64; the default is x86_64. The `mobile` Cargo profile disables
Wgpu debug labels and preserves overflow checking. APK signing uses a generated
example key under `target/mobile-example`.

The reusable `DeviceCoordinator<DemoRoot>` owns device preflight, native
acknowledgment, frames, and captures. The Android adapter queues the selection
permit and checks it with the surface revision before native dispatch. Timeouts
retain ownership until acknowledgment or surface invalidation.

The device endpoint binds IPv4 loopback at port 28437. The computer selects an
ADB serial explicitly, opens direct device connections, and chooses PNG paths. Native route
acknowledgment and rendered GPUI geometry precede success. Recreated surfaces
receive a new session; attachment requires rediscovery.
Session generations belong to the endpoint. Incomplete frames expire within
30 seconds; endpoint drop closes sockets and joins its transport threads.
The host publishes PNGs atomically and retains operation and cleanup failures
together, including after caller cancellation.

```sh
python3 examples/mobile/verify.py --serial emulator-5580
python3 examples/mobile/verify_native.py --serial emulator-5580
python3 examples/mobile/verify.py --serial emulator-5580 --lifecycle-only
```

This raw stdio proof records its transcript and PNGs under `target/mobile-evidence`.
It checks native/GPUI agreement, clicks, fresh scenarios, preserved navigation
state, appearance, capture scopes, and preflight rejection. Full display capture
includes visible keyboard/system UI; the GPUI crop uses the inset-aware native
surface bounds. Capture metadata identifies an ADB compositor observation.

The InputConnection bridge is adapted under Apache-2.0 from GPUI Mobile's
`GpuiInputActivity.java` at the pinned revision. GPUI text insertion and native
IME input have separate verification paths.

`verify_native.py` qualifies a portrait Pixel 7 API 36 AOSP en-US LatinIME
layout using observed surface/display geometry. It taps native tabs, the GPUI
surface, and soft keyboard; verifies busy/disconnect/duplicate admission and
partial progress; pauses/resumes the app; and rotates it to recreate the
Activity. It restores portrait rotation and releases its own forwarding. Run it
on a disposable emulator with those inputs. Other IMEs, permissions, and native
application effects need separate native qualification.

The `mobile automation` workflow checks neutral Android/iOS contracts, builds
both Android ABIs, and runs the x86_64 MCP/native proofs with SwiftShader Vulkan.
Its API 36 system image supplies Roboto/Noto fonts; artifact logs record the
resolved system-image fingerprint, renderer, and emulator version. Visual
baseline acceptance remains deliberate. [Qualification evidence](EVIDENCE.md)
records the local experiment and its limits.

The maintained [AndroidX native harness](native/README.md) uses hash-pinned
UI Automator 2.4.0 artifacts for native selectors, actual touch/IME input,
rotation, pause/resume, hierarchy XML, and screenshot decoding. It runs in CI.
The isolated [Android library probes](tools/README.md) retain the comparative
Rust ADB/UI Automator qualification and replay-fault evidence.
