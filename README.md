# GPUI Storybook

[![Build Status](https://github.com/stayhydated/gpui-storybook/actions/workflows/ci.yml/badge.svg)](https://github.com/stayhydated/gpui-storybook/actions/workflows/ci.yml)
[![Docs](https://docs.rs/gpui-storybook/badge.svg)](https://docs.rs/gpui-storybook/)
[![Crates.io](https://img.shields.io/crates/v/gpui-storybook.svg)](https://crates.io/crates/gpui-storybook)

A storybook-style workspace for building and inspecting GPUI components, with built-in theming, locale switching, per-crate story filtering, and both gallery and docked layouts.

## Workspace

| Package | Role |
| :------ | :--- |
| `gpui-storybook` | Facade crate that re-exports the runtime and macros. |
| `gpui-storybook-core` | Runtime UI: gallery, dock workspace, title bar, theming, i18n, and assets. |
| `gpui-storybook-macros` | `#[story]`, `#[derive(ComponentStory)]`, and `#[story_init]`. |
| `gpui-storybook-components` | Shared dock-sidebar UI pieces used by the runtime. |
| `gpui-storybook-toml` | Loader for crate-local `storybook.toml` discovery config. |
| `gpui-storybook-example-story` | Example app using explicit story structs. |
| `gpui-storybook-example-component` | Example app using `#[derive(ComponentStory)]`. |

## Highlights

- Searchable sidebar gallery for story browsing.
- Optional dock workspace behind the `dock` feature.
- Story registration through either `#[story]` or `#[derive(ComponentStory)]`.
- Global setup hooks through `#[story_init]`.
- `storybook.toml` filtering by group and disabled story names.
- Theme, locale, and title-bar controls built into the runtime.

## Example apps

```bash
cargo run -p gpui-storybook-example-story
cargo run -p gpui-storybook-example-component
```

With dock layout:

```bash
cargo run -p gpui-storybook-example-story --features dock
cargo run -p gpui-storybook-example-component --features dock
```

## Quick start

```rust
use es_fluent::EsFluent;
use es_fluent_lang::es_fluent_language;
use gpui_storybook::{Assets, Gallery};
use strum::EnumIter;

#[es_fluent_language]
#[derive(Clone, Copy, Debug, EnumIter, EsFluent, PartialEq)]
pub enum Languages {}

fn main() {
    let app = gpui_platform::application().with_assets(Assets);

    app.run(|cx| {
        gpui_storybook::init(Languages::default(), cx);
        gpui_storybook::change_locale(Languages::default());

        gpui_storybook::create_new_window("My App - Stories", |window, cx| {
            let stories = gpui_storybook::generate_stories(window, cx);
            Gallery::view(stories, None, window, cx)
        }, cx);
    });
}
```

## Registering stories

```rust
use gpui::{App, Focusable, Render, Window};

#[gpui_storybook::story("Components")]
pub struct ButtonStory;

impl gpui_storybook::Story for ButtonStory {
    fn title() -> String {
        "Button".into()
    }

    fn new_view(window: &mut Window, cx: &mut App) -> gpui::Entity<impl Render + Focusable> {
        Self::view(window, cx)
    }
}
```

Use `#[gpui_storybook::story_init]` to register global setup functions that should run once per app:

```rust
#[gpui_storybook::story_init]
fn register_icons(cx: &mut gpui::App) {
    // global setup
}
```

## Registering components directly with `ComponentStory`

```rust
use gpui::{App, IntoElement, RenderOnce, Window};

#[derive(IntoElement, gpui_storybook::ComponentStory)]
#[storybook(
    title = "Welcome Card",
    section = crate::StorySection::Intro,
    example = WelcomeCard::example(),
)]
pub struct WelcomeCard {
    title: gpui::SharedString,
}

impl WelcomeCard {
    pub fn example() -> Self {
        Self {
            title: "Component Registration".into(),
        }
    }
}

impl RenderOnce for WelcomeCard {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        self.title
    }
}
```

`ComponentStory` generates the internal `Story`, `Render`, and `Focusable` wrapper that storybook needs. The component keeps its own rendering logic and can optionally supply an `example = ...` expression instead of relying on `Default`.

## Organizing sections

Sections can be string literals or enum variants. Enum variants are ordered by discriminant to produce stable section ordering:

```rust
#[derive(Clone, Copy)]
enum StorySection {
    Basics,
    Components,
    Patterns,
}

#[gpui_storybook::story(StorySection::Components)]
pub struct CardStory;
```

`ComponentStory` accepts the same section forms through `#[storybook(section = ...)]`.

## `storybook.toml`

Add `storybook.toml` to a crate root when you want that crate to participate in runtime filtering:

```toml
group = "UI Kit"
disable_story = ["LegacyCardStory"]
```

- `group`: Required when `storybook.toml` exists. Used for runtime filtering and top-level sidebar grouping.
- Omit `allow`: Only the crate's own `group` is included.
- `allow = ["Shared", "UI Kit"]`: Include specific groups.
- `allow = ["*"]`: Include every group.
- `allow = []`: Include none.
- `disable_story`: Optional denylist by registered story type name.

At runtime, `generate_stories` prefers the `storybook.toml` associated with the current binary crate and falls back to searching upward from the working directory.

## Acknowledgements

This project is heavily inspired by the story section of [gpui-component](https://github.com/longbridge/gpui-component/tree/main/crates/story).

See related discussion on ownership transfer [here](https://github.com/longbridge/gpui-component/discussions/1473).
