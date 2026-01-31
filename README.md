# GPUI Storybook

[![Build Status](https://github.com/stayhydated/gpui-storybook/actions/workflows/ci.yml/badge.svg)](https://github.com/stayhydated/gpui-storybook/actions/workflows/ci.yml)
[![Docs](https://docs.rs/gpui-storybook/badge.svg)](https://docs.rs/gpui-storybook/)
[![Crates.io](https://img.shields.io/crates/v/gpui-storybook.svg)](https://crates.io/crates/gpui-storybook)

A storybook-style workspace for building and inspecting GPUI components, with built-in theming, i18n, and a searchable gallery.

## Features

- Gallery UI with search, sections, and active story focus.
- `Story` trait and `StoryContainer` wrapper for consistent rendering.
- Attribute macros to register stories and global init hooks.
- Theme switching and appearance controls (mode, font size, radius, scrollbar).
- Locale management wired to es-fluent and gpui-component.
- Asset loading that merges local assets with gpui-component icons.

## Installation

```toml
[dependencies]
gpui = { git = "https://github.com/zed-industries/zed" }
gpui-component = { git = "https://github.com/longbridge/gpui-component" }
gpui-storybook = "0.5"
```

The default feature set enables the story registration macros. Disable `default-features` if you want the runtime only.

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

`gpui_storybook::init` runs `gpui_component::init` and the storybook runtime setup for you.

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

## Themes and appearance

The storybook window exposes theme and appearance controls:

- Light/dark mode and theme selection.
- Font size, radius, and scrollbar visibility.
- Theme selection and scrollbar visibility persist to `target/state.json` (font size and radius are session-only).

## Localization

`gpui-storybook` wires locale selection into es-fluent and gpui-component:

- Implement `Language` with `strum::EnumIter` and `FluentDisplay`.
- Call `gpui_storybook::init` once to register the locale manager.
- Call `gpui_storybook::change_locale` after `init` (and whenever you switch languages).

## Assets

`gpui_storybook::Assets` merges:

- Local embedded assets from `crates/gpui-storybook-core/assets`.
- `gpui-component` icon assets under the `icons/` prefix.

## Example app

```bash
cargo run -p gpui-storybook-example
```

## Crate layout

- `gpui-storybook`: Public API and re-exports.
- `gpui-storybook-core`: Gallery UI, story panels, theming, i18n, assets.
- `gpui-storybook-macros`: Proc macros for `#[story]` and `#[story_init]`.

## Compatibility

| `gpui-storybook` | `gpui-component` |
| :--------------- | :--------------- |
| **git** | |
| `master` | `main` |
| **crates.io** | |
| `0.5.x` | `0.5.x` |

## Acknowledgements

This project is heavily inspired by the story section of [gpui-component](https://github.com/longbridge/gpui-component/tree/main/crates/story).

See related discussion on ownership transfer [here](https://github.com/longbridge/gpui-component/discussions/1473).
