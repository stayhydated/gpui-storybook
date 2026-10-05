# Android library qualification

The 2026-10-05 local run exercised all five candidates on the disposable
`gpui_android_api36` emulator, serial `emulator-5580`, with SwiftShader Vulkan.
Inputs were Android API 36, x86_64, Pixel 7, 1080 × 2400 pixels, scale 2.625,
AOSP en-US LatinIME, Rust 1.99.0, and JDK 21. The example APK came from the
existing opted-in mobile build. These results qualify those inputs.

## Observed capabilities

| Candidate | Real-device result | Integration decision |
| --- | --- | --- |
| `adb_client` 3.2.3 | Installed/launched the APK; separated stdout/stderr; created/removed a dynamic forward; tapped GPUI; decoded PNG | Defer adoption until shell status and deadline handling are addressed |
| `adbutils-rs` 0.1.0 | Same ADB operations; preserved exit status; directly opened the bounded Storybook socket; decoded PNG | Preferred candidate for a narrow async transport adapter |
| `uiautomator` 1.0.2 | Bootstrapped its service; found/clicked native tabs; tapped a registered GPUI target; captured JPEG | Optional native backend with `Settings.max_retry = 1` |
| `uiautomator2-rs` 0.1.2 | Started/stopped its service; native selectors/input and JPEG capture passed | Prefer its public single-attempt JSON-RPC function for native mutations |
| AndroidX UI Automator 2.4.0 | Native selectors, GPUI touch, keyboard input, leased PNG capture, rotation, pause/resume passed | Preferred independent native qualification harness |

The ADB clients each installed the actual example APK. Both created a `tcp:0`
forward and removed the resulting local endpoint. Their forwarding methods
return unit; the experiment observed the allocated endpoint through ADB's
forward list. `adbutils-rs` also transported a real Storybook `list_targets`
request through `AdbDevice::create_connection`, with bounded framing and
request identity checked by the runner.

For the shell command below, both clients returned the expected stdout and
stderr. `adbutils-rs` returned status **7**; `adb_client` returned **None**.
The latter is recorded as a capability limitation even though its other probes
completed successfully.

```sh
printf probe-out; printf probe-err >&2; exit 7
```

All five native input paths incremented the rendered GPUI public count by
exactly one. The UI Automator paths selected Notes and Counter using native
text-prefix selectors. Their hierarchy XML exposed the native `SurfaceView`;
GPUI target positions came from Storybook's semantic registry and observed
surface scale/offsets.

All five captures independently decoded at **1080 × 2400**. The three PNGs
contained 378 distinct colors; each Rust UI Automator JPEG contained 508.
The AndroidX and Rust UI Automator captures were also inspected visually.
These full display observations include native shell/system regions.

## Dropped input replies

The relay forwards requests to the actual `u2.jar` service and discards one
reply after that service completes a native click. Its supervised upstream
recovers when the library stops the service. The oracle is the example's
rendered public count, independently read through the Storybook endpoint.
This is an injected transport failure against the real application.

| Client path | Count delta for one requested click | Returned outcome |
| --- | --- | --- |
| `uiautomator`, default three attempts | 2 | Success after replay |
| `uiautomator`, `max_retry = 1` | 1 | Transport error; outcome unknown |
| `uiautomator2`, `Server::jsonrpc_call` recovery | 2 | Success after replay |
| `uiautomator2::jsonrpc::jsonrpc_call` | 1 | Transport error; outcome unknown |

The first client's relay test uses its public custom endpoint constructor with
the default attempt count. The second uses a device-side ADB reverse endpoint
to the relay. Ordinary discovery probes separately exercised each library's
real service bootstrap. Mutation replay is disqualifying for Storybook's
input contract; the tested single-attempt paths preserve uncertainty.

## AndroidX lifecycle proof

The original custom instrumentation ran in `dev.storybook.tools`, independently
of the example process. The maintained harness now lives in `../native` and
runs in `dev.storybook.mobile.test`. It uses AndroidX `By`, `Until`, `UiObject2`, and `UiDevice`.
It selects native tabs, taps GPUI with observed target geometry, and commits
`a` plus a trailing space through the actual AOSP keyboard and InputConnection.
The keyboard layout is qualified separately from GPUI text insertion.

Its PNG capture acquires and finishes a Storybook capture ticket. Decoding
verifies display dimensions and 37 sampled colors within the GPUI surface.
Rotation increases the surface revision, changes the session, and retains
count/PID. Pause yields `no_live_host`; resume retains the count. Instrumentation
finishes with code -1 and `passed: true`, then the runner stops its test package.

The harness initially used a JDK file API outside the Android boot API and
treated transient route readiness as an assertion failure. Both harness issues
were corrected; the complete native proof then passed. D8 reports stripped
debug-local metadata in two upstream coroutine methods during APK generation.

## Host safeguards for adoption

Use `adbutils-rs` through selected low-level transport APIs, with an overall
deadline, bounded reads, explicit serial, and owned cleanup. Its convenience
installer can uninstall and retry after certain compatibility failures;
production installation policy belongs to the host. Its `forward_port` helper
can reuse a forward or reserve a free port before creating it. Prefer the
direct device socket for Storybook traffic, or implement verified allocation
and ownership explicitly.

Keep session validation, admission, leases, receipts, and capture provenance
in the Storybook owners. UI Automator selectors complement GPUI's typed
registry. Use single-attempt native input and report unknown completion after
a lost reply. Process cancellation bounds host waiting; admitted native input
still requires device-side ownership and completion handling.

Qualify native service process ownership and bounded output before embedding
a Rust wrapper in the host. The second wrapper's stop operation uses a broad
class-name `pkill`; the experiment confines this to its owned emulator. Its
low-level transport also needs host-owned response bounds and identity checks.
The first wrapper's custom endpoint mode relaxes response-ID checking, and its
published direct-call timeout is hard-coded independently of `http_timeout`.

## Reproduction and validation

Run the commands in [README.md](README.md). The Cargo lockfile fixes the Rust
versions; `../native/androidx-dependencies.json` fixes Maven artifacts and SHA-256 hashes.
Generated evidence stays under `target/android-tools-evidence`:

- `rust-report.json`: capabilities, counter deltas, relay events, retry outcomes.
- `androidx-report.json` and `androidx-instrumentation.txt`: native lifecycle proof.
- `*.xml`, `*.png`, `*-capture.jpg`, and process logs: observations and diagnostics.
- `decoded-captures.json`: independent local image decoding results.

The inner workspace built and passed clippy with warnings denied. Rust
formatting, Python compilation, APK signature verification, Markdown checks,
and Git whitespace checks passed. Production workspace tests and publication
builds retain the earlier mobile experiment's validation; this change adds an
isolated tool workspace and native test APK. Remote CI execution and additional
Android/device/IME inputs require their own qualification.

The upstream contracts are documented by
[adb_client](https://docs.rs/adb_client/3.2.3/adb_client/),
[adbutils-rs](https://docs.rs/adbutils-rs/0.1.0/adbutils/),
[uiautomator](https://docs.rs/uiautomator/1.0.2/uiautomator/),
[uiautomator2-rs](https://docs.rs/uiautomator2-rs/0.1.2/uiautomator2/), and
[AndroidX release notes](https://developer.android.com/jetpack/androidx/releases/test-uiautomator).
