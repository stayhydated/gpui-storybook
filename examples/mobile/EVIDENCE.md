# Android automation stabilization evidence

The `experiment/mobile-automation` worktree implements the reusable contracts,
embedded GPUI attachment, Android endpoint/example, computer MCP backend, and
capture scopes from the mobile automation plan. The coordinated public API
publication boundary is **0.8.0**; this experiment retains workspace version
0.7.1. The original plan remains unchanged. The 2026-10-05 stabilization promotes
`adbutils-rs =0.1.0` into a bounded host transport and AndroidX UI Automator 2.4.0
into the maintained native qualification harness. The shared GPUI crate owns
the reusable device coordinator; the Android example supplies its JNI adapter.

## Qualified inputs

| Input | Value |
| --- | --- |
| Rust | Pinned 1.99.0 |
| GPUI / GPUI Kit | Published `gpui-pre =0.3.7`, `gpui-kit 0.7.0` |
| GPUI Mobile | `9075e3aa3eea812127f2c60ed66f0cd5798ff245` |
| Android SDK / build tools | Platform 36 / 36.0.0 |
| NDK / native API baseline | 27.1.12297006 / 31 |
| Java | OpenJDK 21, Java source/target 11 |
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
| Public and publication surfaces | Rustdocs, crate/facade/root READMEs, book, integration reference, examples, and catalog are aligned. The reusable mobile workflow gates the main release job. The coordinated API publication boundary is 0.8.0; this local experiment stays at 0.7.1. |
| Validation and isolation | All-feature Linux workspace tests, owning Clippy, Android Clippy/default build, both signed APK ABIs, neutral Android/iOS targets, public docs, book/LLM text/catalog, formatting, and package inventory pass. The local checkpoint stays on the experiment branch; master, Manefi, and the original plan are preserved, with no push. |

The final owner tests include five endpoint cases, seven host transport/capture
cases, the coordinator acknowledgment deadline case, and six embedded GPUI
cases. The complete workspace suite also passes. The ADB peers carry real bounded
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
