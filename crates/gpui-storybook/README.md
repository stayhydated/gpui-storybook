# GPUI Storybook

[![Build Status](https://github.com/stayhydated/gpui-storybook/actions/workflows/ci.yml/badge.svg)](https://github.com/stayhydated/gpui-storybook/actions/workflows/ci.yml)
[![Docs](https://docs.rs/gpui-storybook/badge.svg)](https://docs.rs/gpui-storybook/)
[![Crates.io](https://img.shields.io/crates/v/gpui-storybook.svg)](https://crates.io/crates/gpui-storybook)

A storybook-style workspace for building and inspecting GPUI components, with built-in theming, i18n, and a searchable gallery.

## Features

- Gallery UI with sidebar search, dock, and active story focus.
- Attribute macros to register stories and global init hooks and `Story` trait.

## Compatibility

| `gpui-storybook` | `gpui-component` | `gpui` |
| :--------------- | :--------------- | :--------------------------------------------- |
| **git** | |
| `master` | `main` | rev `15d8660748b508b3525d3403e5d172f1a557bfa5` |
| **crates.io** | |
| `0.5.x` | `0.5.x` | |

## Example app

```bash
cargo run
```

with dock layout

```bash
cargo run --features dock
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

## Crate-level story discovery config

You can add a `storybook.toml` file to a crate root to control what `generate_stories` includes from that crate:

```toml
group = "UI Kit"
allow = ["ButtonStory", "CardStory"]
```

- `group`: Required section/group name when `storybook.toml` exists; applied to all stories in that crate.
- `allow`: Story struct names to include.
- `allow = ["*"]`: Includes all stories from that crate.
- `allow = []`: Includes none from that crate.

## Acknowledgements

This project is heavily inspired by the story section of [gpui-component](https://github.com/longbridge/gpui-component/tree/main/crates/story).

See related discussion on ownership transfer [here](https://github.com/longbridge/gpui-component/discussions/1473).
