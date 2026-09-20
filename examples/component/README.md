# Component story example

`gpui-storybook-example-component` demonstrates
`#[derive(ComponentStory)]` for components that render from example data while
Storybook supplies the focusable wrapper.

## Overview

The example covers literal, computed, and localized metadata; typed controls
whose reset values come from `example = ...`; grouped variants; runtime
Gallery/Dock selection; and component-owned scenarios. It also shows how
components expose stable semantic targets and serialized values to automation.

The `inspector`, `performance`, and `mcp` features forward the corresponding
facade capabilities. MCP sessions are supported on Linux and macOS; Linux uses
`gpui-storybook-launch` and a private Sway compositor.
