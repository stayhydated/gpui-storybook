# Explicit story example

`gpui-storybook-example-story` demonstrates `#[story]` registrations for
previews that own GPUI state, focus, actions, lifecycle, or custom wrapper UI.

Requires Rust 1.99 or newer, GPUI Kit 0.8.0, and a native GPUI display for the
interactive gallery. This example package is not published.

## Overview

The example covers typed `StoryControls`, stable `Substory` routes, grouped
variants, scoped workbench actions, declared
scenarios, static catalog export, and semantic automation targets and values.
`InteractionStory` is the inert fixture for MCP input and structured-state
checks, while `ActionsAndScenariosStory` demonstrates one command model shared
by buttons, key bindings, workbench actions, and fresh scenario runs.

## Features

No optional workbench features are enabled by default. The facade's normal
`macros` feature remains enabled.

| Feature | Default | Purpose |
| --- | --- | --- |
| `inspector` | No | Forwards `gpui-storybook/inspector`. |
| `mcp` | No | Forwards `gpui-storybook/mcp`; supported on Linux and macOS. |
| `performance` | No | Forwards `gpui-storybook/performance`. |

## Commands

From the workspace root, with the native GPUI prerequisites available:

```sh
cargo run -p gpui-storybook-example-story
```

This opens the gallery with [`HelloWorld`][example] and this package's other
linked registrations.
For MCP sessions, Linux uses `gpui-storybook-launch` and a private Sway compositor.

[example]: https://github.com/stayhydated/gpui-storybook/blob/master/examples/story/src/stories/hello.rs
