# Explicit story example

[![CI](https://github.com/stayhydated/gpui-storybook/actions/workflows/ci.yml/badge.svg)](https://github.com/stayhydated/gpui-storybook/actions/workflows/ci.yml)
[![Codecov](https://codecov.io/github/stayhydated/gpui-storybook/graph/badge.svg)](https://codecov.io/github/stayhydated/gpui-storybook)
[![Book](https://img.shields.io/badge/book-online-blue)](https://stayhydated.github.io/gpui-storybook/book/)
[![crates.io](https://img.shields.io/crates/v/gpui-storybook.svg)](https://crates.io/crates/gpui-storybook)

This package demonstrates the `#[story]` workflow for previews that own GPUI
state, focus, actions, or custom wrapper UI.

Run the Storybook:

```bash
cargo run -p gpui-storybook-example-story
```

Use the title-bar **Layout** select to switch between Gallery and Dock workspace.
The selection is saved for this example binary. Uncomment
`window_mode = "dock"` in this package's `storybook.toml` to start a launch in
Dock workspace; the selector can still change and save the later choice.

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
the workspace root and use **Run fresh** in the Scenarios tab. The sticky
Actions and Scenarios toolbars each keep **Reset** available; either action
restores constructor defaults and clears the last scenario result without
dispatching anything. MCP is needed when driving the same workflow through
remote tools or requesting capture.

`ButtonStory` also demonstrates an opt-in root action scope, boolean, numeric,
and enum-select `StoryControls`, reset behavior, preview viewport presentation,
and stable `Substory` capture routes. `InteractionStory` keeps its root
action-scope handle separate from its nested input interaction focus, so input
editing actions do not leak into the Actions tab.
`GroupedSummaryStory` and `GroupedDetailsStory` share one visible title and
navigation entry. Choose them from the workbench's **Variant** select; the
gallery displays one at a time, and dock mode opens each choice in its own tab.
Controls are opt-in; fields without `#[storybook(control...)]` remain story-only
state.

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

On Linux or macOS, exercise the examples in gallery or dock mode with the `mcp`
feature:

```bash
GPUI_STORYBOOK_MCP_STDIO=1 \
GPUI_STORYBOOK_MCP_ALLOW_INTERACTION=1 \
cargo run -p gpui-storybook-example-story --features mcp
```

On Linux, place `gpui-storybook-launch --` before the Cargo command. The application remains on GPUI's normal
Wayland backend and receives compositor-driven frame callbacks. On macOS, use
the Cargo command directly; GPUI's native image renderer supplies capture. The
`mcp` feature is unsupported on Windows.

Switch to **Dock workspace** to run the identical executor through the dock
host. Rediscover registered actions after each launch before dispatching one.
Opening a route for an interaction batch focuses that story's focus handle. The
fixture therefore inserts text first, moves focus once to reach the select, and
waits a frame after confirming the selection before dispatching a final status
action.
