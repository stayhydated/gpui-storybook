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
- GPUI-owned saved and resolved preference state for appearance, independent
  light/dark theme slots, locale, scrollbar behavior, and persistence status
- embedded localization resources and assets

## Preference runtime

The public facade initializes this runtime from typed `StorybookOptions` and
returns a readiness task. Await readiness before opening the first window so a
consumer's saved appearance and locale intent is applied before the first
frame. Each standard gallery or dock window then forwards appearance changes
and activation events to the runtime.

Saved intent and effective presentation remain separate. `System` appearance
tracks live window appearance, and `System` language re-detects ordered device
locales when a window activates. Light and dark theme selections occupy
independent slots. The active slot follows the resolved scheme; changing the
inactive slot does not force a scheme change.

`PersistenceStatus` describes storage only: loading, ready, saving, or error.
Storage and locale-application diagnostics remain available on
`PreferenceState` and `StorybookReady`. A locale-adapter failure does not turn
the storage status into an error; the typed Storybook/component locale remains
installed and activation retries the consumer adapter. A failed save keeps the
optimistic session value active, gives open windows a localized **Retry Save**
notification action, and exposes generic **Retry Preferences** in the
Preferences menu. Retrying a startup load failure reloads existing intent; only
pending or failed user changes are upserted.

Storybook shell messages use a core-owned embedded localizer. The consumer
locale adapter owns the application's separate GPUI Fluent manager, including
consumer language labels and story messages. If the consumer selects a locale
the shell does not embed, shell messages use embedded English while consumer
messages remain in the selected locale.

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
