# Explicit story example

This package demonstrates the `#[story]` workflow for previews that own GPUI
state, focus, actions, or custom wrapper UI.

Run the gallery:

```bash
cargo run -p gpui-storybook-example-story
```

Run the dock workspace:

```bash
cargo run -p gpui-storybook-example-story --features dock
```

Run the gallery with the opt-in GPUI Inspector integration:

```bash
cargo run -p gpui-storybook-example-story --features inspector
```

The registrations live under `src/stories`; `ButtonStory` also demonstrates
boolean, numeric, and enum-select `StoryControls`, reset behavior, preview
viewport presentation, and stable `Substory` capture routes.
Controls are opt-in; fields without `#[storybook(control...)]` remain story-only
state. See [Write stories](../../book/src/stories.md) for the registration
contract and [Use the workbench](../../book/src/workbench.md) for controls,
theme editing, and optional inspection, and [Automation and
capture](../../book/src/automation.md) for MCP usage.

`InteractionStory` is the inert automation fixture. It provides a text input,
keyboard-operated select, semantic `pointer-target`, schema-backed
`interaction_story::SetAutomationStatus` action, viewport readout, typed
`prefix` control, a one-frame `pressed` state, and a structured
`fixture-state` semantic value. Use `storybook_click_target` for one semantic
click and `storybook_wait_for_value` to establish a bounded JSON postcondition
without a screenshot. Exercise the same route in gallery or dock mode:

```bash
GPUI_STORYBOOK_MCP_STDIO=1 \
GPUI_STORYBOOK_MCP_ALLOW_INTERACTION=1 \
cargo run -p gpui-storybook-example-story --features mcp
```

On Linux, install `gpui-storybook-launch` and place it before the Cargo command
as documented in the automation guide. The application remains on GPUI's normal
Wayland backend and receives compositor-driven frame callbacks.

Add `dock` to the feature list to run the identical executor through the dock
host. Rediscover registered actions after each launch before dispatching one.
Opening a route for an interaction batch focuses that story's focus handle. The
fixture therefore inserts text first, moves focus once to reach the select, and
waits a frame after confirming the selection before dispatching a final status
action.
