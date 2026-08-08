# Component story example

This package demonstrates `#[derive(ComponentStory)]` for components that can
render from example data while Storybook supplies the focusable wrapper.

Run the gallery:

```bash
cargo run -p gpui-storybook-example-component
```

Run the dock workspace:

```bash
cargo run -p gpui-storybook-example-component --features dock
```

Run the gallery with the opt-in GPUI Inspector integration:

```bash
cargo run -p gpui-storybook-example-component --features inspector
```

The registrations live under `src/components` and show literal, computed, and
localized metadata. `WelcomeCard`, `SignalBoard`, and `FieldNotes` also show
how `#[storybook(control)]` stores defaults from `example = ...` and overlays
live values on each render while leaving unmarked component fields out of the
control registry. See [Write stories](../../book/src/stories.md) for
the derive contract, [Use the workbench](../../book/src/workbench.md) for live
editing and viewport settings, and [Getting
started](../../book/src/getting_started.md) for setup.

The standard gallery and dock constructors attach the same optional automation
host for component-derived stories. Launch with `--features mcp` for typed
route, control, and capture tools; also set
`GPUI_STORYBOOK_MCP_ALLOW_INTERACTION=1` when generic in-process keyboard,
action, pointer, scroll, and frame-wait tools are intentionally allowed. Use
the explicit story example's inert `InteractionStory` when testing the complete
interaction surface.

Named and paired capture dimensions target the selected component story region;
the gallery or dock chrome remains mounted around that region. On Linux, run
MCP and startup-capture sessions through Sway's wlroots headless backend as
described in the automation guide; the normal application remains Wayland-backed.
