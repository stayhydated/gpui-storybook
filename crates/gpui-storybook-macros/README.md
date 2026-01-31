# gpui-storybook-macros

[![Docs](https://docs.rs/gpui-storybook-macros/badge.svg)](https://docs.rs/gpui-storybook-macros/)
[![Crates.io](https://img.shields.io/crates/v/gpui-storybook-macros.svg)](https://crates.io/crates/gpui-storybook-macros)

Proc macros for `gpui-storybook`. These macros register stories and init hooks using the inventory system.

Most users should enable the default features of `gpui-storybook`, which re-exports these macros.

## Installation

```toml
[dependencies]
gpui-storybook-macros = "0.5"
```

## `#[story]`

Registers a story struct. Optionally accept a section name as a string literal or enum variant:

```rust
#[gpui_storybook::story("Components")]
pub struct ButtonStory;
```

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

## `#[story_init]`

Registers a global setup function that runs once per app:

```rust
#[gpui_storybook::story_init]
fn register_icons(cx: &mut gpui::App) {
    // global setup
}
```
