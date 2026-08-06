# gpui-storybook-mcp

`gpui-storybook-mcp` exposes typed MCP tools and frame-capture launch helpers
for a live GPUI Storybook window.

Applications should normally enable the `mcp` feature on
[`gpui-storybook`](../gpui-storybook/README.md). The facade installs the
automation controller during initialization, and the standard gallery and dock
views attach it automatically.

```toml
[dependencies]
gpui-storybook = { version = "0.5", features = ["mcp"] }
```

The MCP surface can list and open stories, read/set/reset the active story's
typed controls, capture with a serialized control map, and select a named or
custom viewport. Control values use the same tagged `ControlValue` JSON model
as the workbench:

```json
{
  "key": "disabled",
  "value": { "type": "boolean", "value": true }
}
```

The registered tools are `storybook_list_stories`, `storybook_get_story`,
`storybook_current_story`, `storybook_open_story`,
`storybook_read_controls`, `storybook_set_control`,
`storybook_reset_control`, `storybook_capture_current_story`, and
`storybook_capture_launch_env`.

See [Automation and capture](../../book/src/automation.md) for tool names,
routes, environment variables, and troubleshooting. Direct integration APIs are
documented on [docs.rs](https://docs.rs/gpui-storybook-mcp/).
