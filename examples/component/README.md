# Component story example

`gpui-storybook-example-component` demonstrates
`#[derive(ComponentStory)]` for components that render from example data while
Storybook supplies the focusable wrapper.

Requires Rust 1.99 or newer, GPUI Kit 0.7, and a native GPUI display for the
interactive gallery. This example package is not published.

## Overview

The example covers literal, computed, and localized metadata; typed controls
whose reset values come from `example = ...`; grouped variants; and
component-owned scenarios. It also shows how
components expose stable semantic targets and serialized values to automation.

## Features

No optional workbench features are enabled by default. The facade's normal
`macros` feature remains enabled.

| Feature | Default | Purpose |
| --- | --- | --- |
| `default` | Yes | Empty; enables none of this package's optional features. |
| `inspector` | No | Forwards `gpui-storybook/inspector`. |
| `mcp` | No | Forwards `gpui-storybook/mcp`; supported on Linux and macOS. |
| `performance` | No | Forwards `gpui-storybook/performance`. |

## Commands

From the workspace root, with the native GPUI prerequisites available:

```sh
cargo run -p gpui-storybook-example-component
```

This opens the gallery with this package's linked registrations. The
[maintained source][example] contains the complete declaration and render code.
For MCP sessions, Linux uses `gpui-storybook-launch` and a private Sway compositor.

[example]: https://github.com/stayhydated/gpui-storybook/blob/master/examples/component/src/components/welcome_card.rs
