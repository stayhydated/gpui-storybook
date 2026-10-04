# gpui-storybook-mcp

[![Codecov: gpui-storybook-mcp][codecov-badge]][codecov]
[![crates.io: gpui-storybook-mcp][crate-badge]][crate]

`gpui-storybook-mcp` provides typed MCP automation and story-region PNG capture
for live GPUI Storybook windows on Linux and macOS. Applications normally enable
it through the [facade's `mcp` feature][facade].

## Overview

The tools discover stable story routes, read and set typed controls, run
declared scenarios, inspect rendered semantic values, and capture named
viewports or explicit dimensions. Generic actions and input are exposed only
when `GPUI_STORYBOOK_MCP_ALLOW_INTERACTION=1`; those operations can trigger
application effects.

Captures wait for the requested story canvas dimensions and exclude gallery
chrome from the PNG. Explicit paired dimensions override named viewports.

Linux launch commands use `gpui-storybook-launch` and a private Sway session.
Wayland window capture requires the GPUI Git patches from the repository root
`Cargo.toml` in the application's workspace root.
macOS uses GPUI's native image renderer. Logs belong on standard error so the
JSON Lines transport on standard input and output remains valid.

## Example

A control mutation uses the tagged `ControlValue` schema advertised by the tool:

```json
{
  "control_key": "disabled",
  "value": { "type": "boolean", "value": true }
}
```

[codecov-badge]: https://codecov.io/github/stayhydated/gpui-storybook/branch/master/graph/badge.svg?component=gpui-storybook-mcp
[codecov]: https://codecov.io/github/stayhydated/gpui-storybook
[crate-badge]: https://img.shields.io/crates/v/gpui-storybook-mcp.svg?label=gpui-storybook-mcp
[crate]: https://crates.io/crates/gpui-storybook-mcp
[facade]: https://crates.io/crates/gpui-storybook
