# gpui-storybook

[![Docs](https://docs.rs/gpui-storybook/badge.svg)](https://docs.rs/gpui-storybook/)
[![Crates.io](https://img.shields.io/crates/v/gpui-storybook.svg)](https://crates.io/crates/gpui-storybook)

`gpui-storybook` is the user-facing crate for a storybook-style GPUI workflow. It re-exports the runtime types and (by default) the proc macros for registering stories.

## Installation

```toml
[dependencies]
gpui = { git = "https://github.com/zed-industries/zed" }
gpui-component = { git = "https://github.com/longbridge/gpui-component" }
gpui-storybook = "0.5"
```

Disable default features if you want the runtime without macros:

```toml
[dependencies]
gpui-storybook = { version = "0.5", default-features = false }
```

## Usage

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
#[gpui_storybook::story("Components")]
pub struct ButtonStory;
```

```rust
#[gpui_storybook::story_init]
fn register_icons(cx: &mut gpui::App) {
    // global setup
}
```

## Sections

Use string literals or enum variants to group stories. Enum variants provide stable ordering by discriminant:

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
