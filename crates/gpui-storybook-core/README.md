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

The automation module also owns the MCP-independent interaction request and
result types, runtime action discovery, story-relative coordinate validation,
and one frame-aware executor shared by gallery and dock hosts. Navigation,
control mutations, capture, and interaction batches share an exclusive
operation guard; catalog, current-story, control, and action reads remain
available while a batch is active and may observe intermediate rendered state.
Custom integrations should use `StorybookAutomation` instead of dispatching
window input independently so validation, cancellation boundaries, and capture
ordering remain consistent. Capture sizing preserves the surrounding shell and
targets the rendered story region used for pointer bounds and PNG output.
Application bootstrap remains the embedding application's responsibility. On
Linux, non-interactive automation should run the normal Wayland application
through Sway's wlroots headless backend; the MCP launch helper supplies that
compositor wrapper.

In native debug builds, `STORYBOOK_THEME_DIR` selects the consumer-owned custom
theme directory watched by the runtime. With no override, the runtime watches
its bundled theme directory. Wasm retains in-memory editing without filesystem
watching.

See the [automation guide](../../book/src/automation.md), [workbench
guide](../../book/src/workbench.md), [user
guide](../../book/src/introduction.md), and [API
documentation](https://docs.rs/gpui-storybook-core/).
