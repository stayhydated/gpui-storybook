# gpui-storybook-core

`gpui-storybook-core` provides the gallery, dock workspace, story containers,
window-scoped controls/theme/inspection workbench, preview presentation state,
localization bridge, preference UI, and automation controller used by GPUI
Storybook.

Application developers should normally depend on
[`gpui-storybook`](../gpui-storybook/README.md), which owns initialization,
configuration discovery, story generation, and optional macro and MCP
re-exports. Depend on this crate directly only when building a custom Storybook
runtime integration.

Its public integration APIs include `ControlValue`, `ControlSpec`,
`ControlTarget`, `WorkbenchState`, `ThemeDraft`, `StoryInspectorState`, and the
shared automation controller. The facade re-exports the application-facing
parts.

In native debug builds, `STORYBOOK_THEME_DIR` selects the consumer-owned custom
theme directory watched by the runtime. With no override, the runtime watches
its bundled theme directory. Wasm retains in-memory editing without filesystem
watching.

See the [workbench guide](../../book/src/workbench.md), [user
guide](../../book/src/introduction.md), and [API
documentation](https://docs.rs/gpui-storybook-core/).
