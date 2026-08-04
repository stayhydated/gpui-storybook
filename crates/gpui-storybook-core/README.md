# gpui-storybook-core

`gpui-storybook-core` provides the gallery, dock workspace, story containers,
window shell, localization bridge, preference UI, and automation controller used
by GPUI Storybook.

Application developers should normally depend on
[`gpui-storybook`](../gpui-storybook/README.md), which owns initialization,
configuration discovery, story generation, and optional macro and MCP
re-exports. Depend on this crate directly only when building a custom Storybook
runtime integration.

See the [user guide](../../book/src/introduction.md) and [API
documentation](https://docs.rs/gpui-storybook-core/).
