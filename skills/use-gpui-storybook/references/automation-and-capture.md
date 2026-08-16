# Automation and capture

Read this reference when enabling MCP, selecting routes, launching captures, or
diagnosing automation.

## Enable automation

Forward the facade feature:

```toml
[features]
mcp = ["gpui-storybook/mcp"]
```

Set `GPUI_STORYBOOK_MCP_STDIO=1` to serve MCP over stdio. Route tracing and
diagnostic logs to standard error.

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
private Sway executable. The launch-env tool emits this command on Linux and a
direct Cargo command elsewhere.

### Verify raw stdio in this repository

Use the explicit story example for a safe end-to-end check. Run that Cargo
command through the launcher:

```bash
GPUI_STORYBOOK_MCP_STDIO=1 \
GPUI_STORYBOOK_MCP_ALLOW_INTERACTION=1 \
gpui-storybook-launch -- cargo run -p gpui-storybook-example-story --features mcp
```

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

Set `GPUI_STORYBOOK_MCP_ALLOW_INTERACTION=1` only when the client should receive
generic in-process interaction tools. The value must be exactly `1`; otherwise
the tools are omitted. This capability can trigger any effect reachable from a
story action or input handler, so use an inert fixture or a safe backend.

The standard `Gallery::view` and `StoryWorkspace::view` constructors attach
the controller installed by `gpui_storybook::init`.

## MCP tools

- `storybook_list_stories`
- `storybook_get_story`
- `storybook_current_story`
- `storybook_open_story`
- `storybook_read_controls`
- `storybook_read_semantic_values`
- `storybook_set_control`
- `storybook_reset_control`
- `storybook_capture_current_story`
- `storybook_capture_launch_env`
- `storybook_list_actions` (interaction capability)
- `storybook_list_interaction_targets` (interaction capability)
- `storybook_run_steps` (interaction capability)

Use advertised typed fields. Width and height are optional only as a pair.
Control operations use tagged `ControlValue` objects shared with the UI:

```json
{
  "key": "disabled",
  "value": { "type": "boolean", "value": true }
}
```

Read controls after opening the intended route. Reset with a key for one value
or omit the key for all values. A capture request can include a `controls` map
so it applies serialized values immediately before rendering.

Wrap rendered application state with
`gpui_storybook::semantic_value(key, label, json_value, child)`, then call
`storybook_read_semantic_values` to receive the active route's values in stable
key order. The tool refreshes the route before reading and remains available
without the generic interaction capability. Use it for semantic postconditions;
use capture when pixels and layout are the evidence.

Capture requests also accept `responsive`, `mobile`, `tablet`, or `desktop` as
a `viewport`. Explicit paired width and height take precedence. The live
gallery or dock chrome remains mounted for layout, while the returned PNG is
cropped to the story region and excludes that chrome. The launch-env tool
accepts the same named presets when dimensions are omitted. Treat the returned
`pixel_width` and `pixel_height` as authoritative; viewport text rendered by a
story can describe its logical live-window bounds instead of the PNG size.

## Interaction batches

Prefer controls, then registered actions, semantic targets, keystrokes, and
finally story-relative pointer coordinates. Discover non-internal runtime
actions and their JSON argument schemas with `storybook_list_actions` after
every launch.

Wrap visible controls with
`gpui_storybook::interaction_target(key, label, child)`. After opening a route,
call `storybook_list_interaction_targets` to discover live route-relative
bounds, then use `{ "type": "click_target", "key": "..." }`. Keys must be
unique within one story or substory route.

`storybook_run_steps` accepts an optional route, controls, paired rendered-pixel
dimensions or viewport, a required non-empty step list, and an optional final
capture. Step types are `focus_next`, `focus_previous`, `blur`, `keystrokes`,
`text`, `dispatch_action`, `click_target`, `pointer_move`, `pointer_click`, `scroll`, and
`wait_frames`. Supplying a route focuses the selected story's focus handle
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
uses the same presentation with temporary storage. On Linux,
`storybook_capture_launch_env` returns an `env` map and a `command` array. Merge
every `env` entry into the child process environment before executing
`command`; on Linux the command invokes the installed launcher, which creates a
private Wayland runtime, waits for headless Sway, and then runs Cargo. It does
not inline the capture or MCP variables.

Captures exclude gallery or dock chrome. A substory route crops to its section.
Paired dimensions target the story region rather than collapsing the complete
window. Use returned pixel dimensions as the rendered source of truth.

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
