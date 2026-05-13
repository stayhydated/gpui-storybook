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

## What to inspect

- `src/main.rs`: app startup, embedded i18n module setup, locale initialization, and window creation
- `src/lib.rs`: shared `StorySection` enum for stable ordering
- `src/stories/*.rs`: explicit story structs and `impl gpui_storybook::Story`
- `storybook.toml`: crate-level runtime group for discovery

## Core pattern

```rs
#[gpui_storybook::story(crate::StorySection::Buttons)]
pub struct ButtonStory;

impl gpui_storybook::Story for ButtonStory {
    fn title() -> String {
        "Button".into()
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render + Focusable> {
        Self::view(window, cx)
    }
}
```

This flow is the right fit when a story is more than "render the component with example data".

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
group = "gpui-storybook-example-story"
```

`allow` is intentionally omitted, so the example includes only its own `group`.
