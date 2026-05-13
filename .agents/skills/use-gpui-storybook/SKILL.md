---
name: use-gpui-storybook
description: "Use when Codex needs to help application developers adopt GPUI Storybook in an app: setting up a storybook binary, adding stories with #[story] or #[derive(ComponentStory)], configuring storybook.toml group/allow/disable_story behavior, choosing gallery versus dock mode, wiring locale initialization, or troubleshooting missing stories."
---

# Use GPUI Storybook

## Start

Treat this as a user-facing application workflow. Center examples and edits on the public facade crate, `gpui-storybook`.

Before editing an app, inspect:

- `Cargo.toml` for existing `gpui`, `gpui-component`, `gpui-storybook`, and feature setup.
- Any existing storybook binary or example app.
- Existing `storybook.toml` files.
- Existing story section enums or naming conventions.

## Runtime Setup

Use this shape for a storybook binary:

```rust
use es_fluent::EsFluent;
use es_fluent_lang::es_fluent_language;
use gpui_storybook::{Assets, Gallery};
use strum::EnumIter;

es_fluent_manager_embedded::define_i18n_module!();

#[es_fluent_language]
#[derive(Clone, Copy, Debug, EnumIter, EsFluent, PartialEq)]
pub enum Languages {}

fn main() {
    let app = gpui_platform::application().with_assets(Assets);

    app.run(|cx| {
        gpui_storybook::init(cx, Languages::default());
        gpui_storybook::change_locale(cx, Languages::default()).unwrap();

        gpui_storybook::create_new_window("My App - Stories", |window, cx| {
            let stories = gpui_storybook::generate_stories(window, cx);
            Gallery::view(stories, None, window, cx)
        }, cx);
    });
}
```

For the dock workspace, enable the `dock` feature and use `create_dock_window` plus `StoryWorkspace::view(...)` instead of `create_new_window` plus `Gallery::view(...)`.

## Choose Registration

Use explicit `#[story]` when the story needs its own GPUI view state, focus handle, actions, lifecycle, or wrapper UI:

```rust
#[gpui_storybook::story(crate::StorySection::Buttons)]
pub struct ButtonStory {
    focus_handle: gpui::FocusHandle,
}

impl ButtonStory {
    pub fn view(_: &mut gpui::Window, cx: &mut gpui::App) -> gpui::Entity<Self> {
        cx.new(|cx| Self {
            focus_handle: cx.focus_handle(),
        })
    }
}

impl gpui_storybook::Story for ButtonStory {
    fn title(_: &gpui::App) -> String {
        "Button".into()
    }

    fn new_view(
        window: &mut gpui::Window,
        cx: &mut gpui::App,
    ) -> gpui::Entity<impl gpui::Render + gpui::Focusable> {
        Self::view(window, cx)
    }
}
```

Use `#[derive(ComponentStory)]` when the component can render from example data and storybook should generate the wrapper view:

```rust
#[derive(gpui::IntoElement, gpui_storybook::ComponentStory)]
#[storybook(
    title = "Welcome Card",
    description = "Preview of the welcome card",
    section = crate::StorySection::Intro,
    example = WelcomeCard::example(),
)]
pub struct WelcomeCard {
    title: gpui::SharedString,
}
```

`ComponentStory` expects a non-generic struct. Without `example = ...`, the generated wrapper renders `<Component as Default>::default()`.
`title` and `description` expressions are emitted inside methods that receive `cx: &gpui::App`, so they can call `gpui_storybook::localize_message(cx, ...)`.

Use `#[gpui_storybook::story_init]` for one-time setup that must run after `gpui_storybook::init(...)` and before stories are shown:

```rust
#[gpui_storybook::story_init]
fn register_icons(cx: &mut gpui::App) {
    // global setup
}
```

## Sections And Filtering

Prefer enum sections when stable ordering matters. String sections are fine for simple grouping.

```rust
#[derive(Clone, Copy)]
#[repr(usize)]
enum StorySection {
    Intro = 1,
    Components = 2,
}
```

Place `storybook.toml` next to the crate whose stories need a runtime group or filter:

```toml
group = "UI Kit"
allow = ["UI Kit", "Shared"]
disable_story = ["LegacyCardStory"]
```

Apply these rules:

- `group` is required when `storybook.toml` exists.
- Omitting `allow` includes only the crate's own `group`.
- `allow = ["*"]` includes every group.
- `allow = []` includes none.
- `disable_story` matches the registered story type name exactly.
- For `ComponentStory`, `disable_story` uses the component type name, not the generated wrapper type.

If stories are unexpectedly missing, inspect runtime logs for discovered story count, selected runtime config, group filtering, and `disable_story` matches. Also confirm the crate containing story registrations is linked by the binary.
