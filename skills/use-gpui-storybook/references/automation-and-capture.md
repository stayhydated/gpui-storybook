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
- `storybook_set_control`
- `storybook_reset_control`
- `storybook_capture_current_story`
- `storybook_capture_launch_env`
- `storybook_list_actions` (interaction capability)
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

Capture requests also accept `responsive`, `mobile`, `tablet`, or `desktop` as
a `viewport`. Explicit paired width and height take precedence. The launch-env
tool accepts the same named presets when dimensions are omitted.

## Interaction batches

Prefer controls, then registered actions, then keystrokes, and finally
story-relative pointer coordinates. Discover non-internal runtime actions and
their JSON argument schemas with `storybook_list_actions` after every launch.

`storybook_run_steps` accepts an optional route, controls, paired rendered-pixel
dimensions or viewport, a required non-empty step list, and an optional final
capture. Step types are `focus_next`, `focus_previous`, `blur`, `keystrokes`,
`text`, `dispatch_action`, `pointer_move`, `pointer_click`, `scroll`, and
`wait_frames`.

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

Captures exclude gallery or dock chrome. A substory route crops to its section.
Use returned pixel dimensions rather than requested dimensions as the rendered
source of truth.

## Failure checks

- Route missing: inspect active filters and discover the base key again.
- No live host: await initialization and construct a standard view.
- Automation busy: wait before submitting another capture or mutation; work is
  not queued.
- Interaction tools missing: set the explicit capability before server
  construction and rediscover tools.
- Invalid action: rediscover actions for this launch and use its JSON schema.
- Partial interaction failure: inspect the dispatched count and do not retry.
- Invalid dimensions: provide positive width and height together.
- Corrupt stdio: move application logs to standard error.
- Startup timeout: confirm story registration linkage and open the route
  interactively.
