# gpui-storybook-launch

[![crates.io: gpui-storybook-launch][crate-badge]][crate]

`gpui-storybook-launch` runs a child command inside a private headless Sway
session on Linux, supplying the compositor-driven frame callbacks required by
GPUI Storybook MCP and startup capture without using the physical display.

Storybook's Wayland window capture requires the GPUI Git patches from the
Storybook repository's root `Cargo.toml` in the child application's workspace
root.

## Example

From the GPUI Storybook workspace, place environment variables before the
launcher and the example's child command after `--`:

```sh
GPUI_STORYBOOK_MCP_STDIO=1 \
gpui-storybook-launch -- cargo run -p gpui-storybook-example-story --features mcp
```

The launcher uses `sway` from `PATH` by default. `GPUI_STORYBOOK_SWAY` and the
`--sway` option select a different executable. It inherits the child's standard
streams, returns its exit status, and stops the private compositor when the
child exits. Startup errors include available Sway diagnostics, retaining logs
that contain invalid UTF-8.

[crate-badge]: https://img.shields.io/crates/v/gpui-storybook-launch.svg?label=gpui-storybook-launch
[crate]: https://crates.io/crates/gpui-storybook-launch
