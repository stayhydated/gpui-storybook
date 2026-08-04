# Automation and capture

Read this reference when enabling MCP, selecting routes, launching captures, or
diagnosing automation.

## Enable automation

Forward the facade feature:

```toml
[features]
mcp = ["gpui-storybook/mcp"]
```

Set `GPUI_STORYBOOK_MCP_STDIO=1` to serve MCP over stdio. Route tracing and
diagnostic logs to standard error.

The standard `Gallery::view` and `StoryWorkspace::view` constructors attach
the controller installed by `gpui_storybook::init`.

## MCP tools

- `storybook_list_stories`
- `storybook_get_story`
- `storybook_current_story`
- `storybook_open_story`
- `storybook_capture_current_story`
- `storybook_capture_launch_env`

Use advertised typed fields. Width and height are optional only as a pair.

## Routes

Base routes use `{cargo-package-name}-{registered-type-name}`. Substory routes
append `/substory-key`.

Discover active base routes with `storybook_list_stories`. Discover a
substory suffix from its `Substory` enum, string section title, or custom
`StorySectionBase`.

## Startup capture

Launch an MCP-enabled binary with:

```bash
WGPU_CAPTURE_ROUTE=my-app-storybook-ButtonStory \
WGPU_CAPTURE_PATH=target/storybook-captures/button.png \
cargo run -p my-app-storybook --features mcp
```

Optional variables:

- `WGPU_CAPTURE_WIDTH` and `WGPU_CAPTURE_HEIGHT`: positive paired values;
- `WGPU_CAPTURE_FRAME`: positive one-based frame gate.

Capture startup disables persistence and forces light appearance, the
`Default Light` theme, and the typed fallback language. Stdio-only startup
uses the same presentation with temporary storage.

Captures exclude gallery or dock chrome. A substory route crops to its section.
Use returned pixel dimensions rather than requested dimensions as the rendered
source of truth.

## Failure checks

- Route missing: inspect active filters and discover the base key again.
- No live host: await initialization and construct a standard view.
- Capture pending: wait before submitting another screenshot.
- Invalid dimensions: provide positive width and height together.
- Corrupt stdio: move application logs to standard error.
- Startup timeout: confirm story registration linkage and open the route
  interactively.
