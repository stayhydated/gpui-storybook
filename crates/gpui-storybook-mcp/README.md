# gpui-storybook-mcp

`gpui-storybook-mcp` exposes typed MCP tools and frame-capture launch helpers
for a live GPUI Storybook window.

Applications should normally enable the `mcp` feature on
[`gpui-storybook`](../gpui-storybook/README.md). The facade installs the
automation controller during initialization, and the standard gallery and dock
views attach it automatically.

Linux automation runs the normal Wayland-backed application through Sway's
wlroots headless backend. On Debian or Ubuntu, install the runtime with:

```bash
sudo apt-get install --no-install-recommends \
  libgl1-mesa-dri mesa-vulkan-drivers sway
```

Install the reusable Linux launcher once:

```bash
cargo install gpui-storybook-launch
```

The `command` returned by `storybook_capture_launch_env` invokes this launcher;
its separate `env` map must be merged into the child process environment before
executing that command. Set `GPUI_STORYBOOK_SWAY` when Sway comes from a private
package extraction. macOS and Windows commands continue to launch Cargo
directly.

```toml
[dependencies]
gpui-storybook = { version = "0.5", features = ["mcp"] }
```

The default MCP surface can list and open stories, read/set/reset the active
story's typed controls, capture with a serialized control map, and select a
named or custom viewport. Control values use the same tagged `ControlValue`
JSON model as the workbench:

```json
{
  "control_key": "disabled",
  "value": { "type": "boolean", "value": true }
}
```

The default tools are `storybook_list_stories`, `storybook_get_story`,
`storybook_current_story`, `storybook_open_story`,
`storybook_read_controls`, `storybook_read_semantic_values`,
`storybook_read_value`, `storybook_wait_for_value`, `storybook_set_control`,
`storybook_reset_control`, `storybook_capture_current_story`, and
`storybook_capture_launch_env`.
The first tool call waits for the standard gallery or dock to publish its
catalog and attach the live host, with a 30-second startup deadline.

## Enable interaction intentionally

Generic in-process interaction is a separate runtime capability. Set the gate
to exactly `1` for a stdio launch:

```bash
GPUI_STORYBOOK_MCP_STDIO=1 \
GPUI_STORYBOOK_MCP_ALLOW_INTERACTION=1 \
cargo run -p my-app-storybook --features mcp
```

For a repository-local smoke test, use
`gpui-storybook-example-story` and the inert
`gpui-storybook-example-story-InteractionStory` route. The
[automation guide](../../book/src/automation.md#verify-raw-stdio-with-the-example)
shows the raw MCP initialization, discovery, and tool-call sequence.

On Linux, apply the complete [headless Sway
launcher](../../book/src/automation.md#enable-mcp-support) to the same session.

The gate adds `storybook_list_actions`, `storybook_list_interaction_targets`,
`storybook_click_target`, and `storybook_run_steps`. When it is unset or has
any other value, those tools are absent from discovery. The read-only semantic
value tools are always advertised; they read JSON values registered with
`StorybookElementExt::storybook_value` after refreshing the selected route.
Direct
embedders can avoid process environment changes:

```rust
let options = gpui_storybook::mcp::StorybookMcpServerOptions::default()
    .with_interaction(true);
let server = gpui_storybook::mcp::server_with_options(automation, options)?;
```

`tool_registry_with_options` builds the shared MCP definitions and handlers.
MCP retains the live automation state across calls until its transport or
application host is explicitly stopped.

`storybook_list_actions` returns automation-visible action names,
documentation, and JSON argument schemas from the launched GPUI application.
GPUI keymap sentinels and Storybook-private workbench actions are omitted.
Registrations are runtime state, so rediscover actions for every launch.

Import `gpui_storybook::StorybookElementExt as _`, then mark important story
controls with `.storybook_target()`. The GPUI element ID becomes the key and
also supplies a human-readable label; use `.storybook_target_as(key, label)`
when either needs an explicit value.
`storybook_list_interaction_targets` returns the selected route's stable keys,
labels, and live route-relative bounds. `storybook_click_target` accepts an
optional `story_key` and required `target_key`, then clicks that target exactly
once. Advanced batches use a `click_target` step with `target_key` and reject
missing or duplicate targets before dispatch.

Wrap any Serde-serializable rendered state with `.storybook_value(&state)`.
Use `storybook_read_value` for one known `value_key`, or
`storybook_wait_for_value` with an optional JSON Pointer and bounded frame count
for asynchronous state. `.storybook_value_as(key, label, &state)` handles
elements without GPUI IDs or independent display labels. These reads do not
capture a frame; use capture only for visual rendering assertions.

`storybook_run_steps` performs one ordered batch against the active story or
substory capture region. It can open a route, apply tagged control values, size
the story region for paired rendered-pixel dimensions or a named viewport,
dispatch up to 64 steps, and optionally capture the first rendered frame after
the last step:

```json
{
  "story_key": "gpui-storybook-example-story-InteractionStory",
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

Other steps are `focus_previous`, `blur`, `click_target`, `pointer_move`,
`pointer_click`, and `scroll`. Opening the route focuses the story's focus handle, which is the
fixture input, so the example inserts text before moving focus to the select.
Pointer points use `normalized` coordinates in `0.0..=1.0` by default, or
non-negative `logical_pixels`
relative to the freshly rendered route bounds. Click dispatch is move, down,
then up. Coordinates cannot target Storybook chrome, and there is no
element-selector or global-screen input.

One batch permits up to 64 binding strings per `keystrokes` step, 4 KiB across
UTF-8 `text` values and keystroke syntax, and 120 explicitly waited frames. The
entire batch, including every keystroke and action payload, is validated before
input dispatch. Capture, route navigation, control mutation, and interaction
batches share one exclusive operation guard; reads remain available and can
observe intermediate rendered state. A runtime failure reports the
dispatched-step count and is never retried automatically.

The mutation tool is advertised as destructive, non-idempotent, and
open-world. Enabling the gate authorizes input dispatch, not downstream
behavior: launch story applications against safe services and hardware.

See [Automation and capture](../../book/src/automation.md) for tool names,
routes, environment variables, and troubleshooting. Direct integration APIs are
documented on [docs.rs](https://docs.rs/gpui-storybook-mcp/).
