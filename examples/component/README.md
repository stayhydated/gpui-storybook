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
See [Write stories](../../book/src/stories.md) for
the derive contract, [Use the workbench](../../book/src/workbench.md) for live
editing, viewport settings, action/keymap diagnostics, and performance
telemetry, and [Getting
started](../../book/src/getting_started.md) for setup.

`WelcomeCard` also passes `scenarios = WelcomeCard::scenarios()` to the derive.
Its named scenario proves that component-generated wrappers use the same fresh
story, typed-control, workbench, and automation contract as explicit stories.

The standard `gpui_storybook::init`, gallery, and dock paths attach the live
in-process scenario host for component-derived stories, so workbench runs use a
plain application launch. The sticky Scenarios toolbar's **Reset** action
recreates the component wrapper at its example defaults and clears the last
result without running a scenario. Add `--features mcp` for remote typed route,
control, and capture tools; also set
`GPUI_STORYBOOK_MCP_ALLOW_INTERACTION=1` when generic in-process keyboard,
action, pointer, scroll, and frame-wait tools are intentionally allowed. Use
the explicit story example's inert `InteractionStory` when testing the complete
interaction surface.

Named and paired capture dimensions target the selected component story region;
the gallery or dock chrome remains mounted around that region. On Linux, run
MCP and startup-capture sessions through `gpui-storybook-launch` and Sway's
wlroots headless backend as described in the automation guide; the normal
application remains Wayland-backed. Component stories can import
`StorybookElementExt` and mark important children with `.storybook_target()`
for stable MCP discovery and clicking, then attach Serde-serializable state
with `.storybook_value(&state)` for focused value reads and bounded waits.
