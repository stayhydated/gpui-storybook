# gpui-storybook-mcp

[![Codecov: gpui-storybook-mcp][codecov-badge]][codecov]
[![crates.io: gpui-storybook-mcp][crate-badge]][crate]

`gpui-storybook-mcp` provides typed MCP automation and story-region PNG capture
for live GPUI Storybook windows on Linux and macOS. Applications normally enable
it through the [facade's `mcp` feature][facade].

Requires Rust 1.99 or newer and a live GPUI Storybook host on Linux or macOS.
Depend on this crate directly when owning the MCP server or capture lifecycle.

## Overview

The tools discover stable story routes, read and set typed controls, run
declared scenarios, inspect rendered semantic values, and capture named
viewports or explicit dimensions. Generic actions and input are exposed only
when `GPUI_STORYBOOK_MCP_ALLOW_INTERACTION=1`; those operations can trigger
application effects.

Captures wait for the requested story canvas dimensions and exclude gallery
chrome from the PNG. Explicit paired dimensions override named viewports.

Linux launch commands use `gpui-storybook-launch` and a private Sway session.
Wayland window capture requires the GPUI Git patches from the
[`wayland-render-image` branch][wayland-patches]
in the application's workspace root.
macOS uses GPUI's native image renderer. Logs belong on standard error so the
JSON Lines transport on standard input and output remains valid.

## Example

In an active session, select a story with `storybook_open`, then read its control
keys with `storybook_read_controls`. For a story exposing a `disabled` control,
pass this arguments payload to `storybook_set_control` (it is not a complete
JSON-RPC message):

```json
{
  "control_key": "disabled",
  "value": { "type": "boolean", "value": true }
}
```

The tool applies the boolean to the active concrete story and returns its updated
control snapshot. See the [tool API reference][api] for the advertised schemas and
automation failures.

[codecov-badge]: https://codecov.io/gh/stayhydated/gpui-storybook/branch/master/graph/badge.svg?component=gpui-storybook-mcp
[codecov]: https://codecov.io/gh/stayhydated/gpui-storybook
[crate-badge]: https://img.shields.io/crates/v/gpui-storybook-mcp.svg?label=gpui-storybook-mcp
[crate]: https://crates.io/crates/gpui-storybook-mcp
[facade]: https://crates.io/crates/gpui-storybook
[wayland-patches]: https://github.com/stayhydated/gpui-storybook/blob/wayland-render-image/Cargo.toml
[api]: https://docs.rs/gpui-storybook-mcp
