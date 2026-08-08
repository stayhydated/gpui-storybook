# Automation and capture

Enable the `mcp` feature to inspect and open stories from another process or
to capture the rendered story region as a PNG. The standard gallery and dock
views attach the automation controller installed by `gpui_storybook::init`.

## Enable MCP support

Forward the facade feature from the Storybook package:

```toml
[features]
mcp = ["gpui-storybook/mcp"]
```

Run an MCP server over standard input and output:

```bash
GPUI_STORYBOOK_MCP_STDIO=1 \
cargo run -p my-app-storybook --features mcp
```

On Linux, install Sway and Mesa's software graphics drivers. For Debian or
Ubuntu:

```bash
sudo apt-get install --no-install-recommends \
  libgl1-mesa-dri mesa-vulkan-drivers sway
```

Then run MCP sessions through a headless Wayland compositor:

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

This uses the normal Wayland-backed GPUI application. The private
`XDG_RUNTIME_DIR` and discovered `WAYLAND_DISPLAY` select Sway's in-memory
wlroots compositor, which provides a compatibility seat, window management,
and frame callbacks. Mesa renders both compositor and application in software.
macOS and Windows continue to use their native launch paths.
`--unsupported-gpu` bypasses Sway's host-driver startup check; the explicit
headless backend and software renderer keep this wrapper off the physical GPU.

This launch exposes route, control, and capture tools. Generic input can invoke
arbitrary application behavior, so enable it separately and only against a
safe Storybook backend:

```bash
GPUI_STORYBOOK_MCP_STDIO=1 \
GPUI_STORYBOOK_MCP_ALLOW_INTERACTION=1 \
cargo run -p my-app-storybook --features mcp
```

The interaction variable must equal `1`. When it is absent or has another
value, the action-discovery and interaction tools are omitted from MCP tool
discovery.

Send application logs to standard error so they do not corrupt the MCP
protocol stream.

A stdio launch uses temporary preference storage and a deterministic light
presentation with the `Default Light` theme and the application's fallback
language. It does not overwrite interactive preferences.

### Verify raw stdio with the example

Inside this repository, use the explicit story example as a safe end-to-end
target. Replace the final Cargo command in the Sway wrapper with:

```bash
GPUI_STORYBOOK_MCP_STDIO=1 \
GPUI_STORYBOOK_MCP_ALLOW_INTERACTION=1 \
cargo run -p gpui-storybook-example-story --features mcp
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

## Use the MCP tools

| Tool | Purpose |
|---|---|
| `storybook_list_stories` | List registered stories and stable route metadata |
| `storybook_get_story` | Inspect one story or substory route |
| `storybook_current_story` | Inspect the story displayed by the live window |
| `storybook_open_story` | Navigate the live window to a route |
| `storybook_read_controls` | Read control metadata and current values from the active variant |
| `storybook_set_control` | Set one control on the active story instance |
| `storybook_reset_control` | Reset one control, or all controls when `key` is omitted |
| `storybook_capture_current_story` | Capture the active story region |
| `storybook_capture_launch_env` | Build environment variables and a platform launch command |
| `storybook_list_actions` | List runtime GPUI actions, documentation, and argument schemas; interaction gate required |
| `storybook_run_steps` | Run one ordered in-process interaction batch with optional capture; interaction gate required |

Tool inputs and outputs use closed typed schemas. Use the advertised `key`,
`output_path`, `width`, `height`, `viewport`, `controls`, and launch properties;
unknown, missing, or invalid fields return structured errors.

## Run an in-process interaction batch

Use typed controls first when a story exposes them. Use registered actions for
semantic application commands, keystrokes for keyboard behavior, and
story-relative pointer coordinates as the fallback.

`storybook_run_steps` can open a route, apply a `controls` map, size the story
region for paired rendered-pixel `width` and `height` values or a named
`viewport`, execute a non-empty `steps` array, and capture the resulting route.
For example, the explicit example application's inert fixture supports text,
select navigation, and a typed action:

```json
{
  "route": "gpui-storybook-example-story-InteractionStory",
  "viewport": "mobile",
  "controls": {
    "prefix": { "type": "text", "value": "mcp" }
  },
  "steps": [
    { "type": "text", "value": "héllo 世界" },
    { "type": "focus_next" },
    { "type": "keystrokes", "keys": ["enter", "down", "enter"] },
    { "type": "wait_frames", "count": 1 },
    {
      "type": "dispatch_action",
      "name": "interaction_story::SetAutomationStatus",
      "args": { "value": "action-dispatched" }
    },
    { "type": "wait_frames", "count": 1 }
  ],
  "capture": {
    "output_path": "target/storybook-captures/interaction.png"
  }
}
```

When `route` is supplied, Storybook focuses that story's focus handle before
the first step. The fixture maps its focus handle to the text input, so the
first `focus_next` moves from that input to the select.

The closed step variants are:

| Step | Fields and behavior |
|---|---|
| `focus_next`, `focus_previous`, `blur` | Move or clear GPUI focus |
| `keystrokes` | Parse and dispatch each GPUI binding string in `keys` |
| `text` | Insert the UTF-8 `value` into the focused basic text input; this is not IME, clipboard, paste, or dead-key simulation |
| `dispatch_action` | Build a registered action by `name` and optional JSON `args`, dispatch it, and resume the batch after GPUI delivers that deferred dispatch |
| `pointer_move` | Dispatch a move at the story-relative `point` |
| `pointer_click` | Dispatch move, down, and up at `point`; optional `button`, `click_count`, and `modifiers` default to a single left click with no modifiers |
| `scroll` | Dispatch pixel `delta_x` and `delta_y` at `point` |
| `wait_frames` | Refresh and continue after the positive rendered-frame `count` |

Pointer points default to normalized coordinates:

```json
{ "space": "normalized", "x": 0.5, "y": 0.5 }
```

Both normalized coordinates must be finite and in `0.0..=1.0`. Use
`"space": "logical_pixels"` for non-negative GPUI logical pixels measured
from the current route origin. The executor resolves fresh bounds after route
opening and story-region sizing, rejects points beyond those bounds, and
translates them to window coordinates. It cannot target the gallery sidebar,
dock panels, title bar, global screen, or an element selector.

A batch allows at most 64 steps, up to 64 binding strings in each `keystrokes`
step, 4 KiB across UTF-8 `text` values and keystroke syntax, 120 explicitly
waited frames, and one final capture. Zero-frame waits and non-finite point or
scroll values are rejected. Every keystroke and action payload is constructed
before route, control, or input dispatch begins. The result reports a request
ID, active story, dispatched-step count, available GPUI dispatch observations,
whether any focus handle remains, and the optional capture. Dispatch means the
event was delivered; it does not prove the story's business operation
succeeded.

## Discover and dispatch actions

Call `storybook_list_actions` after each application launch. It omits GPUI
keymap sentinels and Storybook-private workbench actions, then returns each
automation-visible action's name, documentation, and JSON argument schema.
Validate the desired action against that runtime result before placing its name
and arguments in a `dispatch_action` step.

Action dispatch is deferred by GPUI and has no generic handler result. Its
observation is therefore `dispatched`, not `handled` or `succeeded`. The
interaction tool is marked destructive, non-idempotent, and open-world because
an application can bind a click or action to filesystem, network, process,
hardware, or other external effects.

Direct MCP embedders can enable the same capability without modifying process
environment:

```rust
let options = gpui_storybook::mcp::StorybookMcpServerOptions::default()
    .with_interaction(true);
let server = gpui_storybook::mcp::server_with_options(automation, options)?;
```

## Reproduce a controlled story

Open the route before reading or changing its controls:

```json
{
  "key": "my-app-storybook-ButtonStory"
}
```

`storybook_read_controls` returns each `ControlSpec` with its current value.
Pass a tagged value to `storybook_set_control`:

```json
{
  "key": "disabled",
  "value": { "type": "boolean", "value": true }
}
```

Other value tags are `integer`, `float`, `text`, `color`, `choice`, and `json`.
A color value contains `h`, `s`, `l`, and `a` numbers. The setter enforces the
advertised bounds and select options before updating the concrete story entity.

Call `storybook_reset_control` with a `key` to reset one value, or with an empty
object to reset all active-story controls.

## Address stories by stable route

A macro-generated base route has this form:

```text
{cargo-package-name}-{registered-type-name}
```

For example:

```text
my-app-storybook-ButtonStory
```

A captureable section appends its substory key:

```text
my-app-storybook-ButtonStory/with-progress
```

Use `storybook_list_stories` for active base keys. The catalog does not
enumerate sections rendered inside a story, so inspect the `Substory` enum,
`section(...)` title, or custom `StorySectionBase` for the suffix.

## Capture during startup

Set a route and output path before launching an MCP-enabled binary:

```bash
WGPU_CAPTURE_ROUTE=my-app-storybook-ButtonStory \
WGPU_CAPTURE_PATH=target/storybook-captures/button.png \
cargo run -p my-app-storybook --features mcp
```

Storybook opens the route, creates missing parent directories, writes the PNG,
and exits after capture. Capture startup disables preference persistence and
uses the deterministic light presentation. On Linux, wrap this command with
headless Sway as shown above.

| Environment variable | Meaning |
|---|---|
| `WGPU_CAPTURE_ROUTE` | Story key or `story-key/substory-key` route |
| `WGPU_CAPTURE_PATH` | PNG destination; required to write a capture |
| `WGPU_CAPTURE_WIDTH` | Requested captured story-region width in pixels |
| `WGPU_CAPTURE_HEIGHT` | Requested captured story-region height in pixels |
| `WGPU_CAPTURE_FRAME` | Optional one-based frame gate |

Set width and height together, and make both values greater than zero.
`WGPU_CAPTURE_FRAME`, when present, must also be greater than zero.
On Linux, `storybook_capture_launch_env` returns an `env` map and a `command`
array. Merge every `env` entry into the child process environment before
executing `command`. The command creates a private Wayland runtime, starts
headless Sway with a software GLES renderer, waits for its socket, and then
runs Cargo, but it does not inline the capture or MCP variables.

## Capture a live session

Call `storybook_open_story` with a stable route, then call
`storybook_capture_current_story`. The capture tool accepts an optional
`output_path`, optional paired `width` and `height`, a named `viewport`, and a
`controls` object. The control map is applied immediately before capture:

```json
{
  "output_path": "target/storybook-captures/button.png",
  "viewport": "mobile",
  "controls": {
    "disabled": { "type": "boolean", "value": true },
    "padding": { "type": "float", "value": 16.0 }
  }
}
```

Named viewports are `responsive`, `mobile` (390×844), `tablet` (768×1024),
and `desktop` (1440×900). Explicit paired dimensions take precedence over a
named viewport.

Only one Storybook automation mutation can be active at a time. Route opening,
control setters and resets, live or startup capture, and interaction batches
share the same operation guard and return `automation_busy` instead of queuing.
Catalog, current-story, control, and action reads remain available during a
batch and can observe intermediate rendered state. A successful capture result
contains the request ID, actual path, rendered pixel dimensions, and story
metadata.

`storybook_capture_launch_env` can construct the environment and platform
command for an external launcher. It accepts a route plus optional output path,
frame, paired dimensions, package, binary, feature list, and stdio selection.
It also accepts a named viewport when paired dimensions are omitted.

## Understand capture bounds and size

Captures contain the story view, excluding the gallery sidebar and header or
the dock workspace chrome. Substory routes crop to the registered section
region.

Width and height target the captured story region. Storybook adjusts the host
window around the existing gallery or dock chrome so sidebars, headers, and the
workbench remain mounted, then crops that chrome from the returned PNG. Display
scaling or compositor behavior can change the rendered result, so treat the
returned `pixel_width` and `pixel_height` as authoritative. Viewport text
rendered by a story can describe its logical live-window bounds instead of the
PNG size.

An interaction capture is part of the same exclusive UI-thread operation. It
captures the first requested rendered frame after the final step; explicit
`wait_frames` steps delay that frame. If capture fails after input was
dispatched, the structured error includes the partial dispatched-step count.
Do not retry an interaction batch automatically.

## Troubleshoot automation

| Symptom | Action |
|---|---|
| Route not found | Discover the base key with `storybook_list_stories`, inspect the substory definition, and check filtering |
| No live host is attached | Await initialization and construct a standard `Gallery` or `StoryWorkspace` view |
| Width or height is rejected | Set both dimensions to positive integers |
| A control is rejected | Read the current control specs and use the advertised type, bounds, and options |
| Automation is busy | Wait for the active capture or mutation to complete; requests are not queued |
| Interaction tools are missing | Set `GPUI_STORYBOOK_MCP_ALLOW_INTERACTION=1` before server construction and rediscover tools |
| Action is unknown or arguments are invalid | Call `storybook_list_actions` for this launch and follow its argument schema |
| Pointer point is rejected | Use finite normalized coordinates in `0.0..=1.0` or route-relative logical pixels inside the rendered bounds |
| Interaction reports partial execution | Inspect `steps_dispatched`; establish postconditions with a capture or read and do not retry automatically |
| Stdio messages cannot be decoded | Route tracing and diagnostics to standard error |
| Startup capture times out | Confirm registrations are linked and test the same route interactively |
