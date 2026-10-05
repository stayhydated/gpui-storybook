# Android automation experiment evidence

The `experiment/mobile-automation` worktree implements the reusable contracts,
embedded GPUI attachment, Android endpoint/example, computer MCP backend, and
capture scopes from the mobile automation plan. The coordinated public API
publication boundary is **0.8.0**; this experiment retains workspace version
0.7.1. The original plan remains unchanged.

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
Activity recreation. A separate default-interaction stdio session verified explicit
APK installation, fresh launch, stop-on-EOF, and owned forwarding cleanup.
Both renderer paths ran against the real endpoint.

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

`cargo machete --with-metadata` reports the same existing dependency findings
in this worktree and the original checkout: `es-fluent-build`, core `gtk`,
preferences `gpui-kit`, and test-crate `tokio`. The new owners introduce no
additional findings.

The GitHub mobile workflow is ready for remote execution; its hosted jobs require
a pushed branch or pull request. Local commands establish the evidence above.
The Wayland startup-capture regression belongs to `wayland-render-image` and
requires that branch's renderer patches. This worktree retains published GPUI.

iOS qualification covers target-neutral compilation. Simulator transport,
native-main-thread dispatch, retained surface lifecycle, and permitted capture
form its follow-up runtime work. The native harness qualifies the declared AOSP
IME/layout; other IMEs and application permission/effect flows need their own
native tests.

## Android library follow-up

The isolated [library qualification](tools/EVIDENCE.md) exercised both Rust ADB
clients, both Rust UI Automator clients, and AndroidX UI Automator 2.4.0 against
this real example. It records the shell exit-status limitation, native dropped
reply/replay measurements, and an independent instrumentation lifecycle proof.
