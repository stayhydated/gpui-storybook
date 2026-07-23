# gpui-storybook-mcp

Public integration crate for MCP-based storybook automation.

Most applications should enable the `mcp` feature on `gpui-storybook` instead
of depending on this crate directly.

## What it provides

- `StorybookAutomation`, re-exported from `gpui-storybook-core`
- stdio MCP serving with `start_stdio(...)`
- environment-driven capture startup with `start_capture_session_from_env(...)`
- capture launch environment helpers for external tools
- MCP tools for listing, opening, inspecting, and capturing stories

## Facade usage

```toml
[dependencies]
gpui-storybook = { version = "*", features = ["mcp"] }
```

```rs
app_cx.spawn(async move |cx| {
    let _ready = readiness.await;
    cx.update(|app_cx| {
        gpui_storybook::create_new_window("Stories", move |window, cx| {
            let stories = gpui_storybook::generate_stories(window, cx);
            gpui_storybook::Gallery::view(stories, None, window, cx)
        }, app_cx);
    });
}).detach();
```

Pass the same typed `StorybookOptions` used by an interactive Storybook to
`gpui_storybook::init(...)`, await its readiness task, and only then construct
the gallery or dock window. Initialization installs MCP automation when the
feature is enabled. Use `StoryWorkspace::view(...)` for dock mode. The explicit
`view_with_automation(...)` constructors remain available for direct core users
or custom controllers.

## MCP tools

- `storybook_list_stories`
- `storybook_get_story`
- `storybook_current_story`
- `storybook_open_story`
- `storybook_capture_current_story`
- `storybook_capture_launch_env`

Tool inputs use the stable story key emitted by registration macros:
`{crate-package-name}-{registered-story-name}`. Explicit `#[story]` entries use
the story struct name. `ComponentStory` entries use the component type name.
Every tool advertises a closed input schema, a structured output schema, and
MCP behavior annotations. Missing required fields, unknown arguments, invalid
story keys, and invalid capture options return machine-readable structured
errors.

## Environment capture

Run a storybook binary with a capture route to open and capture one story:

```bash
WGPU_CAPTURE_ROUTE=gpui-storybook-example-story-ButtonStory \
WGPU_CAPTURE_PATH=target/storybook-captures/button.png \
cargo run -p gpui-storybook-example-story --features mcp
```

Optional environment variables:

- `WGPU_CAPTURE_FRAME`
- `WGPU_CAPTURE_WIDTH`
- `WGPU_CAPTURE_HEIGHT`
- `GPUI_STORYBOOK_MCP_STDIO=1`

`WGPU_CAPTURE_FRAME` must be greater than zero. Width and height must be set
together and greater than zero; they request a live window resize before
capture. Captures are cropped to the story view, excluding the sidebar and
storybook header or dock chrome. The capture result reports the actual rendered
pixel size, which can differ on scaled or compositor-managed displays.

The facade derives its automation preference profile from these real launch
variables. A route or output path selects capture mode: persistence is disabled
and resolved presentation is forced to light appearance, the registered
`Default Light` theme, and the application's configured fallback language.
Stdio without capture uses the same deterministic presentation with temporary
storage. Capture takes precedence when both are requested, and neither profile
replaces the user's saved interactive intent.

Sub-story routes use `story-key/substory-key`. Plain string sections use
title-derived slugs through `gpui_storybook::capture_substory(...)`; sections
passed a `#[derive(gpui_storybook::Substory)]` enum variant use the variant's
stable key. For example:
`gpui-storybook-example-story-ButtonStory/with-progress`.
