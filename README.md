# GPUI Storybook

[![Build Status](https://github.com/stayhydated/gpui-storybook/actions/workflows/ci.yml/badge.svg)](https://github.com/stayhydated/gpui-storybook/actions/workflows/ci.yml)
[![Codecov](https://codecov.io/github/stayhydated/gpui-storybook/graph/badge.svg)](https://codecov.io/github/stayhydated/gpui-storybook)
[![Docs](https://docs.rs/gpui-storybook/badge.svg)](https://docs.rs/gpui-storybook/)
[![Crates.io](https://img.shields.io/crates/v/gpui-storybook.svg)](https://crates.io/crates/gpui-storybook)

GPUI Storybook is a searchable component preview shell for GPUI applications. It
supports stateful stories, component-derived stories, persistent appearance and
language preferences, a live controls/theme/inspect workbench, opt-in GPUI
Inspector integration, an optional dock workspace, and optional MCP automation
for typed controls, in-process interaction, and PNG capture.

## Try the examples

Run the explicit `#[story]` example:

```bash
cargo run -p gpui-storybook-example-story
```

Run the `#[derive(ComponentStory)]` example:

```bash
cargo run -p gpui-storybook-example-component
```

Add `--features dock` to either command to open the dock workspace. Add
`--features inspector` to expose the GPUI Inspector button and story-root
metadata. Both features are opt-in and can be combined as
`--features dock,inspector`.

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
the top bar immediately before the appearance settings button. Responsive
frames keep a small, symmetric resize gutter so every edge and corner handle
remains reachable, including when the frame is larger than the visible pane.

The Theme tab edits every serialized theme color in memory. Native debug builds
can watch a consumer theme directory by setting `STORYBOOK_THEME_DIR` before
launch; Wasm supports in-app editing without filesystem watching. Choosing a
named base theme also activates its registered light or dark appearance, while
Storybook remembers the theme in the opposite slot for later appearance
changes.

The Inspect tab always shows the active story key and source location. Enable
the `inspector` feature to add its GPUI Component Inspector button and
Storybook metadata for selected story roots.

Enable the `mcp` feature to discover routes, drive controls, and capture the
story region. Generic focus, keyboard, action, pointer, scroll, and frame-wait
steps require an explicit capability gate because they can activate arbitrary
application behavior:

```bash
GPUI_STORYBOOK_MCP_STDIO=1 \
GPUI_STORYBOOK_MCP_ALLOW_INTERACTION=1 \
cargo run -p my-app-storybook --features mcp
```

Wrap important controls with stable semantic targets so an MCP client can
discover and activate them without screen coordinates:

```rust
use gpui::InteractiveElement as _;
use gpui_storybook::StorybookElementExt as _;

Button::new("execute-request")
    .label("Execute")
    .storybook_target()
```

The GPUI element ID becomes the stable route-local key, and Storybook derives
its display label (`execute-request` becomes `Execute request`). Use
`storybook_target_as(key, label)` when those values need to differ.

Wrap Serde-serializable rendered application state with `storybook_value` when
automation needs a machine-readable postcondition:

```rust
div()
    .id("response")
    .child(response_panel)
    .storybook_value(&response_state)
```

`storybook_read_value` reads one key and `storybook_wait_for_value` refreshes a
bounded number of frames until the value or a JSON Pointer matches. The
interaction-gated `storybook_click_target` performs one semantic click without
constructing a step batch. MCP uses explicit `story_key`, `target_key`,
`value_key`, and `control_key` input names, and its initial tool call waits for
the live Storybook host with a bounded deadline. None of these semantic reads
capture a frame; use screenshot capture when rendered appearance is the
assertion.

Linux automation uses the normal Wayland-backed GPUI application under Sway's
wlroots headless backend. Install Sway and Mesa's software graphics drivers;
install `gpui-storybook-launch`, then run the Cargo command through it.
`storybook_capture_launch_env` emits this launcher automatically on Linux.
macOS and Windows keep their normal native launch commands.

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
