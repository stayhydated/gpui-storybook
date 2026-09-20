# GPUI Storybook

GPUI Storybook gives a GPUI application a dedicated place to render, inspect,
and automate components outside the application's normal navigation. Use it to
browse registered stories, edit live controls and theme colors, inspect story
roots, switch appearance and language, and capture stable story routes.

## What you can build

A Storybook binary can provide:

- a searchable gallery and a dock workspace selected at runtime;
- stateful stories with their own interaction focus, explicit root action
  scope, and lifecycle;
- component stories generated from example data;
- a right-side workbench with controls, theme editing, preview settings, story
  source details, selected-story actions and key bindings, opt-in performance
  telemetry, and opt-in GPUI Inspector integration;
- persistent, consumer-scoped appearance and language preferences;
- stable story and substory routes for Linux and macOS MCP automation and PNG
  capture.

The `gpui-storybook` facade is the normal application dependency. The
`-core`, `-macros`, and `-toml` crates support deeper runtime or tooling
integrations. The Linux/macOS `-mcp` crate owns remote automation and capture.

## Choose a registration style

Use `#[story]` when a preview needs a dedicated GPUI entity, focus handle,
actions, or custom layout. Implement `Story`, `Render`, and `Focusable` for
the registered type, and derive or implement `StoryControls`.

Use `#[derive(ComponentStory)]` when a component already implements
`IntoElement` and can be constructed from `Default` or an example
expression. Storybook generates the wrapper entity.

Both styles produce the same searchable story metadata and stable automation
keys. You can mix them in one binary.

## Choose a window mode

Open the standard Storybook window, then use its title-bar **Layout** select to
switch between the focused Gallery and the panel-based Dock workspace.
Storybook saves the typed mode per consumer; story registration, configuration,
and the workbench remain shared between both layouts. The active
`storybook.toml` can set `window_mode = "gallery"` or `window_mode = "dock"`
for a launch-specific initial layout.

## Next steps

Start by [setting up a Storybook binary](getting_started.md), then
[register a story](stories.md). Once it renders, choose the workflow you need:

- [Edit controls, themes, and preview settings](workbench.md).
- [Run portable tests and compare visual baselines](portable_testing.md).
- [Configure grouping, filtering, and launch overrides](configuration.md).
- [Understand saved and resolved preferences](preferences.md).
- [Enable MCP automation or PNG capture](automation.md).
