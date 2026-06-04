---
name: use-gpui-storybook
description: "Use when Codex needs to help application developers adopt GPUI Storybook in an app: setting up a storybook binary, adding stories with #[story] or #[derive(ComponentStory)], configuring storybook.toml group/allow/disable_story behavior, choosing gallery versus dock mode, wiring locale initialization, or troubleshooting missing stories."
---

# Use GPUI Storybook

## Scope Boundary

Treat this skill as a hosted public-usage guide for GPUI Storybook consumers.
Use it only for user-facing application workflows: setting up a storybook
binary, adding stories, configuring `storybook.toml`, choosing gallery or dock
mode, wiring locale initialization, and troubleshooting missing stories.

Do not use this skill as a contributor guide for `gpui-storybook` repository
internals. For build, test, format, lint, maintenance, release, or architecture
work, read the repository source, `AGENTS.md`, and the relevant crate
documentation directly.

## Core Workflow

Start from the user-facing facade. Most application code uses
`gpui-storybook` plus one story registration style:

1. Inspect `Cargo.toml` for existing `gpui`, `gpui-component`,
   `gpui-storybook`, and feature setup.
2. Inspect any existing storybook binary, example app, `storybook.toml` files,
   section enums, and naming conventions.
3. Initialize the storybook runtime before creating the story window:
   `gpui_storybook::init(cx, Languages::default())`.
4. Choose gallery mode for a focused story browser, or enable the `dock` feature
   and use the dock workspace when stories need docked panels.
5. Use explicit `#[story]` when the story needs its own GPUI view state, focus
   handle, actions, lifecycle, or wrapper UI.
6. Use `#[derive(ComponentStory)]` when the component can render from example
   data and storybook should generate the wrapper view.
7. Put `storybook.toml` next to the crate whose stories need a runtime group or
   filter.

## Reference Selection

This skill has no extra reference files. Prefer the current public READMEs,
example applications, and source snippets over memory when details matter:

- `README.md`: top-level setup and usage guidance.
- `crates/gpui-storybook/README.md`: facade API guidance.
- `examples/story/README.md`: explicit `#[story]` workflow.
- `examples/component/README.md`: `#[derive(ComponentStory)]` workflow.
- `crates/gpui-storybook-toml/README.md`: `storybook.toml` semantics.

## Implementation Rules

Use this shape for a storybook binary:

```rust
// src/i18n.rs
use es_fluent::EsFluent;
use es_fluent_lang::es_fluent_language;
use strum::EnumIter;

es_fluent_manager_embedded::define_i18n_module!();

#[es_fluent_language]
#[derive(Clone, Copy, Debug, EnumIter, EsFluent, PartialEq)]
pub enum Languages {}

// src/lib.rs
pub mod i18n;

// src/main.rs
use my_app::i18n::Languages;
use gpui_storybook::{Assets, Gallery};

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

For the dock workspace, enable the `dock` feature and use
`create_dock_window` plus `StoryWorkspace::view(...)` instead of
`create_new_window` plus `Gallery::view(...)`.

Use explicit `#[story]` when the story owns state:

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

Use `#[derive(ComponentStory)]` when storybook can generate the wrapper view:

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

`ComponentStory` expects a non-generic struct. Without `example = ...`, the
generated wrapper renders `<Component as Default>::default()`.

`title` and `description` expressions are emitted inside methods that receive
`cx: &gpui::App`, so they can call `gpui_storybook::localize_message(cx, ...)`.

Use `#[gpui_storybook::story_init]` for one-time setup that must run after
`gpui_storybook::init(...)` and before stories are shown:

```rust
#[gpui_storybook::story_init]
fn register_icons(cx: &mut gpui::App) {
    // global setup
}
```

Prefer enum sections when stable ordering matters. String sections are fine for
simple grouping:

```rust
#[derive(Clone, Copy)]
#[repr(usize)]
enum StorySection {
    Intro = 1,
    Components = 2,
}
```

Apply these `storybook.toml` rules:

```toml
group = "UI Kit"
allow = ["UI Kit", "Shared"]
disable_story = ["ExperimentalCardStory"]
```

- `group` is required when `storybook.toml` exists.
- Omitting `allow` includes only the crate's own `group`.
- `allow = ["*"]` includes every group.
- `allow = []` includes none.
- `disable_story` matches the registered story type name exactly.
- For `ComponentStory`, `disable_story` uses the component type name, not the generated wrapper type.
- `generate_stories` uses the `storybook.toml` from the registered story crate whose package name matches the running binary.

If stories are unexpectedly missing, inspect runtime logs for discovered story
count, selected runtime config, group filtering, and `disable_story` matches.
Also confirm the crate containing story registrations is linked by the binary.
