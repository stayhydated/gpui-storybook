# GPUI Storybook

[![Build Status](https://github.com/stayhydated/gpui-storybook/actions/workflows/ci.yml/badge.svg)](https://github.com/stayhydated/gpui-storybook/actions/workflows/ci.yml)
[![Codecov](https://codecov.io/github/stayhydated/gpui-storybook/graph/badge.svg)](https://codecov.io/github/stayhydated/gpui-storybook)
[![Docs](https://docs.rs/gpui-storybook/badge.svg)](https://docs.rs/gpui-storybook/)
[![Crates.io](https://img.shields.io/crates/v/gpui-storybook.svg)](https://crates.io/crates/gpui-storybook)

GPUI Storybook is a searchable component preview shell for GPUI applications. It
supports stateful stories, component-derived stories, persistent appearance and
language preferences, a live controls/theme/inspection workbench, an optional
dock workspace, and optional MCP automation for typed controls, in-process
interaction, and PNG capture.

## Try the examples

Run the explicit `#[story]` example:

```bash
cargo run -p gpui-storybook-example-story
```

Run the `#[derive(ComponentStory)]` example:

```bash
cargo run -p gpui-storybook-example-component
```

Add `--features dock` to either command to open the dock workspace.

Both modes include a right-side workbench. Control registration is explicit:
mark fields with `#[storybook(control)]` to edit the selected story instance
without rebuilding, and leave all other fields unmarked:

```rust
#[derive(gpui_storybook::StoryControls)]
struct ButtonStory {
    #[storybook(control)]
    disabled: bool,
    #[storybook(control(min = 0.0, max = 32.0, step = 1.0))]
    padding: f32,
}
```

The preview canvas stays centered inside a visible frame. **Mobile**, **Tablet**,
and **Desktop** use locked preset dimensions; **Responsive** exposes resize
handles and starts from the dimensions of the fixed preset selected immediately
before it. The canvas remains centered within the visible main pane as the
sidebars change width or visibility; dedicated left and right panel icons sit in
the top bar immediately before the appearance settings button. Responsive frames
fill that pane without an artificial inset and remain visibly contained within it.

The Theme tab edits every serialized theme color in memory. Native debug builds
can watch a consumer theme directory by setting `STORYBOOK_THEME_DIR` before
launch; Wasm supports in-app editing without filesystem watching.

Enable the `mcp` feature to discover routes, drive controls, and capture the
story region. Generic focus, keyboard, action, pointer, scroll, and frame-wait
steps require an explicit capability gate because they can activate arbitrary
application behavior:

```bash
GPUI_STORYBOOK_MCP_STDIO=1 \
GPUI_STORYBOOK_MCP_ALLOW_INTERACTION=1 \
cargo run -p my-app-storybook --features mcp
```

Linux automation uses the normal Wayland-backed GPUI application under Sway's
wlroots headless backend. Install Sway and Mesa's software graphics drivers;
`storybook_capture_launch_env` generates the complete compositor wrapper
automatically on Linux. macOS and Windows keep their normal native launch
commands.

Interaction runs inside the live GPUI window; it does not require compositor
or operating-system input injection. Named and paired capture dimensions target
the story region while the gallery or dock chrome stays mounted for layout; the
returned PNG is cropped to the story region and excludes that chrome. See
[Automation and capture](book/src/automation.md) for the closed step schema,
safety limits, and capture ordering.

## Start using Storybook

Most applications should depend on the `gpui-storybook` facade crate. Register
a component or a stateful story, initialize Storybook, await preference
readiness, and then open a gallery or dock window.

- [User guide](book/src/introduction.md)
- [Getting started](book/src/getting_started.md)
- [Story registration](book/src/stories.md)
- [Use the workbench](book/src/workbench.md)
- [Configuration](book/src/configuration.md)
- [Automation and capture](book/src/automation.md)
- [API documentation](https://docs.rs/gpui-storybook/)
