# gpui-storybook-core

`gpui-storybook-core` provides the gallery, dock workspace, story containers,
the runtime layout selector, window-scoped controls/theme/inspect/actions
workbench, optional GPUI Inspector integration, preview presentation state,
localization bridge, preference UI, and automation controller used by GPUI
Storybook.

Application developers should normally depend on
[`gpui-storybook`](../gpui-storybook/README.md), which owns initialization,
configuration discovery, story generation, and optional macro and MCP
re-exports. Depend on this crate directly only when building a custom Storybook
runtime integration.

The standard shell owns one automation command receiver while it swaps Gallery
and Dock workspace views. Its title-bar **Layout** select saves a typed
`StorybookWindowMode`; application title-bar additions remain composed beside
the selector. The facade resolves the initial mode from a per-window
`StorybookWindow::with_mode` value, then the active `storybook.toml`
`window_mode`, then the saved consumer preference.

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

`StoryContainer::variant_group` gives duplicate-title stories one navigation
descriptor while retaining their concrete containers. The workbench presents
those members in a **Variant** select; gallery mode renders the selected member,
and dock mode mounts selected members as independent tabs.

Its public integration APIs include `ControlValue`, `ControlSpec`,
`ControlTarget`, `StoryPresentation`, `WorkbenchState`, `ThemeDraft`, and the
shared automation controller. The `inspector` feature adds
`StoryInspectorState`, the Inspector button, and GPUI Component's Inspector
integration. The `performance` feature adds window profiler histograms and
debug frame-overlay controls. The Actions tab evaluates the selected story's
explicit `Story::action_scope_focus_handle`, not its primary interaction focus,
and removes actions also exposed through the workbench/root path. Nested input,
Storybook shell, and unrelated component actions therefore stay outside the
story catalog. Bindings and dispatch use that same action scope; a story without
one exposes no inferred actions. The facade re-exports the application-facing
parts.

`StoryScenario` keeps named steps, initial controls and presentation, exact
semantic postconditions, and optional capture with its owning story. Scenario
runs recreate the concrete story and rebind its focus and control target before
delegating to the shared interaction executor; gallery, dock, workbench, and MCP
therefore observe one fresh-run contract. `Gallery::view_with_automation` and
`StoryWorkspace::view_with_automation` carry their supplied controller into the
Scenarios workbench without requiring a default global. Postcondition and
capture failures after dispatch report the request ID and completed-step count.

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
Each story root replaces its complete route registry on every frame so removed
substories, semantic targets, and values cannot leak into later capture reads;
isolated runners can call `reset_capture_regions_for_story` before constructing
a fresh same-key context.
Facade-created controllers expose a readiness future that completes after the
standard gallery or dock publishes its catalog and attaches the live command
receiver. Application bootstrap remains the embedding application's responsibility.
`StorybookElementExt` records stable keys and route-relative live bounds during
prepaint with `storybook_target`, and route-local Serde-serializable state during
the same phase with `storybook_value`. The implicit methods use the GPUI element ID as
the key and derive a readable label; the `_as` methods accept both explicitly.
Duplicate keys within either registry are rejected.
The Linux-only MCP integration runs non-interactive automation through the
`gpui-storybook-launch` Sway wrapper while retaining the normal Wayland
application path. macOS and Windows do not support the MCP feature.

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
