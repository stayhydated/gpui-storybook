# gpui-storybook-example-story

User-facing example app for the explicit `#[story]` + `Story` workflow.

Use this example when the story itself needs to own state, focus, or additional wrapper UI around the component being previewed.

## Run it

```bash
cargo run -p gpui-storybook-example-story
```

With the dock workspace:

```bash
cargo run -p gpui-storybook-example-story --features dock
```

With MCP automation and capture helpers:

```bash
cargo run -p gpui-storybook-example-story --features mcp
GPUI_STORYBOOK_MCP_STDIO=1 cargo run -p gpui-storybook-example-story --features mcp
```

## What to inspect

- `src/main.rs`: app startup, embedded i18n module setup, locale initialization, feature-gated MCP automation, and window creation
- `src/lib.rs`: shared `StorySection` enum for stable ordering
- `src/stories/*.rs`: explicit story structs and `impl gpui_storybook::Story`
- `src/stories/grouped_story.rs`: two story structs with the same title, grouped into one sidebar item
- `src/stories/custom_section_story.rs`: custom section component built on `StorySectionBase`
- `storybook.toml`: crate-level runtime group for discovery

## Core pattern

```rs
#[gpui_storybook::story(crate::StorySection::Buttons)]
pub struct ButtonStory;

impl gpui_storybook::Story for ButtonStory {
    fn title(_: &App) -> String {
        "Button".into()
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render + Focusable> {
        Self::view(window, cx)
    }
}
```

This flow is the right fit when a story is more than "render the component with example data".
`title` and `description` receive `&App`, so explicit stories can localize metadata with `gpui_storybook::localize_message(cx, ...)`.

The stable automation key for this story is
`gpui-storybook-example-story-ButtonStory`: the package name plus the registered
story struct name.

## Grouped title pattern

Stories in the same group and section that return the same `title` are shown on
one storybook page. `grouped_story.rs` registers `GroupedSummaryStory` and
`GroupedDetailsStory`; both return `"Grouped Story"` from `title`, while their
`description` values identify the individual variants in the combined page.

## Locale setup

The example library defines its embedded i18n module in `src/i18n.rs`, derives the app language enum with `EsFluent`, and the binary initializes Storybook with the default language before selecting the active locale:

```rs
// src/i18n.rs
es_fluent_manager_embedded::define_i18n_module!();

#[es_fluent_language]
#[derive(Clone, Copy, Debug, EnumIter, EsFluent, PartialEq)]
pub enum Languages {}

// src/main.rs
use gpui_storybook_example_story::i18n::Languages;

gpui_storybook::init(cx, Languages::default());
gpui_storybook::change_locale(cx, Languages::default()).unwrap();
```

## Example config

```toml
group = "gpui-storybook-example-story"
```

`generate_stories` uses this file because the package name matches the running binary name.
`allow` is intentionally omitted, so the example includes only its own `group`.

## Capture a story

The `mcp` feature enables live story automation and PNG capture. This example
wires `StorybookAutomation` into both gallery and dock modes.
The stdio tools expose typed input/output schemas and structured argument
errors, so clients can discover story keys and capture options from MCP tool
metadata.

```bash
WGPU_CAPTURE_ROUTE=gpui-storybook-example-story-ButtonStory \
WGPU_CAPTURE_PATH=target/storybook-captures/button.png \
cargo run -p gpui-storybook-example-story --features mcp
```

Add `WGPU_CAPTURE_WIDTH` and `WGPU_CAPTURE_HEIGHT` together to request a live
window resize before capture; both values must be greater than zero. Captures
are cropped to the story page, excluding the sidebar and storybook header. The
returned capture metadata reports the actual rendered pixel size.

The facade `gpui_storybook::section(...)` helper renders the standard styled
section and registers capture sub-routes for each section. It accepts plain
strings and `#[derive(gpui_storybook::Substory)]` enum variants; enum variants
keep capture keys stable even if visible section titles change. Custom section
components can store `gpui_storybook::StorySectionBase` and call
`base.capture(...)` from `RenderOnce` to reuse the same capture metadata without
the standard section styling. For the Button story, use routes such as:

```bash
WGPU_CAPTURE_ROUTE=gpui-storybook-example-story-ButtonStory/normal-button
WGPU_CAPTURE_ROUTE=gpui-storybook-example-story-ButtonStory/button-with-icon
WGPU_CAPTURE_ROUTE=gpui-storybook-example-story-ButtonStory/with-progress
```

```rust
#[derive(gpui_storybook::Substory)]
enum ButtonSubstory {
    NormalButton,
    #[substory(title = "Button with Icon")]
    ButtonWithIcon,
    #[substory(title = "With Progress")]
    WithProgress,
}
```

`custom_section_story.rs` shows the custom component form. Its
`metric_section(...)` constructor stores `StorySectionBase`; the component
renders its own section layout, then calls `base.capture(...)` from
`RenderOnce`. Its capture routes include:

```bash
WGPU_CAPTURE_ROUTE=gpui-storybook-example-story-CustomSectionStory/product-metrics
WGPU_CAPTURE_ROUTE=gpui-storybook-example-story-CustomSectionStory/health-signals
```
