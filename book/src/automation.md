# Automation and capture

Enable the `mcp` feature to inspect and open stories from another process or
to capture the rendered story region as a PNG. The standard gallery and dock
views attach the automation controller installed by `gpui_storybook::init`.

## Enable MCP support

Forward the facade feature from the Storybook package:

```toml
[features]
mcp = ["gpui-storybook/mcp"]
```

Run an MCP server over standard input and output:

```bash
GPUI_STORYBOOK_MCP_STDIO=1 \
cargo run -p my-app-storybook --features mcp
```

Send application logs to standard error so they do not corrupt the MCP
protocol stream.

A stdio launch uses temporary preference storage and a deterministic light
presentation with the `Default Light` theme and the application's fallback
language. It does not overwrite interactive preferences.

## Use the MCP tools

| Tool | Purpose |
|---|---|
| `storybook_list_stories` | List registered stories and stable route metadata |
| `storybook_get_story` | Inspect one story or substory route |
| `storybook_current_story` | Inspect the story displayed by the live window |
| `storybook_open_story` | Navigate the live window to a route |
| `storybook_read_controls` | Read control metadata and current values from the active variant |
| `storybook_set_control` | Set one control on the active story instance |
| `storybook_reset_control` | Reset one control, or all controls when `key` is omitted |
| `storybook_capture_current_story` | Capture the active story region |
| `storybook_capture_launch_env` | Build environment variables and a Cargo launch command |

Tool inputs and outputs use closed typed schemas. Use the advertised `key`,
`output_path`, `width`, `height`, `viewport`, `controls`, and launch properties;
unknown, missing, or invalid fields return structured errors.

## Reproduce a controlled story

Open the route before reading or changing its controls:

```json
{
  "key": "my-app-storybook-ButtonStory"
}
```

`storybook_read_controls` returns each `ControlSpec` with its current value.
Pass a tagged value to `storybook_set_control`:

```json
{
  "key": "disabled",
  "value": { "type": "boolean", "value": true }
}
```

Other value tags are `integer`, `float`, `text`, `color`, `choice`, and `json`.
A color value contains `h`, `s`, `l`, and `a` numbers. The setter enforces the
advertised bounds and select options before updating the concrete story entity.

Call `storybook_reset_control` with a `key` to reset one value, or with an empty
object to reset all active-story controls.

## Address stories by stable route

A macro-generated base route has this form:

```text
{cargo-package-name}-{registered-type-name}
```

For example:

```text
my-app-storybook-ButtonStory
```

A captureable section appends its substory key:

```text
my-app-storybook-ButtonStory/with-progress
```

Use `storybook_list_stories` for active base keys. The catalog does not
enumerate sections rendered inside a story, so inspect the `Substory` enum,
`section(...)` title, or custom `StorySectionBase` for the suffix.

## Capture during startup

Set a route and output path before launching an MCP-enabled binary:

```bash
WGPU_CAPTURE_ROUTE=my-app-storybook-ButtonStory \
WGPU_CAPTURE_PATH=target/storybook-captures/button.png \
cargo run -p my-app-storybook --features mcp
```

Storybook opens the route, creates missing parent directories, writes the PNG,
and exits after capture. Capture startup disables preference persistence and
uses the deterministic light presentation.

| Environment variable | Meaning |
|---|---|
| `WGPU_CAPTURE_ROUTE` | Story key or `story-key/substory-key` route |
| `WGPU_CAPTURE_PATH` | PNG destination; required to write a capture |
| `WGPU_CAPTURE_WIDTH` | Requested live window width in pixels |
| `WGPU_CAPTURE_HEIGHT` | Requested live window height in pixels |
| `WGPU_CAPTURE_FRAME` | Optional one-based frame gate |

Set width and height together, and make both values greater than zero.
`WGPU_CAPTURE_FRAME`, when present, must also be greater than zero.

## Capture a live session

Call `storybook_open_story` with a stable route, then call
`storybook_capture_current_story`. The capture tool accepts an optional
`output_path`, optional paired `width` and `height`, a named `viewport`, and a
`controls` object. The control map is applied immediately before capture:

```json
{
  "output_path": "target/storybook-captures/button.png",
  "viewport": "mobile",
  "controls": {
    "disabled": { "type": "boolean", "value": true },
    "padding": { "type": "float", "value": 16.0 }
  }
}
```

Named viewports are `responsive`, `mobile` (390×844), `tablet` (768×1024),
and `desktop` (1440×900). Explicit paired dimensions take precedence over a
named viewport.

Only one screenshot request can be pending at a time. A successful result
contains the request ID, actual path, rendered pixel dimensions, and story
metadata.

`storybook_capture_launch_env` can construct the environment and command for
an external launcher. It accepts a route plus optional output path, frame,
paired dimensions, package, binary, feature list, and stdio selection.
It also accepts a named viewport when paired dimensions are omitted.

## Understand capture bounds and size

Captures contain the story view, excluding the gallery sidebar and header or
the dock workspace chrome. Substory routes crop to the registered section
region.

Width and height request a live window resize. Display scaling or compositor
behavior can change the rendered result, so treat the returned `pixel_width`
and `pixel_height` as authoritative.

## Troubleshoot automation

| Symptom | Action |
|---|---|
| Route not found | Discover the base key with `storybook_list_stories`, inspect the substory definition, and check filtering |
| No live host is attached | Await initialization and construct a standard `Gallery` or `StoryWorkspace` view |
| Width or height is rejected | Set both dimensions to positive integers |
| A control is rejected | Read the current control specs and use the advertised type, bounds, and options |
| Capture is already pending | Wait for the active request to complete |
| Stdio messages cannot be decoded | Route tracing and diagnostics to standard error |
| Startup capture times out | Confirm registrations are linked and test the same route interactively |
