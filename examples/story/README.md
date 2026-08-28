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

Inspect explicitly scoped story-root actions in every build, or add GPUI timing
telemetry:

```bash
cargo run -p gpui-storybook-example-story --features performance
```

The registrations live under `src/stories`.
`ActionsAndScenariosStory` is the focused command-workflow example: three
documented unit actions share one root action scope, visible Buttons dispatch
those same actions, contextual key bindings appear in the Actions tab, and two
scenarios recreate the story before dispatching ordered commands and checking
the rendered `actions-scenarios-state` value. Launch with plain `cargo run` from
the workspace root and use **Run fresh** in the Scenarios tab; MCP is needed
when driving the same workflow through remote tools or requesting capture.

`ButtonStory` also demonstrates an opt-in root action scope, boolean, numeric,
and enum-select `StoryControls`, reset behavior, preview viewport presentation,
and stable `Substory` capture routes. `InteractionStory` keeps its root
action-scope handle separate from its nested input interaction focus, so input
editing actions do not leak into the Actions tab.
Controls are opt-in; fields without `#[storybook(control...)]` remain story-only
state. See [Write stories](../../book/src/stories.md) for the registration
contract and [Use the workbench](../../book/src/workbench.md) for controls,
theme editing, action/keymap diagnostics, performance telemetry, and optional
inspection, and [Automation and
capture](../../book/src/automation.md) for MCP usage.

Export the linked static registration catalog without starting GPUI:

```bash
cargo run -p gpui-storybook-example-story --example catalog
```

The deterministic JSON includes stable identity, source provenance, story
Rustdocs, and static control shapes for documentation or CI tooling.

`InteractionStory` is the broader automation fixture. It provides a text input,
keyboard-operated select, semantic `pointer-target`, schema-backed
`interaction_story::SetAutomationStatus` action, viewport readout, typed
`prefix` control, a one-frame `pressed` state, and a structured
`fixture-state` semantic value. Use `storybook_click_target` for one semantic
click and `storybook_wait_for_value` to establish a bounded JSON postcondition
without a screenshot.

Its `type-click-and-dispatch` scenario demonstrates the explicit
`Story::scenarios()` workflow: each run recreates the fixture, applies the
`prefix` control, executes four named steps, checks three exact JSON
postconditions, and reports the result in the Scenarios tab or MCP.

Exercise the examples in gallery or dock mode:

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
