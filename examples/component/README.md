# Component story example

[![CI](https://github.com/stayhydated/gpui-storybook/actions/workflows/ci.yml/badge.svg)](https://github.com/stayhydated/gpui-storybook/actions/workflows/ci.yml)
[![Codecov](https://codecov.io/github/stayhydated/gpui-storybook/graph/badge.svg)](https://codecov.io/github/stayhydated/gpui-storybook)
[![Book](https://img.shields.io/badge/book-online-blue)](https://stayhydated.github.io/gpui-storybook/book/)
[![crates.io](https://img.shields.io/crates/v/gpui-storybook.svg)](https://crates.io/crates/gpui-storybook)

This package demonstrates `#[derive(ComponentStory)]` for components that can
render from example data while Storybook supplies the focusable wrapper.

Run the Storybook:

```bash
cargo run -p gpui-storybook-example-component
```

Use the title-bar **Layout** select to switch between Gallery and Dock workspace.
The selection is saved for this example binary. Uncomment
`window_mode = "dock"` in this package's `storybook.toml` to start a launch in
Dock workspace; the selector can still change and save the later choice.

Run the gallery with the opt-in GPUI Inspector integration:

```bash
cargo run -p gpui-storybook-example-component --features inspector
```

Inspect focus-scoped actions in every build, or add GPUI timing telemetry:

```bash
cargo run -p gpui-storybook-example-component --features performance
```

The registrations live under `src/components` and show literal, computed, and
localized metadata. `WelcomeCard`, `SignalBoard`, and `FieldNotes` also show
how `#[storybook(control)]` stores defaults from `example = ...` and overlays
live values on each render while leaving unmarked component fields out of the
control registry. Component stories that declare the same visible title, group,
and section share one navigation entry; the workbench **Variant** select chooses
the concrete wrapper, and dock mode keeps each selected wrapper in its own tab.

`WelcomeCard` also passes `scenarios = WelcomeCard::scenarios()` to the derive.
Its named scenario proves that component-generated wrappers use the same fresh
story, typed-control, workbench, and automation contract as explicit stories.

The standard `gpui_storybook::init`, gallery, and dock paths attach the live
in-process scenario host for component-derived stories, so workbench runs use a
plain application launch. The sticky Scenarios toolbar's **Reset** action
recreates the component wrapper at its example defaults and clears the last
result without running a scenario. On Linux or macOS, add `--features mcp` for
remote typed route, control, and capture tools; the feature is unsupported on
Windows. Also set
`GPUI_STORYBOOK_MCP_ALLOW_INTERACTION=1` when generic in-process keyboard,
action, pointer, scroll, and frame-wait tools are intentionally allowed. Use
the explicit story example's inert `InteractionStory` when testing the complete
interaction surface.

Named and paired capture dimensions target the selected component story region;
the gallery or dock chrome remains mounted around that region. On Linux, run
MCP and startup-capture sessions through `gpui-storybook-launch` and Sway's
wlroots headless backend; the application remains Wayland-backed. On macOS, launch Cargo directly and use
GPUI's native image renderer. Component stories can import
`StorybookElementExt` and mark important children with `.storybook_target()`
for stable MCP discovery and clicking, then attach Serde-serializable state
with `.storybook_value(&state)` for focused value reads and bounded waits.
