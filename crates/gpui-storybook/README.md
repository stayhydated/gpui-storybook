# GPUI Storybook

[![Build Status](https://github.com/stayhydated/gpui-storybook/actions/workflows/ci.yml/badge.svg)](https://github.com/stayhydated/gpui-storybook/actions/workflows/ci.yml)
[![Docs](https://docs.rs/gpui-storybook/badge.svg)](https://docs.rs/gpui-storybook/)
[![Crates.io](https://img.shields.io/crates/v/gpui-storybook.svg)](https://crates.io/crates/gpui-storybook)

A storybook-style workspace for building and inspecting GPUI components, with built-in theming, i18n, and a searchable gallery.

## Features

- Gallery UI with sidebar search, dock, and active story focus.
- Proc macros for attribute-based story registration, component-attached registration, and global init hooks.
- `Story` trait for full control when a component needs custom story behavior.

## Compatibility

| `gpui-storybook` | `gpui-component` | `gpui` |
| :--------------- | :--------------- | :--------------------------------------------- |
| **git** | |
| `master` | `main` | rev `15d8660748b508b3525d3403e5d172f1a557bfa5` |
| **crates.io** | |
| `0.5.x` | `0.5.x` | |

## Example apps

Story-struct pattern:

```bash
cargo run -p gpui-storybook-example-story
```

Component-derived pattern:

```bash
cargo run -p gpui-storybook-example-component
```

With dock layout:

```bash
cargo run -p gpui-storybook-example-story --features dock
cargo run -p gpui-storybook-example-component --features dock
```

## Quick start

```rust
use gpui::Application;
use gpui_storybook::{Assets, Gallery};

fn main() {
    let app = Application::new().with_assets(Assets);

    app.run(|cx| {
        gpui_storybook::init(MyLanguage::default(), cx);

        gpui_storybook::create_new_window("My App - Stories", |window, cx| {
            let stories = gpui_storybook::generate_stories(window, cx);
            Gallery::view(stories, None, window, cx)
        }, cx);
    });
}
```

## Registering stories with `#[story]`

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

`ComponentStory` generates the wrapper `Story`, `Render`, and `Focusable` implementations that storybook needs. The component only defines its own component rendering and, when needed, an `example = ...` constructor.

`title` and `description` accept expressions that convert into `String`, so both string literals and `String::from(...)`-style values work.

Use `#[gpui_storybook::story_init]` to register global setup functions that should run once per app:

```rust
#[gpui_storybook::story_init]
fn register_icons(cx: &mut gpui::App) {
    // global setup
}
```

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

#[derive(gpui::IntoElement, gpui_storybook::ComponentStory)]
#[storybook(section = StorySection::Patterns, example = PatternCard::example())]
pub struct PatternCard;
```

## Crate-level story discovery config

You can add a `storybook.toml` file to a crate root to control what `generate_stories` includes from that crate:

```toml
group = "UI Kit"
allow = ["UI Kit"]
disable_story = ["CardStory"]
```

- `group`: Required runtime discovery group when `storybook.toml` exists; used for `allow` matching and as the top-level sidebar bucket without overwriting a story's declared section beneath it.
- `allow`: Optional list of allowed group identifiers for the current app/runtime.
- omit `allow`: Allows only the config's own `group`.
- `allow = ["*"]`: Includes all groups.
- `allow = []`: Includes none.
- `disable_story`: Optional per-story denylist by registered story type name.

## Acknowledgements

This project is heavily inspired by the story section of [gpui-component](https://github.com/longbridge/gpui-component/tree/main/crates/story).

See related discussion on ownership transfer [here](https://github.com/longbridge/gpui-component/discussions/1473).
