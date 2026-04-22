# GPUI Storybook

[![Build Status](https://github.com/stayhydated/gpui-storybook/actions/workflows/ci.yml/badge.svg)](https://github.com/stayhydated/gpui-storybook/actions/workflows/ci.yml)
[![Docs](https://docs.rs/gpui-storybook/badge.svg)](https://docs.rs/gpui-storybook/)
[![Crates.io](https://img.shields.io/crates/v/gpui-storybook.svg)](https://crates.io/crates/gpui-storybook)

`gpui-storybook` is a storybook-style shell for building and inspecting GPUI components.

It is built around three goals:

1. Fast iteration with a searchable preview shell.
1. Stable organization through sections and crate-level groups.
1. Good developer experience with built-in theming, locale switching, and optional dock layouts.

## Compatibility

| `gpui-storybook` | `gpui-component` | `gpui` |
| :---------------- | :--------------- | :------ |
| **git** | | |
| `branch = "master"` | `branch = "main"` | `rev = "f7d46cf7d02c88d3d71ec495a31d7f19bd5eb96b"` |

## Examples

Explicit `#[story]` workflow:

```bash
cargo run -p gpui-storybook-example-story
```

Component-attached `#[derive(ComponentStory)]` workflow:

```bash
cargo run -p gpui-storybook-example-component
```

Dock workspace:

```bash
cargo run -p gpui-storybook-example-story --features dock
cargo run -p gpui-storybook-example-component --features dock
```

## Quick start

The examples contain the full `Cargo.toml` setup. The minimal runtime shape looks like this:

```rs
use es_fluent::EsFluent;
use es_fluent_lang::es_fluent_language;
use gpui_storybook::{Assets, Gallery};
use strum::EnumIter;

#[es_fluent_language]
#[derive(Clone, Copy, Debug, EnumIter, EsFluent, PartialEq)]
pub enum Languages {}

fn main() {
    let app = gpui_platform::application().with_assets(Assets);

    app.run(|cx| {
        gpui_storybook::init(Languages::default(), cx);
        gpui_storybook::change_locale(Languages::default()).unwrap();

        gpui_storybook::create_new_window("My App - Stories", |window, cx| {
            let stories = gpui_storybook::generate_stories(window, cx);
            Gallery::view(stories, None, window, cx)
        }, cx);
    });
}
```

Turn on the `dock` feature when you want a panel-based workspace instead of the gallery layout:

```toml
[dependencies]
gpui-storybook = { version = "*", features = ["dock"] }
```

## Choose a registration style

### Explicit stories with `#[story]`

Use this when the story needs its own state, focus management, or view wrapper.

```rs
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

See [`examples/story`](examples/story/README.md) for the full explicit workflow.

### Component-attached stories with `#[derive(ComponentStory)]`

Use this when the component should stay focused on its own data and rendering, and storybook should generate the wrapper view.

```rs
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

See [`examples/component`](examples/component/README.md) for the full derive-based workflow.

### One-time app setup with `#[story_init]`

Use `#[gpui_storybook::story_init]` for initialization that should run once after `gpui_storybook::init(...)` and before stories are shown.

```rs
#[gpui_storybook::story_init]
fn register_icons(cx: &mut gpui::App) {
    // global setup
}
```

## Organize stories with sections

Both registration styles accept either string sections or enum variants. Enum discriminants become stable section ordering.

```rs
#[derive(Clone, Copy)]
#[repr(usize)]
enum StorySection {
    Basics = 1,
    Components = 2,
    Patterns = 3,
}

#[gpui_storybook::story(StorySection::Components)]
pub struct CardStory;
```

`#[storybook(section = StorySection::Patterns)]` follows the same rules.

## Filter stories with `storybook.toml`

Put a `storybook.toml` next to the crate whose stories you want to group or filter:

```toml
group = "UI Kit"
allow = ["UI Kit", "Shared"]
disable_story = ["LegacyCardStory"]
```

- `group` is required when `storybook.toml` exists.
- Omitting `allow` means "only include this crate's own `group`".
- `allow = ["*"]` includes every group.
- `allow = []` includes none.
- `disable_story` matches the registered story type name.
- For `ComponentStory`, the registered story name is the component type name.

At runtime, `generate_stories` prefers the `storybook.toml` that belongs to the current binary crate and falls back to searching upward from the working directory.
