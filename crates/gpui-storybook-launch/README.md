# gpui-storybook-launch

`gpui-storybook-launch` runs a command inside a private Sway session on Linux,
using wlroots' headless backend and the Pixman software renderer. This supplies
the compositor-driven frame callbacks required by GPUI Storybook MCP and
startup-capture sessions without touching the physical display.

This crate and command support Linux only. macOS and Windows are unsupported
and produce a compile-time error instead of bypassing the compositor lifecycle.

Install the command once, then place Storybook environment variables before it:

```sh
cargo install gpui-storybook-launch
GPUI_STORYBOOK_MCP_STDIO=1 \
GPUI_STORYBOOK_MCP_ALLOW_INTERACTION=1 \
gpui-storybook-launch -- cargo run -p my-storybook --features mcp
```

The launcher uses `sway` from `PATH`. Set `GPUI_STORYBOOK_SWAY` or pass
`--sway /path/to/sway` when using a private package extraction. It waits for the
Wayland socket, inherits the child's standard streams, returns the child's exit
status, and stops Sway when the child exits.

Most applications should depend on the `gpui-storybook` facade. This crate is a
small standalone command for automation hosts and CI runners.
