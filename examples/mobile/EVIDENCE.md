# Android automation stabilization evidence

The maintained `mobile-automation` branch implements the reusable contracts,
embedded GPUI attachment, Android endpoint/example, computer MCP backend, and
capture scopes from the mobile automation plan. The coordinated public API
publication boundary is **0.8.0**; the workspace retains version
0.7.1. The original plan remains unchanged. The 2026-10-05 stabilization promotes
`adbutils-rs =0.1.0` into a bounded host transport and AndroidX UI Automator 2.4.0
into the maintained native qualification harness. The shared GPUI crate owns
the reusable device coordinator; the Android example supplies its JNI adapter.

## Maintained Android support

Promotion on 2026-10-05 retains version 0.7.1 and the existing release gate.
The main CI workflow calls `mobile.yml` and requires its result before release.
CI runs on pushes to master and pull requests targeting master; publication
remains scoped to the master release job.
The crate/facade/root descriptions, book, integration skill, public catalog,
and contributor ownership/validation guidance describe the maintained Android
surface. The workspace commands are `mobile-build`, `mobile-host`, and
`mobile-test`.

The isolated probe package is named `gpui-storybook-android-tools`; its Python
runners select that binary. Candidate UI Automator/Appium dependencies remain
in its unpublished workspace with their recorded lifecycle and distribution
limits. Android x86_64 has emulator runtime evidence, arm64 has signed APK build
evidence, and iOS has neutral contract compilation evidence.

The branch was renamed from `experiment/mobile-automation` to
`mobile-automation`. The original plan and master checkout remain unchanged;
the promotion is local and nothing is published or pushed.

Promotion validation passed: `just mobile-build`, installation/launch and raw
MCP discovery through `just mobile-host emulator-5580`, and the complete
`just mobile-test emulator-5580` recipe (MCP, explicit Android Cargo test,
native lifecycle, and AndroidX). The host returned cleanly on EOF with unchanged
forwarding. The tools workspace built and passed Clippy under its new name.
Book, LLM text, and catalog builds, workflow lint, scoped formatting/Markdown,
Python compilation, and package inventory passed. Logs and the host transcript
are retained under `target/mobile-evidence/promotion-*`.

The first package-inventory attempt required `--allow-dirty` for the edited
manifest; the passing check used that flag without publishing. The first host
attempt found the demo emulator closed. Starting the owned qualification
emulator allowed the recipe checks to pass. Existing Java/D8 diagnostics remain
recorded below. No public API or workspace version changed during promotion.

## Jetpack Compose integration

The 2026-10-05 example adds a real Compose shell alongside the retained GPUI
SurfaceView. Compose owns tabs, appearance, and an independent counter. Native
clicks and MCP `compose.increment`/`compose.reset` commands share Activity-owned
state. `native.compose-counter` exposes that state through semantic discovery,
single-value reads, and bounded waits. Invalid names, non-object arguments, and
unexpected fields fail before mutation. GPUI scenarios preserve the native count.

The reusable wire contract adds `HostAction::Invoke`; native snapshots advertise
action schemas and distinct native value keys. The native adapter validates
arguments before enqueueing, checks its permit/surface before applying work, and
acknowledges the committed Compose frame. The real socket/GPUI regression verifies
rejected work stays out of the queue, disconnect retains ownership without replay,
acknowledgment releases it, and surface replacement revokes queued permits.

The AndroidX proof clicks `storybook.compose.increment`, observes visible count
1, dispatches the advertised increment to visible count 2, resets to 0 with a disabled reset
control, then retains count 2 through rotation and pause/resume. Its hierarchy
uses Compose resource-ID test tags. It independently verifies GPUI input and
state, IME commits, capture dimensions/content, and a retained process. The
captured display visibly contains both counters; the GPUI crop excludes Compose.

The pinned Gradle wrapper builds the Kotlin/Compose app around the selected Rust
ABI. The resolved dependency lock and SHA-256 metadata are checked in; normal
builds verify them. Signed x86_64 and arm64-v8a automation APKs and a default-feature
arm64 APK built successfully. The final runtime qualification uses the owned
`gpui_android_api36` emulator with SwiftShader and the declared API 36 inputs.

The full `just mobile-test emulator-5580` recipe passed. Additional MCP proof
covers single-value native reads and bounded native waits; launch/EOF and host
lifecycle proofs pass independently. The owning all-feature Rust suite passes
101 tests with its Android case ignored locally; that case passes explicitly
on the emulator. Linux and
Android Clippy with warnings denied, public Rustdocs, book, LLM text, catalog,
and scoped format/Markdown checks pass. Logs and transcripts use
`target/mobile-evidence/compose-*`; the native report and screenshot remain in
`target/mobile-evidence/androidx`.

Qualification exposed two IME issues: a suggestion strip changes keyboard
height, and requesting SurfaceView focus on an already focused text input
displaces its InputConnection. Key rows now use the observed keyboard bottom
and qualified density offsets; active input retains focus. Both native input
proofs pass after the corrections. A single-value harness assertion initially
read the wrong result field; it now checks the advertised `semantic_value`.
Existing native harness D8 diagnostics remain recorded. Arm64 runtime, other
keyboard layouts, and hosted platform jobs retain their qualification scope.

## Qualified inputs

| Input | Value |
| --- | --- |
| Rust | Pinned 1.99.0 |
| GPUI / GPUI Kit | Published `gpui-pre =0.3.7`, `gpui-kit 0.7.0` |
| GPUI Mobile | `9075e3aa3eea812127f2c60ed66f0cd5798ff245` |
| Android SDK / build tools | Platform 36 / 36.0.0 |
| NDK / native API baseline | 27.1.12297006 / 31 |
| Java | OpenJDK 21; Compose app JVM 17, independent native harness source/target 11 |
| Android app build | Gradle 8.13, AGP 8.13.2, Kotlin/Compose compiler 2.3.10 |
| Compose | BOM 2025.12.01, Activity Compose 1.11.0; locked dependencies with SHA-256 verification |
| APK ABIs | x86_64 and arm64-v8a, signed and verified |
| Runtime emulator | 37.1.11, API 36 default x86_64, Pixel 7 |
| Display | 1080 × 2400 physical pixels, scale 2.625 |
| Native input | AOSP en-US LatinIME, portrait layout |
| Renderers | NVIDIA GeForce RTX 5070 Ti Vulkan and SwiftShader Device (Subzero) Vulkan |
| Build profile | `mobile`: dev optimization, debug assertions disabled, overflow checks enabled |
| System fonts | Image-owned Roboto, Roboto Flex, and Noto families; hashes recorded with artifacts |

System-image fingerprint:

```text
Android/sdk_phone64_x86_64/emu64x:16/BE2A.250530.026.D1/13818094:userdebug/test-keys
```

The mobile profile avoids a Vulkan emulator crash during debug object labeling.
GLES-only rendering failed its surface configuration check in this environment.
SwiftShader disables subpixel text antialiasing; visual acceptance therefore
belongs to its recorded renderer/font inputs. The inert Latin-text example
uses loaded system fonts; emoji fallback requires separate qualification.

## Observable acceptance

The real raw stdio MCP proof discovers both registered routes, checks native
and GPUI agreement, clicks the rendered Counter target, dispatches its registered
action, reads public values, waits for an exact structured state, changes native
appearance, and runs both fresh scenarios repeatedly. Navigation preserves the
other surface's state. Device resizing and malformed steps fail before dispatch.

Both scopes decode as valid PNGs with matching observed dimensions and rendered
text/control variation. Display captures show the native title, selected tab,
GPUI content, system bars, and the visible Notes keyboard. GPUI captures exclude
those shell/IME regions and match the corresponding display crop. The PNGs were
also inspected visually. The observation reports request/session identity,
route/surface revisions, orientation, geometry, and
`adb_compositor_observation` provenance.

The native proof separately taps the actual SurfaceView using observed scale
and offsets, selects native tabs, and commits the letter `a` and a trailing space through the AOSP soft keyboard
and InputConnection. It verifies the IME inset boundary, device-owned busy
admission, disconnected work completing once, duplicate-ID rejection, one
reported dispatched step on partial failure, stale capture rejection, retained
state/process through pause/resume, and new surface/session identity after
Activity recreation. A separate default-interaction stdio session verifies explicit streamed APK
installation, fresh launch, stop-on-EOF, and unchanged ADB forwarding inventory.
The production host uses direct device sockets.
Both renderer paths ran against the real endpoint during the initial experiment.
The final stabilization run uses SwiftShader. Its typed device-protocol
presentation proof samples `[255, 255, 255, 255]` in light mode and
`[10, 10, 10, 255]` in dark mode, then restores the original light sample.
Native appearance acknowledgment precedes GPUI execution even with the current
route retained. MCP calls follow the advertised schema.

Local transcripts, environment metadata, logs, and PNGs are retained under
`target/mobile-evidence`; the software renderer's artifacts are in its
`swiftshader` subdirectory. Regenerate them with the two verification scripts.
Generated evidence is independent of accepted visual baselines.

## Repository validation

- Neutral contracts, shared GPUI execution/regions, embedded production views,
  real loopback admission, core/gallery, MCP, and portable-runner tests passed.
- Both original registration examples passed with all features.
- Neutral contracts/device endpoint compiled for Android arm64/x86_64 and iOS arm64.
- Actual opted-in Android compositions built signed APKs for both ABIs;
  the default-feature Android composition also compiled.
- Focused Linux clippy and Android clippy passed with warnings denied.
- Workspace public Rustdocs, mdBook, LLM text, and the web catalog built.
- Markdown, Rust/TOML formatting, workflow actionlint, package-content listing,
  and Git whitespace checks passed.
- The MCP normal dependency graph excludes GPUI, GTK, desktop core, and preferences.

`cargo machete --with-metadata` remains nonzero for existing findings:
`es-fluent-build` in core and both registration examples, core `gtk`, preferences
`gpui-kit`, test-crate `tokio`, and `serde` in the isolated library experiment.
The stabilization's production owners introduce no additional findings.

The GitHub mobile workflow is ready for remote execution; its hosted jobs require
a pushed branch or pull request. Local commands establish the evidence above.
The Wayland startup-capture regression belongs to `wayland-render-image` and
requires that branch's renderer patches. This worktree retains published GPUI.

iOS qualification covers target-neutral compilation. Simulator transport,
native-main-thread dispatch, retained surface lifecycle, and permitted capture
form its follow-up runtime work. The native harness qualifies the declared AOSP
IME/layout; other IMEs and application permission/effect flows need their own
native tests.

## Candidate selection

The isolated [library qualification](tools/EVIDENCE.md) exercised both Rust ADB
clients, both Rust UI Automator clients, and AndroidX UI Automator 2.4.0 against
this real example. It records the shell exit-status limitation, native dropped
reply/replay measurements, and an independent instrumentation lifecycle proof.

The Rust UI Automator wrappers remain in the isolated qualification workspace.
Their default recovery paths replayed a completed click after its reply was
lost; single-attempt calls preserved uncertainty. Their native service ownership
and transport limits need their own integration work before production adoption.
AndroidX supplies independent native input qualification while Storybook retains
session identity, receipts, mutation ownership, and capture provenance.

## Stabilization audit

| Requirement | Owning implementation and proof |
| --- | --- |
| Bounded ADB adoption | `mobile-host/src/adb.rs` wraps selected SDK primitives with explicit serial/loopback server, deadlines, bounded shell-v2/wire/PNG reads, streamed installation, and owned temporary-file cleanup. Real ADB protocol peers verify exit 7, cancellation, failed installation, lost replies, and exactly one delivered mutation. The emulator verifies shell status and package preservation after invalid APK installation. |
| Maintained native harness | `examples/mobile/native` owns the hash-pinned AndroidX APK and runner. Native selectors, actual GPUI touch, AOSP IME input, leased screenshot, rotation, and pause/resume are exercised on the declared emulator. CI builds it and preserves its artifacts. |
| Reusable coordinator | `automation-gpui::device::DeviceCoordinator<Root>` owns native preflight/acknowledgment, rendered waits, capture tickets, and settlement. The example supplies native snapshots and a JNI queue adapter. Embedded GPUI tests exercise the public coordinator and deferred admission revocation. |
| Atomic device ownership | `mobile` serializes session, duplicate receipts, and admission under one mutex. Native surface release suspends admission immediately; owner-thread replacement installs a new session and reopens it atomically. Old lease drops cannot release newer work. Owner regressions and the native lifecycle proof verify these boundaries. |
| Cancellation and cleanup | Unknown native completion retains its lease until acknowledgment or invalidation. Host capture/install tasks survive canceled response futures on their owning runtime; shutdown drains them. Finish revokes a pending capture by its preparation ID. Real peers exercise lost preparation replies and provider/decoder failures. Emulator proofs cover successful install/launch/EOF, startup failure cleanup, and preservation of an externally restarted PID. |
| Public and publication surfaces | Rustdocs, crate/facade/root READMEs, book, integration reference, examples, and catalog are aligned. The reusable mobile workflow gates the main release job. The coordinated API publication boundary is 0.8.0; the workspace stays at 0.7.1. |
| Validation and isolation | All-feature Linux workspace tests, owning Clippy, Android Clippy/default build, both signed APK ABIs, neutral Android/iOS targets, public docs, book/LLM text/catalog, formatting, and package inventory pass. Local checkpoints are retained on `mobile-automation`; master, Manefi, and the original plan are preserved, with no push. |

The owner tests include eleven endpoint/state cases, thirteen host
transport/capture/task cases, eighteen neutral contract cases, the coordinator
acknowledgment deadline case, and six embedded GPUI cases. The complete Linux
workspace suite passes with 323 tests and five platform/environment-gated cases
ignored; the explicit Android shell/install test also passes on the emulator. The ADB peers carry real bounded
wire traffic to a `DeviceEndpoint`; application counters and decoded artifacts
are independent outcome oracles.

Native invalidation can reject a read after rendered discovery. The AndroidX
harness rediscovery repeats only `get_host`/`read_values` observations within a
deadline and record their count; they never repeat touch, keyboard, installation,
or route mutation. The final presentation regression belongs to the typed device
protocol because the MCP step schema has no presentation input field.

## Remaining qualification scope

Local validation ran on Linux and the declared Android emulator. The macOS and
Windows matrix and hosted GitHub jobs await remote execution; nothing was pushed.
Arm64 Android qualification is a signed build, rather than a physical-device run.
iOS remains neutral compilation until its simulator/native lifecycle, transport,
and capture proof is implemented. The Wayland fork regression remains scoped to
its dedicated branch. Java emits its existing source-11/deprecated-API warning;
D8 strips invalid local debug metadata from two upstream coroutine methods.
Neither diagnostic prevented APK signature verification or native execution.

Cleanup operations use bounded connections to the selected device. If that
device becomes unreachable, remote cleanup can fail and reports its error; the
host never resolves an unknown outcome by replaying the mutation. Library users
keep the attachment/first-install runtime alive through `shutdown`.

## Further stabilization and crate adoption

The follow-up audit's five findings are resolved in their owning crates. The
2026-10-05 verification used fresh signed x86_64 and arm64-v8a APKs and the owned
`gpui_android_api36` emulator. Evidence is retained in
`target/mobile-evidence/further-stabilization-*`.

| Requirement | Implementation and observable proof |
| --- | --- |
| Overall frame deadline and endpoint shutdown | A deadline-aware stream bounds the incoming prefix and body together, and response writes separately. Endpoint drop suspends admission, shuts down registered sockets, and joins transport threads. Real socket regressions send periodic incomplete prefixes/bodies, then verify closure before admission; drop closes incomplete reads and pending replies, revokes permits, and leaves an empty connection registry. |
| APK special-file rejection and complete install deadline | Input metadata is checked before open; Unix opens use `O_NONBLOCK`, and opened-handle metadata is checked again. Validation/opening, upload, and installation share the install deadline. A real FIFO with a 50 ms install budget returns an input error without contacting ADB; owned shutdown settles without a writer. Real device testing preserves shell exit 7, the installed package, and temporary-file inventory after invalid APK installation. |
| Fresh session identity | The endpoint generates a monotonically increasing suffix from a process-unique seed. Replacement clears receipts and revokes leases atomically, independently of native surface revision metadata. Property sequences verify distinct sessions and retained receipts; the public coordinator regression replaces twice against unchanged native observations and rejects old requests each time. Real native/AndroidX rotation observes a new session and retained application state. |
| Secondary cleanup outcomes | `SettlementFailed` retains typed `operation` and `cleanup` errors and preserves the primary source. Real protocol peers fail installation and removal together, and fail PNG decoding and capture completion together. Both outcomes reach the caller; the MCP regression verifies both nested machine-readable errors, including an unknown request ID. |
| Atomic PNG publication | `tempfile::NamedTempFile` encodes beside the destination and persists only after success. Regressions preserve prior bytes after partial encoding, preserve a directory on failed replacement, reclaim temporary files, and decode a successfully replaced PNG. Real MCP capture independently decodes display/GPUI dimensions and content and verifies their crop relationship. |
| Owned host task tracking | `tokio-util::TaskTracker` replaces manual counters/notifications, with a mutex admission guard serializing registration and closure. Tests prove permanent closure and drainage of active work. Canceled install/capture callers leave their owned cleanup running until shutdown returns; real EOF verifies install, launch, detach/stop, and forwarding settlement. |
| Structured tracing | `tracing` spans carry request/session identity and close timing; combined failures emit both errors. The CLI subscriber writes to stderr. The real raw MCP EOF proof runs with debug tracing and parses its stdout successfully; its log contains request IDs, elapsed install timing, and span close timing. |
| Property and concurrency coverage | Three bounded `proptest` properties cover fragmented wire sequences, every generated request truncation, and session/receipt sequences. Two Loom models run the production gate transition methods under Loom's mutex, exploring replacement/old-release and suspension/admission orderings. Real sockets and GPUI/native tests cover their surrounding boundaries. |
| Additional Android crate qualification | The unpublished tools workspace pins `appium-client =0.2.2`. Real native selectors, one-click public state, a dropped completed-click reply, a leased PNG, explicit session closure, asynchronous Drop, and clean forwarding pass with the isolated Appium server. The report and distribution/lifecycle limits are in `tools/EVIDENCE.md`. |

Validation commands completed successfully:

```sh
cargo test --workspace --all-features --locked
cargo test -p gpui-storybook-example-embedded --all-features --locked
cargo clippy -p gpui-storybook-automation -p gpui-storybook-mobile \
  -p gpui-storybook-mobile-host -p gpui-storybook-mcp \
  -p gpui-storybook-automation-gpui -p gpui-storybook-example-embedded \
  --all-targets --all-features --locked -- -D warnings
cargo clippy -p gpui-storybook-mobile-host --all-targets --all-features --locked -- -D warnings
cargo doc --workspace --all-features --no-deps --locked
cargo check -p gpui-storybook-automation -p gpui-storybook-mobile \
  --target aarch64-apple-ios --locked
MDBOOK_BUILD__CREATE_MISSING=false cargo xtask build book
MDBOOK_BUILD__CREATE_MISSING=false cargo xtask build llms-txt
cargo xtask build web
cargo build --manifest-path examples/mobile/tools/Cargo.toml --locked
cargo clippy --manifest-path examples/mobile/tools/Cargo.toml --all-targets --locked -- -D warnings
```

Both Android builds used `examples/mobile/build.py --automation --profile mobile`
with the declared SDK/NDK and each ABI. Runtime verification used `verify.py`,
`verify_native.py`, `native/verify.py`, and `tools/appium.py` with explicit serial
`emulator-5580`. The explicit Android Cargo test ran with
`STORYBOOK_ANDROID_SERIAL=emulator-5580`. The EOF proof used debug tracing,
`--lifecycle-only`, and the new x86_64 APK. The Appium PNG was inspected visually
and showed native tabs, the GPUI counter at two, and its Increment control.

Initial authoring checks caught the new MCP enum match arm, an incorrect test
helper name/status assertion, the Appium selector's borrowed-string requirement,
and incorrect route names in its runner. These were corrected before the passing
runs. The changed TOML files were formatted after their initial format check.
The final Rust/TOML/Markdown and Python checks pass.

The TaskTracker guard and Loom models cover the stated lifecycle transitions;
network, renderer, and JNI boundaries have their own integration evidence.
Atomic replacement guarantees complete-file observation; durability across power
loss requires a separate storage policy. Appium qualification covers the named
Android inputs and injected reply failure. Hosted macOS/Windows/mobile jobs and
iOS runtime qualification retain the limits documented above. The experiment
kept the coordinated 0.8.0 publication boundary and workspace version 0.7.1.
The owned Appium server and emulator were stopped, and forwarding was clean.
EOF stopped the owned example; master, Manefi, and the original plan were preserved.
