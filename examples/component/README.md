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

## What to inspect

- `src/main.rs`: app startup, embedded i18n module setup, locale initialization, and window creation
- `src/lib.rs`: shared `StorySection` enum for stable ordering
- `src/components/*.rs`: components annotated with `#[derive(ComponentStory)]`
- `storybook.toml`: crate-level runtime group for discovery

## Core pattern

```rs
use gpui::{IntoElement, RenderOnce};

#[derive(IntoElement, gpui_storybook::ComponentStory)]
#[storybook(
    title = "Welcome Card",
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

## Locale setup

The binary defines its embedded i18n module, derives the app language enum with `EsFluent`, initializes storybook with the default language, and selects the active locale:

```rs
es_fluent_manager_embedded::define_i18n_module!();

#[es_fluent_language]
#[derive(Clone, Copy, Debug, EnumIter, EsFluent, PartialEq)]
pub enum Languages {}

gpui_storybook::init(cx, Languages::default());
gpui_storybook::change_locale(cx, Languages::default()).unwrap();
```

## Example config

```toml
group = "gpui-storybook-example-component"
```

`allow` is intentionally omitted, so the example includes only its own `group`.
