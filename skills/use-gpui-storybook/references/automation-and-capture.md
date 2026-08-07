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
run stdio and startup-capture sessions through a headless Wayland compositor:

```bash
(
  runtime_dir="$(mktemp -d)"
  chmod 700 "$runtime_dir"
  printf '%s\n' \
    'output * mode 1920x1200' \
    'seat seat0 fallback true' \
    'for_window [app_id=".*"] floating enable' \
    > "$runtime_dir/sway.conf"

  cleanup() {
    kill "$sway_pid" 2>/dev/null || true
    wait "$sway_pid" 2>/dev/null || true
    rm -rf "$runtime_dir"
  }
  trap cleanup EXIT

  export XDG_RUNTIME_DIR="$runtime_dir"
  unset DISPLAY I3SOCK SWAYSOCK WAYLAND_DISPLAY WAYLAND_SOCKET ZED_HEADLESS
  WLR_BACKENDS=headless \
  WLR_HEADLESS_OUTPUTS=1 \
  WLR_LIBINPUT_NO_DEVICES=1 \
  WLR_RENDERER=gles2 \
  WLR_RENDERER_ALLOW_SOFTWARE=1 \
  LIBGL_ALWAYS_SOFTWARE=1 \
  sway --unsupported-gpu --config "$runtime_dir/sway.conf" \
    > "$runtime_dir/sway.log" 2>&1 &
  sway_pid=$!

  until wayland_socket="$(find "$runtime_dir" -maxdepth 1 \
    -type s -name 'wayland-*' -print -quit)" && \
    [ -n "$wayland_socket" ]; do
    if ! kill -0 "$sway_pid" 2>/dev/null; then
      cat "$runtime_dir/sway.log" >&2
      exit 1
    fi
    sleep 0.05
  done

  export WAYLAND_DISPLAY="${wayland_socket##*/}"
  export LIBGL_ALWAYS_SOFTWARE=1
  GPUI_STORYBOOK_MCP_STDIO=1 \
  cargo run -p my-app-storybook --features mcp
)
```

Sway provides a compatibility seat, window management, and frame callbacks
while retaining GPUI's normal Wayland backend. The launch-env tool emits a
bounded-readiness version of this wrapper on Linux and continues to emit a
direct Cargo command elsewhere.
The `--unsupported-gpu` flag only bypasses Sway's host-driver check; the
headless backend and software GLES renderer remain explicitly selected.

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
a `viewport`. Explicit paired width and height take precedence. Both forms size
the captured story region while retaining the surrounding gallery or dock
chrome. The launch-env tool accepts the same named presets when dimensions are
omitted.

## Interaction batches

Prefer controls, then registered actions, then keystrokes, and finally
story-relative pointer coordinates. Discover non-internal runtime actions and
their JSON argument schemas with `storybook_list_actions` after every launch.

`storybook_run_steps` accepts an optional route, controls, paired rendered-pixel
dimensions or viewport, a required non-empty step list, and an optional final
capture. Step types are `focus_next`, `focus_previous`, `blur`, `keystrokes`,
`text`, `dispatch_action`, `pointer_move`, `pointer_click`, `scroll`, and
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
uses the same presentation with temporary storage. On Linux, commands from
`storybook_capture_launch_env` create a private Wayland runtime, wait for
headless Sway, and then run Cargo.

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
- Partial interaction failure: inspect the dispatched count and do not retry.
- Invalid dimensions: provide positive width and height together.
- Corrupt stdio: move application logs to standard error.
- Startup timeout: confirm story registration linkage and open the route
  interactively.
