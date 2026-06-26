# gpui-storybook-core

[![Docs](https://docs.rs/gpui-storybook-core/badge.svg)](https://docs.rs/gpui-storybook-core/)
[![Crates.io](https://img.shields.io/crates/v/gpui-storybook-core.svg)](https://crates.io/crates/gpui-storybook-core)

Public integration crate for the storybook runtime used by `gpui-storybook`.

This crate is for applications that need runtime-level control over the window shell, gallery, dock workspace, or title-bar customization. Most applications should start with [`gpui-storybook`](../gpui-storybook/README.md) instead.

## What it provides

- `Gallery` for the searchable sidebar and active-story layout
- `StorybookAutomation` for live story listing, story opening, and capture coordination
- `StoryContainer`, `Story`, the styled `section` helper, and `StorySectionBase` for custom section components with stable sub-story capture keys
- typed registration metadata on `StoryContainer` for automation keys, registered names, and source locations
- `create_new_window` and `create_new_window_with_ui` for the standard storybook shell
- `StorybookWindowUi` for custom app-menu and title-bar additions
- `StoryWorkspace` and `create_dock_window` behind the `dock` feature
- built-in theme persistence, locale wiring, and embedded assets

Enable the `capture` feature when automation needs to render the active story
to a PNG through GPUI's platform test-support image path. Most applications
should use the facade crate's `mcp` feature instead of enabling this directly.

## Typical direct use

```rs
use gpui::App;
use gpui_storybook_core::{
    automation::{StorybookAutomation, set_default_storybook_automation},
    gallery::Gallery,
    story::StoryContainer,
    story::{create_new_window_with_ui},
    storybook_window_ui::{StorybookWindow, StorybookWindowUi},
};

fn open_story_window(stories: Vec<gpui::Entity<StoryContainer>>, cx: &mut App) {
    let ui = StorybookWindowUi::new().with_app_menu_items(|_| Vec::new());
    set_default_storybook_automation(cx, StorybookAutomation::new());

    create_new_window_with_ui("Stories", move |window, cx| {
        StorybookWindow::new(Gallery::view(stories.clone(), None, window, cx))
        .with_ui(ui)
    }, cx);
}
```

If you need compile-time story registration and `storybook.toml` filtering, use the facade crate instead of building directly on `gpui-storybook-core`.
