# GPUI Storybook

[![Build Status](https://github.com/stayhydated/gpui-storybook/actions/workflows/ci.yml/badge.svg)](https://github.com/stayhydated/gpui-storybook/actions/workflows/ci.yml)
[![Docs](https://docs.rs/gpui-storybook/badge.svg)](https://docs.rs/gpui-storybook/)
[![Crates.io](https://img.shields.io/crates/v/gpui-storybook.svg)](https://crates.io/crates/gpui-storybook)

`gpui-storybook` is the user-facing facade crate for the workspace.

Use it when you want:

- story discovery and runtime filtering through `generate_stories`
- the standard gallery window or optional dock workspace
- `#[story]`, `#[derive(ComponentStory)]`, and `#[story_init]` through one dependency
- built-in theme, locale, and title-bar wiring

Most applications should depend on this crate directly.

## Run the example apps

```bash
cargo run -p gpui-storybook-example-story
cargo run -p gpui-storybook-example-component
```

With dock layout:

```bash
cargo run -p gpui-storybook-example-story --features dock
cargo run -p gpui-storybook-example-component --features dock
```

## Minimal app shell

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

Enable the `dock` feature when you want the panel-based workspace:

```toml
[dependencies]
gpui-storybook = { version = "*", features = ["dock"] }
```

## Registration styles

### `#[story]` for explicit story types

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

Use this flow when the story needs its own state, focus handling, or wrapper UI.

### `#[derive(ComponentStory)]` for component-attached registration

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

Use this flow when the component should only describe its own example data and render output.

### `#[story_init]` for one-time setup

```rs
#[gpui_storybook::story_init]
fn register_icons(cx: &mut gpui::App) {
    // global setup
}
```

## Sections and ordering

Both registration styles accept string sections or enum variants. Enum discriminants become the sort order used by discovery:

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

## `storybook.toml`

Add a `storybook.toml` to a crate root when you want that crate to participate in runtime grouping or filtering:

```toml
group = "UI Kit"
allow = ["UI Kit", "Shared"]
disable_story = ["LegacyCardStory"]
```

- `group` is required when the file exists.
- Omitting `allow` means "only include this crate's own `group`".
- `allow = ["*"]` includes every group.
- `allow = []` includes none.
- `disable_story` matches the registered story type name.
- For `ComponentStory`, the registered story name is the component type name.

At runtime, `generate_stories` prefers the `storybook.toml` associated with the current binary crate and falls back to searching upward from the working directory.

## Acknowledgements

This project is heavily inspired by the story section of [gpui-component](https://github.com/longbridge/gpui-component/tree/main/crates/story).

See related discussion on ownership transfer [here](https://github.com/longbridge/gpui-component/discussions/1473).
