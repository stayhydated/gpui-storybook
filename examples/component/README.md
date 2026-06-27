# gpui-storybook-example-component

User-facing example app for the component-attached `#[derive(ComponentStory)]` workflow.

Use this example when the component should own only its own data and rendering, while storybook generates the wrapper view and registration glue.

## Run it

```bash
cargo run -p gpui-storybook-example-component
```

With the dock workspace:

```bash
cargo run -p gpui-storybook-example-component --features dock
```

With MCP automation and capture helpers:

```bash
cargo run -p gpui-storybook-example-component --features mcp
GPUI_STORYBOOK_MCP_STDIO=1 cargo run -p gpui-storybook-example-component --features mcp
```

## What to inspect

- `src/main.rs`: app startup, embedded i18n module setup, locale initialization, feature-gated MCP automation, and window creation
- `src/lib.rs`: shared `StorySection` enum for stable ordering and `StoryItems` i18n messages
- `src/components/*.rs`: components annotated with `#[derive(ComponentStory)]`
- `storybook.toml`: crate-level runtime group for discovery

## Core pattern

```rs
use es_fluent::EsFluent;
use gpui::{IntoElement, RenderOnce};

#[derive(EsFluent)]
pub enum StoryItems {
    Title,
}

#[derive(IntoElement, gpui_storybook::ComponentStory)]
#[storybook(
    title = gpui_storybook::localize_message(cx, &crate::StoryItems::Title)
        .unwrap_or_else(|| "Title".into()),
    section = crate::StorySection::Intro,
    example = WelcomeCard::example(),
)]
pub struct WelcomeCard {
    // component data only
}

impl RenderOnce for WelcomeCard {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        // component render only
    }
}
```

This flow keeps the storybook wrapper out of the component implementation. The component stays focused on its example data and markup.
`title` and `description` expressions are emitted inside methods that receive `cx: &App`, so component stories can localize metadata without adding a custom wrapper.

The stable automation key for this component story is
`gpui-storybook-example-component-WelcomeCard`: the package name plus the
component type name.

## Locale setup

The example library defines its embedded i18n module in `src/i18n.rs`, derives the app language enum with `EsFluent`, and the binary initializes Storybook with the default language before selecting the active locale:

```rs
// src/i18n.rs
es_fluent_manager_embedded::define_i18n_module!();

#[es_fluent_language]
#[derive(Clone, Copy, Debug, EnumIter, EsFluent, PartialEq)]
pub enum Languages {}

// src/main.rs
use gpui_storybook_example_component::i18n::Languages;

gpui_storybook::init(cx, Languages::default());
gpui_storybook::change_locale(cx, Languages::default()).unwrap();
```

## Example config

```toml
group = "gpui-storybook-example-component"
```

`generate_stories` uses this file because the package name matches the running binary name.
`allow` is intentionally omitted, so the example includes only its own `group`.

## Capture a story

The `mcp` feature enables live story automation and PNG capture. This example
wires `StorybookAutomation` into both gallery and dock modes.

```bash
WGPU_CAPTURE_ROUTE=gpui-storybook-example-component-WelcomeCard \
WGPU_CAPTURE_PATH=target/storybook-captures/welcome-card.png \
cargo run -p gpui-storybook-example-component --features mcp
```

Add `WGPU_CAPTURE_WIDTH` and `WGPU_CAPTURE_HEIGHT` together to request a live
window resize before capture; both values must be greater than zero. The
returned capture metadata reports the actual rendered pixel size.
