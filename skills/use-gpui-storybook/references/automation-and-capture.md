# Automation and capture

## Enable automation

On Linux or macOS, forward the facade feature:

```toml
[features]
mcp = ["gpui-storybook/mcp"]
```

The `mcp` feature is unsupported on Windows and produces a compile-time error
there. Set `GPUI_STORYBOOK_MCP_STDIO=1` to serve MCP over stdio. Route tracing
and diagnostic logs to standard error.

For Linux Wayland window capture, copy the three `[patch.crates-io]` entries
from GPUI Storybook's
[`wayland-render-image` branch](https://github.com/stayhydated/gpui-storybook/blob/wayland-render-image/Cargo.toml)
into the application's workspace root. They pin `gpui-pre-linux`,
`gpui-pre-wgpu`, and `gpui-pre-platform` to the same Zed fork commit and retain
`gpui-pre =0.3.7` for compatibility with `gpui-kit`. Commit the resolved
`Cargo.lock`; Cargo patches are not inherited from dependencies. Verify the
pinned path on that branch with `just wayland-capture-test`. The `master` branch
uses published GPUI packages, including for portable headless capture.

On Linux, install Sway plus `libgl1-mesa-dri` and `mesa-vulkan-drivers`, then
install the reusable launcher and run stdio and startup-capture sessions
through it:

```bash
cargo install gpui-storybook-launch
GPUI_STORYBOOK_MCP_STDIO=1 \
gpui-storybook-launch -- cargo run -p my-app-storybook --features mcp
```

The launcher provides a compatibility seat, window management, bounded socket
readiness, and frame callbacks while retaining GPUI's normal Wayland backend.
It selects the headless backend and software Pixman renderer, inherits MCP
stdio, and stops Sway when the child exits. Set `GPUI_STORYBOOK_SWAY` for a
private Sway executable. The launch-env tool emits this launcher command on
Linux. On macOS, it emits Cargo directly and GPUI's native image renderer owns
capture.

### Verify raw stdio with the upstream example

From a checkout of `stayhydated/gpui-storybook`, use the explicit story example
for an end-to-end check. On Linux, run it through the launcher:

```bash
GPUI_STORYBOOK_MCP_STDIO=1 \
GPUI_STORYBOOK_MCP_ALLOW_INTERACTION=1 \
gpui-storybook-launch -- cargo run -p gpui-storybook-example-story --features mcp
```

On macOS, omit `gpui-storybook-launch --` and run Cargo directly.

The stable route
`gpui-storybook-example-story-InteractionStory` is an inert fixture with a
typed `prefix` control and the schema-backed
`interaction_story::SetAutomationStatus` action.

An MCP client performs the initialization sequence automatically. For a raw
JSON Lines smoke test, keep the process's standard input open and exchange one
JSON object per line in this order:

1. Send `initialize`:

   ```json
   {"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2026-07-28","capabilities":{},"clientInfo":{"name":"storybook-smoke","version":"1.0"}}}
   ```

2. Read the response with `id: 1`, then send the initialized notification:

   ```json
   {"jsonrpc":"2.0","method":"notifications/initialized"}
   ```

3. Discover the live tool schemas:

   ```json
   {"jsonrpc":"2.0","id":2,"method":"tools/list"}
   ```

4. Invoke a tool with `tools/call`:

   ```json
   {"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"storybook_list_stories","arguments":{}}}
   ```

Read the matching response ID before closing standard input. Use each entry's
advertised `inputSchema` when constructing later calls.
The first tool call waits up to 30 seconds for the gallery to publish
its story catalog and attach the live automation host.

Set `GPUI_STORYBOOK_MCP_ALLOW_INTERACTION=1` only when the client should receive
generic in-process interaction tools. The value must be exactly `1`; otherwise
the tools are omitted. This capability can trigger any effect reachable from a
story action or input handler, so use an inert fixture or a safe backend.

The standard `Gallery::view` constructor attaches
the controller installed by `gpui_storybook::init`.
Its `view_with_automation` variant carries the supplied controller into the
Scenarios workbench even when it is not installed as the default global.
Retain the MCP automation state across calls until the transport or
application host is explicitly stopped.

## Backend integration

Server and registry constructors accept `SharedAutomationBackend`
(`Arc<dyn AutomationBackend>`). Core's gallery controller implements the
interface and facade initialization connects it automatically. Custom hosts
use the portable `gpui-storybook-automation` records and validation and dispatch
through their existing application owner thread.

Advertise an immutable capability set before constructing MCP. Tool exposure
intersects those capabilities with the interaction opt-in. Validate every
requested interaction family, sizing, control mutation, and capture capability
before dispatching a complete batch. Retain host-side exclusive ownership until
submitted work settles; cancellation never authorizes replay. Scenarios create
fresh fixtures and ordinary interactions preserve state.

## Embedded GPUI and Android hosts

The Android integration is maintained through the main workspace and mobile
release checks. In the repository, `just mobile-build [abi]` builds the opted-in
APK, `just mobile-host <serial> [abi]` installs/launches it and serves MCP, and
`just mobile-test <serial>` qualifies the running example on an owned API 36
emulator. Build commands use `ANDROID_HOME` and `ANDROID_NDK_HOME`.
Android x86_64 has emulator runtime evidence; arm64 has signed build evidence.
The iOS lane checks target-neutral contracts.

Use `gpui-storybook-automation-gpui` to attach to an application-owned root and
window. Implement `EmbeddedRoot` for route selection, public revision, controls,
action scope, presentation, and fresh-fixture construction. Register the root
with `capture_story_view_with_scroll`, target/value wrappers, and a stable
catalog. Retain `GpuiHostAttachment`, dispatch on the existing GPUI owner thread,
and invalidate the attachment before surface replacement. Registries belong to
that app/window; repeated keys in other contexts stay independent.

Enable `device` for `DeviceCoordinator`; supply an opted-in endpoint, native
snapshot, and queue adapter. Retain the selection permit, check its surface and
admission immediately before native dispatch, and acknowledge applied or revoked
work. Distinguish rejected enqueueing from unknown delivery; a response deadline
retains ownership until acknowledgment or explicit host invalidation.

On native surface release, call `OperationGate::suspend` immediately. This closes
admission during the handoff to the GPUI owner. After invalidating the old
attachment, `DeviceCoordinator::surface_replaced()` advances an endpoint-owned
session generation and reopens admission atomically. Seed `DeviceEndpoint::listen`
with a process-unique string of 1–107 bytes; native revisions stay observation metadata.

The repository's `examples/embedded` shares its production `DemoRoot` with
`examples/mobile`. The Android example uses Jetpack Compose Counter/Notes tabs,
appearance, an independent native counter, one retained GPUI runtime, and an
opted-in loopback device endpoint. Build and run using
`examples/mobile/README.md`; its builder pins
SDK/build tools 36, NDK 27.1.12297006, API baseline 31, and the `mobile` profile.
GPUI Mobile revision `9075e3aa3eea812127f2c60ed66f0cd5798ff245` composes with the
published GPUI 0.3.7 stack. Keep minimal mobile features and system-font inputs
in qualification evidence.
The Gradle 8.13 wrapper pins AGP 8.13.2, Kotlin/Compose compiler 2.3.10,
Compose BOM 2025.12.01, and Activity Compose 1.11.0. Normal builds use checked-in
dependency locks and SHA-256 verification metadata; refresh them deliberately
with `build.py --write-gradle-locks` and review both artifacts.

Supply native action descriptors and distinct native semantic-value keys in
`NativeShellSnapshot`. Validate `NativeSelection::action()` before enqueueing
`HostAction::Invoke`; ordinary native UI and MCP commands share one state owner,
and acknowledgment follows its committed frame. The example advertises
`compose.increment` and `compose.reset` with empty object arguments:

```json
{"action":{"action":"invoke","name":"compose.increment","arguments":{}}}
```

Read `native.compose-counter` as `{"count":1}` through semantic-value discovery
or `storybook_read_value`. Compose counter state survives navigation and Activity
recreation independently of GPUI scenario fixtures. AndroidX selectors use
`By.res("storybook.compose.increment")`, `storybook.compose.reset`, and
`storybook.compose.count`; navigation uses `storybook.counter`, `storybook.notes`,
and `storybook.appearance`. Keep `testTagsAsResourceId` enabled on the Compose
subtree and verify visible text after native/MCP actions.

Run `gpui-storybook-mobile-host --serial DEVICE --allow-interaction` on Linux
or macOS to attach. Installation, launching the example, and stopping it on EOF
are explicit flags. EOF settles admitted captures and closes host connections; reconnection discovers the
new catalog without replay. Surface replacement changes the session and invalidates
old geometry, targets, capture tickets, and queued operations. Protocol v1 uses
closed, length-prefixed JSON bounded to 1 MiB; the device holds one mutation lease
through native acknowledgment, rendered readiness, and capture settlement.
Incoming frames have an overall 30-second deadline, responses have a five-second
write deadline, and endpoint drop closes sockets and joins transport threads.

Discover native geometry/orientation with `storybook_get_host`, native action
schemas with `storybook_list_host_actions`, and dispatch appearance with
`storybook_dispatch_host_action`. `storybook_capture_host` accepts `display` or
`gpui` plus a computer-owned PNG path. Display includes the native shell, visible
IME, and system UI; GPUI crops the observed surface after IME insets. Captures
return dimensions, request/session identity, route/surface revisions, geometry,
and `adb_compositor_observation` provenance. This is a separately validated
compositor observation. Explicit phone-sized desktop presets require
`StorySizing`; the Android example uses observed dimensions.

The computer uses pinned `adbutils-rs` primitives with host deadlines, bounded
wire/shell/PNG reads, preserved shell-v2 status, and direct device connections.
APK installation streams to an owned temporary path, installs once, and cleans
that path without automatic uninstall. Captures survive caller cancellation on
the attachment runtime; call `RemoteBackend::shutdown` after closing admission.
The install deadline includes local file validation/opening; APK inputs must be
regular files. PNGs replace their destination atomically after encoding and
ticket settlement. Inspect both `operation` and `cleanup` in a
`settlement_failed` result, retaining the primary mutation's replay prohibition.
Use `RUST_LOG=gpui_storybook_mobile_host=debug` for stderr request/session tracing.

Use the maintained `native/build.py` and `native/verify.py` AndroidX UI Automator
2.4.0 harness for native selectors, real touch/IME input, rotation, pause/resume,
and screenshot/hierarchy artifacts. Maven dependencies are hash-pinned and CI
records the declared API 36 Pixel 7/AOSP inputs. Use `verify.py` for the raw stdio MCP proof and `verify_native.py` for native
touch, AOSP en-US IME commits/insets, busy/disconnect/duplicate admission,
partial progress, pause/resume, and recreation. GPUI mouse/text steps and native
input are independently qualified. Preserve live state during navigation; only
scenario fixture hooks reset the selected surface. Native permission dialogs
and other application effects require their own native harness. iOS follow-up
must prove native-main-thread dispatch, simulator transport, lifecycle, and
permitted capture before advertising runtime support.

## MCP tools

- `storybook_list_stories`
- `storybook_get_story`
- `storybook_current_story`
- `storybook_open_story`
- `storybook_list_scenarios`
- `storybook_read_controls`
- `storybook_read_semantic_values`
- `storybook_read_value`
- `storybook_wait_for_value`
- `storybook_set_control`
- `storybook_reset_control`
- `storybook_capture_current_story`
- `storybook_capture_launch_env`
- `storybook_get_host` (native host discovery)
- `storybook_list_host_actions` (native host actions)
- `storybook_dispatch_host_action` (native host actions and interaction opt-in)
- `storybook_capture_host` (device display capture)
- `storybook_list_actions` (interaction capability)
- `storybook_list_interaction_targets` (interaction capability)
- `storybook_click_target` (interaction capability)
- `storybook_run_scenario` (interaction capability)
- `storybook_run_steps` (interaction capability)

Use advertised typed fields. Width and height are optional only as a pair.
Control operations use tagged `ControlValue` objects shared with the UI:

```json
{
  "control_key": "disabled",
  "value": { "type": "boolean", "value": true }
}
```

Read controls after opening the intended route. Reset with a `control_key` for
one value or omit it for all values. A capture request can include a `controls` map
so it applies serialized values immediately before rendering.

Import `gpui_storybook::StorybookElementExt as _`, wrap Serde-serializable
application state with `.storybook_value(&state)`, then use
`storybook_read_value` for one `value_key` or `storybook_wait_for_value` for a
bounded exact match on the complete value or an RFC 6901 JSON Pointer. The wait
performs fresh reads and never retries the preceding interaction. Use capture
when pixels and layout are the evidence. The implicit method uses the GPUI
element ID as its key and derives the label; use
`.storybook_value_as(key, label, &state)` for explicit metadata.

Capture requests also accept `responsive`, `mobile`, `tablet`, or `desktop` as
a `viewport`. Explicit paired width and height take precedence. The live
gallery chrome remains mounted for layout, while the returned PNG is
cropped to the story canvas and excludes that chrome. Live and startup captures
wait up to five seconds for the requested canvas dimensions to fit the visible
story pane; a layout that cannot fit returns `capture_unavailable`.
Changing the workbench viewport or background restores its preview sizing.
The launch-env tool accepts the same named presets when dimensions are omitted.
Treat the returned `pixel_width` and `pixel_height` as authoritative; viewport
text rendered by a
story can describe its logical live-window bounds instead of the PNG size.

## Interaction batches

Prefer controls, then registered actions, semantic targets, keystrokes, and
finally story-relative pointer coordinates. Discover non-internal runtime
actions and their JSON argument schemas with `storybook_list_actions` after
every launch.

Use `storybook_list_scenarios` to discover stable scenario keys on the current
or requested story. `storybook_run_scenario` recreates that story from
constructor defaults, rebinds controls and focus, and runs the declaration's
initial controls and presentation, named steps, exact semantic postconditions,
and optional capture. It shares the interaction operation guard and reports the
declaration with its structured result. Never resume or retry a partial run.

Wrap visible controls with `.storybook_target()`. After opening a route,
call `storybook_list_interaction_targets` to discover live route-relative
bounds, then call `storybook_click_target` with `target_key`, or use
`{ "type": "click_target", "target_key": "..." }` in a batch. Keys must be
unique within one story or substory route. The implicit method uses the GPUI
element ID and derives a readable label; use
`.storybook_target_as(key, label)` when those values need to differ.

`storybook_run_steps` accepts an optional `story_key`, controls, paired rendered-pixel
dimensions or viewport, a required non-empty step list, and an optional final
capture. Step types are `focus_next`, `focus_previous`, `blur`, `keystrokes`,
`text`, `dispatch_action`, `click_target`, `pointer_move`, `pointer_click`, `scroll`, and
`wait_frames`. Supplying a `story_key` focuses the selected story's focus handle
before the first step.

GPUI defers registered-action dispatch; the executor resumes with the next step
after that dispatch is delivered. Use `wait_frames` before a later action when
earlier widget input schedules its own next-frame state change.

Pointer points default to normalized `x`/`y` values in `0.0..=1.0`. The
`logical_pixels` space is relative to fresh active-route bounds. Points cannot
reach Storybook chrome or the global screen. A click dispatches move, down, and
up.

Limits are 64 steps, 64 binding strings per `keystrokes` step, 4 KiB across
UTF-8 text values and keystroke syntax, 120 waited frames, and one final
capture. Complete validation happens before input dispatch. The capture is the
first requested frame after the final step or explicit waits. Runtime failures
report `steps_dispatched`; do not retry automatically.

Capture, navigation, control mutations, and interaction share one exclusive
operation. Reads remain available while it runs and may observe intermediate
state. The interaction tool is destructive, non-idempotent, and open-world;
dispatch does not authorize or prove semantic success.

Direct integrations enable the capability with
`StorybookMcpServerOptions::default().with_interaction(true)` and
`server_with_options` or `register_tools_with_options`.

## Routes

Base routes use `{cargo-package-name}-{registered-type-name}`. Substory routes
append `/substory-key`.

Discover active base routes with `storybook_list_stories`. Discover a
substory suffix from its `Substory` enum, string section title, or custom
`StorySectionBase`.

## Startup capture

Launch an MCP-enabled binary with:

```bash
WGPU_CAPTURE_ROUTE=my-app-storybook-ButtonStory \
WGPU_CAPTURE_PATH=target/storybook-captures/button.png \
cargo run -p my-app-storybook --features mcp
```

Optional variables:

- `WGPU_CAPTURE_WIDTH` and `WGPU_CAPTURE_HEIGHT`: positive paired values;
- `WGPU_CAPTURE_FRAME`: positive one-based frame gate.

Capture startup disables persistence and forces light appearance, the
`Default Light` theme, and the typed fallback language. Stdio-only startup
uses the same presentation with temporary storage.
`storybook_capture_launch_env` returns an `env` map and a `command` array. Merge
every `env` entry into the child process environment before executing
`command`. On Linux, the command invokes the installed launcher, which creates
a private Wayland runtime, waits for headless Sway, and then runs Cargo. On
macOS, it invokes Cargo directly. It does not inline the capture or MCP
variables.

Captures exclude gallery chrome. A substory route crops to its section.
Paired dimensions target the story region rather than collapsing the complete
window. Use returned pixel dimensions as the rendered source of truth.

## Portable headless tests

Use `gpui-storybook-test` when a test or CI job should run a story without the
gallery shell, MCP process, or external compositor lifecycle. Keep the
story-bearing crate linked so inventory discovery retains its registrations.
Each request creates a fresh `HeadlessAppContext`, initializes the core runtime
and linked `story_init` hooks, constructs one registered story, applies typed
controls and presentation, and captures the rendered root or substory region.
On native targets, Storybook's Tokio bridge is installed before the core runtime
and hooks. Hooks can use `gpui_storybook_test::tokio_bridge::Tokio::spawn` or
`gpui_storybook_test::tokio_bridge::Tokio::handle`.
Matrix IDs encode each axis independently. Generated request IDs use a bounded
digest for serialized controls while structured reports retain the complete
typed values, and `output_dir` filenames preserve distinct case labels. Stories
without typed controls report an empty control snapshot; a non-empty control map
still fails. For an application-owned substory surface, install
`RunnerConfig::route_capture` and let the callback own route verification and
cropping without registering that route with the core capture helpers.

Pass consumer assets through `RunnerConfig::asset_source` and install any
application-owned globals with its initializer. Built-in light and dark theme
names apply GPUI Component's modes directly; any other named theme and every
named language matrix axis must have a case configurator. Allow the typed
configuration error to fail the case instead of recording a misleading label.

Build deterministic matrices from route, viewport, canvas background, theme,
language, and named control axes. Keep baseline policy explicit: use `Check`
in verification, and expose `Update` only through a deliberate acceptance
workflow. With the `performance` feature, require enough native GPUI profiler
samples before enforcing draw or dirty-to-present p95 and maximum budgets.

In the pinned `gpui-pre` 0.3.7 stack, `current_headless_renderer` returns a
Metal renderer on macOS and a Wgpu renderer on Linux, so runner captures run on
both. Windows gets `Ok(None)`; its DirectX renderer reaches `render_to_image`
only through a real window, outside the runner's headless context. Linux
Wayland application captures use the Git-patched Wgpu window renderer and
private Sway. The `mcp` feature supports Linux and macOS; the
`wayland-render-image` branch verifies application capture at desktop, tablet,
mobile, and custom sizes.
Treat renderer, fonts, assets, and CI hardware as part of the baseline or timing
environment; keep platform-specific accepted output where rasterization differs.

## Failure checks

- Route missing: inspect active filters and discover the base key again.
- No live host: await initialization and construct a standard view.
- Automation busy: wait before submitting another capture or mutation; work is
  not queued.
- Interaction tools missing: set the explicit capability before server
  construction and rediscover tools.
- Invalid action: rediscover actions for this launch and use its JSON schema.
- Invalid semantic target: list targets after opening the route and remove duplicate route-local keys.
- Invalid semantic value: render the wrapper in the active route and remove duplicate route-local keys.
- Partial interaction failure: inspect the dispatched count and do not retry.
- Invalid dimensions: provide positive width and height together.
- Corrupt stdio: move application logs to standard error.
- Startup timeout: confirm story registration linkage and open the route
  interactively.
