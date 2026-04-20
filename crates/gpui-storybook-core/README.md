# gpui-storybook-core

[![Docs](https://docs.rs/gpui-storybook-core/badge.svg)](https://docs.rs/gpui-storybook-core/)
[![Crates.io](https://img.shields.io/crates/v/gpui-storybook-core.svg)](https://crates.io/crates/gpui-storybook-core)

Public integration crate for the storybook runtime used by `gpui-storybook`.

This crate is for applications that need runtime-level control over the window shell, gallery, dock workspace, or title-bar customization. Most applications should start with [`gpui-storybook`](../gpui-storybook/README.md) instead.

## What it provides

- `Gallery` for the searchable sidebar and active-story layout
- `StoryContainer` and the `Story` runtime contract
- `create_new_window` and `create_new_window_with_ui` for the standard storybook shell
- `StorybookWindowUi` for custom app-menu and title-bar additions
- `StoryWorkspace` and `create_dock_window` behind the `dock` feature
- built-in theme persistence, locale wiring, and embedded assets

## Typical direct use

```rs
use gpui::App;
use gpui_storybook_core::{
    gallery::Gallery,
    story::StoryContainer,
    story::{create_new_window_with_ui},
    storybook_window_ui::{StorybookWindow, StorybookWindowUi},
};

fn open_story_window(stories: Vec<gpui::Entity<StoryContainer>>, cx: &mut App) {
    let ui = StorybookWindowUi::new().with_app_menu_items(|_| Vec::new());

    create_new_window_with_ui("Stories", move |window, cx| {
        StorybookWindow::new(Gallery::view(stories.clone(), None, window, cx)).with_ui(ui)
    }, cx);
}
```

If you need compile-time story registration and `storybook.toml` filtering, use the facade crate instead of building directly on `gpui-storybook-core`.
