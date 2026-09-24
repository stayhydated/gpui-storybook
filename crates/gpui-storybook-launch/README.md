# gpui-storybook-launch

[![crates.io: gpui-storybook-launch][crate-badge]][crate]

`gpui-storybook-launch` runs a child command inside a private headless Sway
session on Linux, supplying the compositor-driven frame callbacks required by
GPUI Storybook MCP and startup capture without using the physical display.

## Example

Place Storybook environment variables before the launcher and the child command
after `--`:

```sh
GPUI_STORYBOOK_MCP_STDIO=1 \
gpui-storybook-launch -- cargo run -p my-storybook --features mcp
```

The launcher uses `sway` from `PATH` by default. `GPUI_STORYBOOK_SWAY` and the
`--sway` option select a different executable. It inherits the child's standard
streams, returns its exit status, and stops the private compositor when the
child exits.

[crate-badge]: https://img.shields.io/crates/v/gpui-storybook-launch.svg?label=gpui-storybook-launch
[crate]: https://crates.io/crates/gpui-storybook-launch
