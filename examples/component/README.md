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

- `build.rs`: rebuild tracking for embedded locale assets
- `src/i18n.rs`: embedded i18n module and typed language enum
- `src/main.rs`: stable consumer options, readiness-before-window startup,
  diagnostic reporting, and gallery/dock window creation
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

The example build script tracks locale assets, the library defines its embedded
i18n module, typed language enum, and GPUI locale adapter in `src/i18n.rs`, and
the binary passes a stable consumer ID plus typed fallback to Storybook:

```rs
// build.rs
fn main() {
    es_fluent_build::track_i18n_assets();
}

// src/i18n.rs
es_fluent_manager_embedded::define_i18n_module!();

#[es_fluent_language]
#[derive(Clone, Copy, Debug, EnumIter, EsFluent, PartialEq)]
pub enum Languages {}

pub fn apply_locale(
    language: Languages,
    cx: &mut gpui::App,
) -> Result<(), gpui_es_fluent::EmbeddedInitError> {
    let _linked_module = &GPUI_STORYBOOK_EXAMPLE_COMPONENT_I18N_MODULE;
    gpui_es_fluent::replace_with_language(cx, language)
}

// src/main.rs
use gpui_storybook::{ConsumerId, StorybookOptions};
use gpui_storybook_example_component::i18n::{self, Languages};

const CONSUMER_ID: &str = "gpui-storybook-example-component";

let consumer_id = match ConsumerId::new(CONSUMER_ID) {
    Ok(consumer_id) => consumer_id,
    Err(error) => {
        tracing::error!(error = %error, "invalid Storybook consumer id");
        app_cx.quit();
        return;
    },
};
let options = StorybookOptions::new(
    consumer_id,
    Languages::default(),
    i18n::apply_locale,
);
let readiness = match gpui_storybook::init(app_cx, options) {
    Ok(readiness) => readiness,
    Err(error) => {
        tracing::error!(error = %error, "failed to initialize Storybook");
        app_cx.quit();
        return;
    },
};

app_cx.spawn(async move |cx| {
    let ready = readiness.await;
    if !ready.diagnostics.is_empty() {
        tracing::warn!(
            persistence_status = ?ready.persistence_status,
            diagnostics = ?ready.diagnostics,
            "component example initialized with preference diagnostics"
        );
    }
    cx.update(|app_cx| {
        // Construct the gallery or dock window only after readiness.
    });
}).detach();
```

The same-module static reference keeps this example's generated Fluent module
linked before the consumer manager is installed. Storybook localizes its shell
through a separate manager and uses embedded English when the selected consumer
locale is unavailable to the shell.

The consumer ID remains stable across launches and differs from the explicit
story example's ID, keeping their workspace-local
`.gpui-storybook/{consumer-id}.json` files and rows isolated. Readiness applies
saved intent before the first frame. Storage and locale diagnostics are
reported independently; a storage error still allows the example to open with
resolved fallbacks.

## Example config

```toml
group = "gpui-storybook-example-component"

# Optional launch-only presentation overrides:
# [overrides]
# color_scheme = "dark"
# theme = "Default Dark"
# language = "en"
```

`init` and `generate_stories` use this file because the package name matches the
running binary name.
`allow` is intentionally omitted, so the example includes only its own `group`.
The commented `[overrides]` table shows how to bypass system appearance and
locale detection without replacing saved intent. `theme` names a registered
theme for the effective color scheme, and `language` must be one of the
example's typed embedded BCP 47 languages.

## Capture a story

The `mcp` feature enables live story automation and PNG capture. This example
wires `StorybookAutomation` into both gallery and dock modes.
The stdio tools expose typed input/output schemas and structured argument
errors, so clients can discover story keys and capture options from MCP tool
metadata.

```bash
WGPU_CAPTURE_ROUTE=gpui-storybook-example-component-WelcomeCard \
WGPU_CAPTURE_PATH=target/storybook-captures/welcome-card.png \
cargo run -p gpui-storybook-example-component --features mcp
```

This capture launch uses disabled preference storage and deterministic light,
`Default Light`, and fallback-language overrides. Stdio-only MCP startup uses
the same presentation with temporary storage, so neither automation path
overwrites the example's interactive saved intent.

Add `WGPU_CAPTURE_WIDTH` and `WGPU_CAPTURE_HEIGHT` together to request a live
window resize before capture; both values must be greater than zero. The
returned capture metadata reports the actual rendered pixel size.
