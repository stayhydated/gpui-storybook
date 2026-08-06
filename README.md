# GPUI Storybook

[![Build Status](https://github.com/stayhydated/gpui-storybook/actions/workflows/ci.yml/badge.svg)](https://github.com/stayhydated/gpui-storybook/actions/workflows/ci.yml)
[![Codecov](https://codecov.io/github/stayhydated/gpui-storybook/graph/badge.svg)](https://codecov.io/github/stayhydated/gpui-storybook)
[![Docs](https://docs.rs/gpui-storybook/badge.svg)](https://docs.rs/gpui-storybook/)
[![Crates.io](https://img.shields.io/crates/v/gpui-storybook.svg)](https://crates.io/crates/gpui-storybook)

GPUI Storybook is a searchable component preview shell for GPUI applications. It
supports stateful stories, component-derived stories, persistent appearance and
language preferences, a live controls/theme/inspection workbench, an optional
dock workspace, and optional MCP automation and PNG capture.

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

Both modes include a right-side workbench. Mark fields with
`#[storybook(control)]` to edit the selected story instance without rebuilding:

```rust
#[derive(gpui_storybook::StoryControls)]
struct ButtonStory {
    #[storybook(control)]
    disabled: bool,
    #[storybook(control(min = 0.0, max = 32.0, step = 1.0))]
    padding: f32,
}
```

The Theme tab edits every serialized theme color in memory. Native debug builds
can watch a consumer theme directory by setting `STORYBOOK_THEME_DIR` before
launch; Wasm supports in-app editing without filesystem watching.

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
