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

See [Automation and capture](../../book/src/automation.md) for tool names,
routes, environment variables, and troubleshooting. Direct integration APIs are
documented on [docs.rs](https://docs.rs/gpui-storybook-mcp/).
