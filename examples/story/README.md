# Explicit story example

`gpui-storybook-example-story` demonstrates `#[story]` registrations for
previews that own GPUI state, focus, actions, lifecycle, or custom wrapper UI.

## Overview

The example covers typed `StoryControls`, stable `Substory` routes, grouped
variants, runtime Gallery/Dock selection, scoped workbench actions, declared
scenarios, static catalog export, and semantic automation targets and values.
`InteractionStory` is the inert fixture for MCP input and structured-state
checks, while `ActionsAndScenariosStory` demonstrates one command model shared
by buttons, key bindings, workbench actions, and fresh scenario runs.

The `inspector`, `performance`, and `mcp` features forward the corresponding
facade capabilities. MCP sessions are supported on Linux and macOS; Linux uses
`gpui-storybook-launch` and a private Sway compositor.
