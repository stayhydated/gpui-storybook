# gpui-storybook-core

`gpui-storybook-core` provides the gallery, dock workspace, story containers,
window-scoped controls/theme/inspect workbench, optional GPUI Inspector
integration, preview presentation state, localization bridge, preference UI,
and automation controller used by GPUI Storybook.

Application developers should normally depend on
[`gpui-storybook`](../gpui-storybook/README.md), which owns initialization,
configuration discovery, story generation, and optional macro and MCP
re-exports. Depend on this crate directly only when building a custom Storybook
runtime integration.

The runtime centers every preview canvas inside a visible frame. Fixed viewport
presets lock that frame to their named dimensions. Responsive mode alone exposes
edge and corner resize handles, and it inherits the dimensions of the fixed
preset that was selected immediately before it. Preview presentation pairs that
viewport choice with a theme, light, dark, or transparent canvas background. The
canvas stays centered within the visible center pane as sidebar visibility and
width change. Left and right panel icons in the top bar, immediately before
appearance settings, toggle story navigation and the workbench in gallery and
dock layouts. The initial Responsive frame keeps a small, symmetric resize
gutter so its handles remain reachable at every size.

Its public integration APIs include `ControlValue`, `ControlSpec`,
`ControlTarget`, `StoryPresentation`, `WorkbenchState`, `ThemeDraft`, and the
shared automation controller. The `inspector` feature adds
`StoryInspectorState`, the Inspector button, and GPUI Component's Inspector
integration. The facade re-exports the application-facing parts.

The automation module also owns the MCP-independent interaction request and
result types, runtime action and semantic-target discovery, structured
semantic-value reads, story-relative coordinate validation, and one frame-aware
executor shared by gallery and dock hosts. Navigation, control mutations,
capture, and interaction batches share an exclusive operation guard; catalog,
current-story, control, action, and semantic-value reads remain available while
a batch is active and may observe intermediate rendered state.
Custom integrations should use `StorybookAutomation` instead of dispatching
window input independently so validation, cancellation boundaries, and capture
ordering remain consistent. Capture sizing preserves the surrounding shell and
targets the rendered story region used for pointer bounds and PNG output.
Application bootstrap remains the embedding application's responsibility.
`StorybookElementExt` records stable keys and route-relative live bounds during
prepaint with `storybook_target`, and route-local JSON state during the same
phase with `storybook_value`. The implicit methods use the GPUI element ID as
the key and derive a readable label; the `_as` methods accept both explicitly.
Duplicate keys within either registry are rejected.
On Linux,
non-interactive automation should run the normal Wayland application through
the `gpui-storybook-launch` Sway wrapper.

In native debug builds, `STORYBOOK_THEME_DIR` selects the consumer-owned custom
theme directory watched by the runtime. With no override, the runtime watches
its bundled theme directory. Wasm retains in-memory editing without filesystem
watching. Selecting a named base theme activates the theme's matching light or
dark appearance immediately and preserves the opposite theme slot for later
appearance changes.

See the [automation guide](../../book/src/automation.md), [workbench
guide](../../book/src/workbench.md), [user
guide](../../book/src/introduction.md), and [API
documentation](https://docs.rs/gpui-storybook-core/).
