# gpui-storybook-launch

[![crates.io: gpui-storybook-launch][crate-badge]][crate]

`gpui-storybook-launch` runs a child command inside a private headless Sway
session on Linux, supplying the compositor-driven frame callbacks required by
GPUI Storybook MCP and startup capture without using the physical display.

Requires Rust 1.99 or newer, Linux, Sway on `PATH`, and a working Wgpu adapter
for the child GPUI application.

The launcher supports inspection and interaction on the published Kit 0.9
stack. Live Wayland window capture belongs to the older `gpui-pre =0.3.7`
[`wayland-render-image` branch][wayland-patches]; those fixed Git patches are
incompatible with Kit 0.9. Use `gpui-storybook-test` for published headless
captures on Linux.

## Commands

From the GPUI Storybook workspace, with Sway and the native rendering
prerequisites available, run the story example as the child command after `--`:

```sh
GPUI_STORYBOOK_MCP_STDIO=1 \
gpui-storybook-launch -- cargo run -p gpui-storybook-example-story --features mcp
```

The launcher uses `sway` from `PATH` by default. `GPUI_STORYBOOK_SWAY` and the
`--sway` option select a different executable. It inherits the child's standard
streams, returns its exit status, and stops the private compositor when the
child exits. Startup errors include available Sway diagnostics.

[crate-badge]: https://img.shields.io/crates/v/gpui-storybook-launch.svg?label=gpui-storybook-launch
[crate]: https://crates.io/crates/gpui-storybook-launch
[wayland-patches]: https://github.com/stayhydated/gpui-storybook/blob/wayland-render-image/Cargo.toml
