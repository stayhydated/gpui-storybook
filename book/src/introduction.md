# GPUI Storybook

GPUI Storybook gives a GPUI application a dedicated place to render, inspect,
and automate components outside the application's normal navigation. Use it to
browse registered stories, switch appearance and language, and capture stable
story routes.

## What you can build

A Storybook binary can provide:

- a searchable gallery of linked stories;
- a dock workspace for panel-based inspection;
- stateful stories with their own focus, actions, and lifecycle;
- component stories generated from example data;
- persistent, consumer-scoped appearance and language preferences;
- stable story and substory routes for MCP automation and PNG capture.

The `gpui-storybook` facade is the normal application dependency. The
`-core`, `-macros`, `-toml`, and `-mcp` crates support deeper runtime or
tooling integrations.

## Choose a registration style

Use `#[story]` when a preview needs a dedicated GPUI entity, focus handle,
actions, or custom layout. Implement `Story`, `Render`, and `Focusable` for
the registered type.

Use `#[derive(ComponentStory)]` when a component already implements
`IntoElement` and can be constructed from `Default` or an example
expression. Storybook generates the wrapper entity.

Both styles produce the same searchable story metadata and stable automation
keys. You can mix them in one binary.

## Choose a window mode

The default gallery is the shortest path to a focused story browser. Enable the
`dock` feature when the Storybook should use docked panels and a workspace
layout. Story registration and configuration are shared between both modes.

## Continue

1. [Set up a Storybook binary](getting_started.md).
2. [Register stateful or component stories](stories.md).
3. [Configure grouping, filtering, and launch overrides](configuration.md).
4. [Understand saved and resolved preferences](preferences.md).
5. [Enable MCP automation or PNG capture](automation.md).
