# gpui-storybook-mcp

[![CI](https://github.com/stayhydated/gpui-storybook/actions/workflows/ci.yml/badge.svg)](https://github.com/stayhydated/gpui-storybook/actions/workflows/ci.yml)
[![Codecov](https://codecov.io/github/stayhydated/gpui-storybook/graph/badge.svg)](https://codecov.io/github/stayhydated/gpui-storybook)
[![Book](https://img.shields.io/badge/book-online-blue)](https://stayhydated.github.io/gpui-storybook/book/)
[![crates.io](https://img.shields.io/crates/v/gpui-storybook-mcp.svg)](https://crates.io/crates/gpui-storybook-mcp)

`gpui-storybook-mcp` exposes typed MCP tools and PNG capture for a live GPUI
Storybook window on Linux and macOS. Applications enable it through the facade's
`mcp` feature. The standard gallery and dock views attach the controller created
during `gpui_storybook::init`.

## Start a session

On Linux, run the application through the Sway-backed launcher:

```bash
GPUI_STORYBOOK_MCP_STDIO=1 \
gpui-storybook-launch -- cargo run -p my-app-storybook --features mcp
```

On macOS, run the Cargo command directly. Linux requires Sway and Mesa software
graphics drivers; macOS captures through GPUI's native image renderer. The
`mcp` feature produces a compile-time error on Windows.

Send logs to standard error. The first tool call waits up to 30 seconds for the
live host, and closing standard input ends the GUI session.

## Control and capture a story

Use `storybook_list_stories` to discover stable keys and `storybook_open_story`
to select one. Read its control specs with `storybook_read_controls`, then pass
a tagged value to `storybook_set_control`:

```json
{
  "control_key": "disabled",
  "value": { "type": "boolean", "value": true }
}
```

`storybook_capture_current_story` accepts a control map, a named viewport, or
positive paired dimensions. Captures contain the selected story or substory
region, with the surrounding Storybook chrome cropped out.

For a new process, `storybook_capture_launch_env` returns a platform command and
an `env` map. Merge that map into the child environment before running the
command. Linux commands use `gpui-storybook-launch`; macOS commands use Cargo.

## Enable interaction

Set `GPUI_STORYBOOK_MCP_ALLOW_INTERACTION=1` before starting the server to expose
action discovery, semantic-target clicks, scenarios, and ordered input batches.
These tools can trigger application effects; use a suitable backend or fixture.

- Discover actions and argument schemas with `storybook_list_actions` after
  each launch.
- Mark visible controls with `StorybookElementExt::storybook_target()` and use
  their route-local keys with `storybook_click_target`.
- Register rendered state with `storybook_value(&state)`. Read it with
  `storybook_read_value`, or use `storybook_wait_for_value` for a bounded JSON
  postcondition. Semantic reads are available without the interaction gate.
- Use `storybook_run_scenario` for a declared workflow that recreates the story,
  or `storybook_run_steps` for an ordered batch against the active route.

Batches validate input before dispatch and serialize mutations with capture and
navigation. Reads remain available during a batch and may observe intermediate
state. A partial failure reports the dispatched-step count and is never retried
automatically.

Direct integrations can use `StorybookMcpServerOptions::with_interaction`,
`server_with_options`, or `tool_registry_with_options`. Retain the automation
handle for the lifetime of the host.
