# Embedded Android automation

Attach automation to your application's existing GPUI root when a native shell
owns navigation and the window lifecycle. The Counter/Notes example shares its
production views between a desktop application and an Android Activity with
Jetpack Compose tabs, an appearance control, and an independent native counter.
The Android Activity, input bridge, Compose shell, and native qualification
harness are written in Kotlin.

Android automation is a maintained integration. Its device endpoint, reusable
GPUI coordinator, computer MCP host, and AndroidX harness participate in the
workspace's test and release checks.

| Target | Qualification |
| --- | --- |
| Android x86_64 | Signed APK and MCP/native lifecycle, input, and capture on the declared API 36 Pixel 7 emulator |
| Android arm64-v8a | Signed APK build; qualify runtime behavior on the intended device and renderer |
| iOS arm64 | Target-neutral contract compilation; runtime adoption requires a simulator lifecycle, transport, and capture proof |

## Choose the integration boundary

| Crate | Application responsibility |
| --- | --- |
| `gpui-storybook-automation` | Shared backend, typed requests, capabilities, snapshots, and bounded wire envelopes |
| `gpui-storybook-automation-gpui` | Supply a root, window, route catalog, controls, action scope, fixtures, and capture provider |
| `gpui-storybook-mobile` | Opt in, poll admitted requests on the owning thread, retain their device operation lease |
| `gpui-storybook-mobile-host` | Select an ADB serial, own bounded direct connections and local PNG paths, serve MCP on the computer |

Implement `EmbeddedRoot` with application-owned route selection, public state
revision, controls, action scope, presentation, and fixture recreation. Create
`GpuiHostAttachment` after application initialization. Each app/window owns its
rendered registry. Invalidate the attachment before replacing a surface so
deferred work cannot target a replacement root through stale handles.

Use `validate_steps` before changing native state for a requested batch. After
the native owner acknowledges selection, apply the GPUI route through the normal
application path and await rendered readiness. `AttachedInteraction::builder()`
carries the operation lease through the shared frame executor. Ad-hoc navigation
preserves live state; scenarios recreate the selected surface's fixture.

Enable the GPUI automation crate's `device` feature and construct
`DeviceCoordinator` with the opted-in `DeviceEndpoint` and your native queue
adapter. Poll with `NativeShellSnapshot` after the ordinary application path
applies the native route. The adapter retains `NativeSelection`, checks its
permit and surface revision on the native lifecycle thread immediately before
dispatch, and acknowledges applied or revoked work. Report rejection before
enqueueing separately from unknown delivery. A native response deadline retains
its device lease until acknowledgment or explicit host invalidation.

Set the `NativeShellSnapshot` builder's `actions` to application-native descriptors
and `values` to native observations whose keys are distinct from GPUI keys.
`HostAction::Invoke` reaches your adapter through `NativeSelection::action()`.
Validate the name and arguments before enqueueing; use the same state owner as
ordinary UI callbacks and acknowledge the committed native frame.

On native surface release, call `OperationGate::suspend` immediately. This closes
admission during the handoff to the GPUI owner. After invalidating the old
attachment, call `DeviceCoordinator::surface_replaced()` to advance the
endpoint-owned session generation and reopen admission atomically. Pass a
process-unique seed of 1–107 bytes to `DeviceEndpoint::listen`; replacement
identity is independent of native revision values.

## Run the repository example

Use the pinned Rust toolchain, JDK 21, Python 3.11+, Android platform 36/build-tools
36.0.0, and NDK 27.1.12297006. The default build targets x86_64 with API 31 as
the native link baseline. Select `--abi arm64-v8a` for aarch64.
The checked-in Gradle 8.13 wrapper uses AGP 8.13.2, Kotlin/Compose compiler
2.3.10, Compose BOM 2025.12.01, and Activity Compose 1.11.0. Normal builds verify
the locked Android dependency closure against checked-in SHA-256 metadata.
When deliberately updating these dependencies, build with
`--write-gradle-locks` and review both the lockfile and verification metadata.

```sh
python3 examples/mobile/build.py --automation \
  --sdk "$ANDROID_HOME" --ndk "$ANDROID_NDK_HOME"
cargo build -p gpui-storybook-mobile-host --locked
```

The selected GPUI Mobile revision is
`9075e3aa3eea812127f2c60ed66f0cd5798ff245`, paired with `gpui-pre =0.3.7`
and GPUI Kit 0.7.0. The workspace `mobile` profile disables renderer debug labels
while retaining overflow checks. Assets come from GPUI Kit; Android supplies
Roboto and Noto system fonts. Renderer, fonts, emulator image, orientation,
density, and keyboard determine capture pixels and belong in recorded evidence.

Start an emulator or select an existing device explicitly. The APK supports API
31 or newer. The example endpoint requires both its build feature and launch
extra; the computer launcher supplies the extra with `--launch-example`.

```sh
cargo run -p gpui-storybook-mobile-host -- \
  --serial emulator-5580 --allow-interaction \
  --install target/mobile-example/x86_64/storybook.apk --launch-example
```

Use the process's stdin/stdout for MCP. Logs stay on stderr. Attaching to an
already running opted-in app needs only `--serial` and the desired interaction
opt-in. `--stop-on-eof` explicitly stops the owned example launched by this command;
startup failure also cleans it up after checking its PID. Host
EOF otherwise detaches after admitted captures settle.

Run the MCP, raw native, and AndroidX qualification paths on an owned API 36
Pixel 7 emulator with AOSP en-US LatinIME:

```sh
python3 examples/mobile/verify.py --serial emulator-5580
python3 examples/mobile/verify_native.py --serial emulator-5580
python3 examples/mobile/native/build.py --sdk "$ANDROID_HOME"
python3 examples/mobile/native/verify.py --serial emulator-5580
```

The workspace command equivalents are `just mobile-build`,
`just mobile-build arm64-v8a`, `just mobile-host emulator-5580`, and
`just mobile-test emulator-5580`. Set `ANDROID_HOME` and `ANDROID_NDK_HOME`
before building. `mobile-test` requires the already running, opted-in example
on an exclusively owned emulator and builds the AndroidX test APK before its
native qualification. `mobile-host` keeps MCP stdin/stdout attached until EOF.

The maintained AndroidX UI Automator 2.4.0 test APK uses hash-pinned Maven
artifacts, native selectors, actual touch/IME input, rotation, pause/resume,
hierarchy XML, and screenshot decoding. Its reports live under
`target/mobile-evidence/androidx` and its APK under `target/mobile-native-apk`.
Its Kotlin build shares the application's pinned Gradle/compiler inputs and
targets JVM 17. AndroidX artifacts retain their independent hash verification;
Gradle locks and SHA-256 metadata pin the Kotlin runtime. Refresh those locks
deliberately with `native/build.py --sdk "$ANDROID_HOME" --write-gradle-locks`.
The MCP proof records a raw JSON Lines MCP transcript and PNGs under
`target/mobile-evidence`, exercises both routes and fresh scenarios, and verifies
native/GPUI agreement and unsupported-request rejection.

## Discover host operations

`storybook_get_host` reports protocol/session identity, native and GPUI routes,
public state revision, observed display/surface geometry, scale, and capabilities.
`storybook_list_host_actions` exposes typed native actions. With interaction
enabled, select appearance through `storybook_dispatch_host_action`:

```json
{"action":{"action":"set_appearance","dark":true}}
```

Light/dark presentation in a step batch or scenario also waits for native
appearance acknowledgment before GPUI execution.

The example advertises `compose.increment` and `compose.reset`. Dispatch either
with an empty arguments object:

```json
{"action":{"action":"invoke","name":"compose.increment","arguments":{}}}
```

Read `native.compose-counter` through `storybook_read_semantic_values` or
`storybook_read_value`; its value is `{"count":1}` after one fresh increment.
Native clicks and MCP commands share Activity-owned Compose state. Navigation,
rotation, and pause/resume retain it; GPUI scenarios recreate their own fixtures.
The native harness verifies visible Compose text after action acknowledgment.

Compose enables `testTagsAsResourceId` for its subtree. Use AndroidX
`By.res("storybook.compose.increment")`, `By.res("storybook.compose.reset")`,
and `By.res("storybook.compose.count")` for the native counter. The selected
tabs and appearance control expose `storybook.counter`, `storybook.notes`, and
`storybook.appearance`. GPUI targets use Storybook's rendered registry.

The ordinary story tools discover routes, edit controls, read semantic values,
click targets, dispatch scoped actions, and run fresh scenarios. GPUI pointer
coordinates remain route-relative logical pixels or normalized fractions.
Android touch coordinates use physical display pixels, including the observed
surface origin and scale. Test native input separately through Android tooling.

## Capture and operation ownership

`storybook_capture_host` accepts `scope: "display"` or `scope: "gpui"` and a
computer-owned `output_path`. Display capture includes the native shell and
visible system/keyboard UI. GPUI capture crops the inset-aware SurfaceView bounds;
`storybook_capture_current_story` uses that crop for the selected root route.

The app retains an exclusive capture ticket while the computer obtains an ADB
PNG. The ticket equals the preparation request ID; completion can revoke a
still-pending frame wait if the preparation reply is lost. Completion validates session, route/state revision, and geometry before
writing the artifact. Results include dimensions, scope, request/session identity,
provider, observed host metadata, and local path. This is a compositor observation
following native and GPUI acknowledgment; its provenance is explicit.

Device evidence uses observed dimensions. Desktop sizing, control application
inside capture, and capture inside an interaction batch require their own
advertised capabilities. Unsupported requests fail before dispatch.

Frames contain a big-endian length and bounded JSON payload of at most 1 MiB.
The device gives each incoming prefix and payload one 30-second deadline and
each response a five-second write deadline. Dropping its endpoint suspends
admission, closes sockets, and joins transport threads.
Protocol/session mismatch rejects admission. The device bounds its request queue
and connections and admits one mutation, scenario, or capture at a time. Reads
may observe intermediate public state. Interaction limits remain 64 steps,
4 KiB text, and 120 explicit frame waits.

The computer wraps selected `adbutils-rs =0.1.0` connection primitives in overall
deadlines and byte budgets: 1 MiB wire/shell responses, 64 MiB encoded/decoded
PNGs, and streamed APKs of at most 512 MiB. Installation runs once and removes
its unique device temporary file; package-manager failures preserve the package.
The 120-second install deadline includes local input validation and opening.
APKs must be regular, nonempty files; special files fail before upload.
The transport selects a loopback ADB server, exposes shell-v2 exit status, and
uses direct device sockets. ADB/platform tools provide the local server.

Canceling a client response leaves submitted work under its device lease until
settlement. Capture work survives caller cancellation on the attachment runtime,
settles its ticket after provider/decoder failure, and validates before writing.
Library users keep that runtime alive and call `RemoteBackend::shutdown` after
closing admission. A transport failure after submission reports an unknown outcome;
rediscover state without replaying the mutation. Activity recreation replaces
surface identity and invalidates old attachments while the GPUI runtime remains
retained. Establish a fresh connection and rediscover routes/targets.

PNG publication atomically replaces the destination after encoding and ticket
settlement, preserving an existing artifact when encoding fails. This guarantee
covers readers observing complete files; power-loss durability is a separate
filesystem concern. A `settlement_failed` error contains `operation` and `cleanup`
typed errors. Inspect both; an `outcome_unknown` primary result still forbids
mutation replay. The host emits cleanup diagnostics to stderr and accepts
`RUST_LOG=gpui_storybook_mobile_host=debug` for request/session tracing.

iOS qualification follows a simulator proof using platform-owned lifecycle,
transport, and capture. The neutral contract's iOS build is one prerequisite.

## Qualification inputs

The mobile workflow checks neutral contracts on Android arm64/x86_64 and iOS
arm64. It builds signed APKs for both Android ABIs using JDK 21, SDK/platform and
build tools 36, NDK 27.1.12297006, and the API 31 linker baseline. Its x86_64 lane
uses a Pixel 7 API 36 default system image, SwiftShader Vulkan, the `mobile`
profile, and the image's Roboto/Noto fonts. Artifacts record image fingerprint,
emulator version, renderer logs, protocol transcripts, and scoped PNGs.

The native proof assumes the image's AOSP en-US LatinIME layout and a disposable
emulator. It taps the actual GPUI surface and soft keyboard using observed scale
and insets, independently of GPUI mouse/text steps. It checks native tab/route
agreement, commit/inset behavior, exclusive operation ownership after disconnect,
duplicate admission, partial progress, capture invalidation, pause/resume, and
Activity recreation. Other IMEs and application permission/effect flows require
their own native tests. Keep visual acceptance scoped to recorded renderer and
font inputs; a PNG's dimensions alone establish geometry.
