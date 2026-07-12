# GPUI Storybook

[![Build Status](https://github.com/stayhydated/gpui-storybook/actions/workflows/ci.yml/badge.svg)](https://github.com/stayhydated/gpui-storybook/actions/workflows/ci.yml)
[![Codecov](https://codecov.io/github/stayhydated/gpui-storybook/graph/badge.svg)](https://codecov.io/github/stayhydated/gpui-storybook)
[![Docs](https://docs.rs/gpui-storybook/badge.svg)](https://docs.rs/gpui-storybook/)
[![Crates.io](https://img.shields.io/crates/v/gpui-storybook.svg)](https://crates.io/crates/gpui-storybook)

`gpui-storybook` is a storybook-style shell for building and inspecting GPUI components.

It is built around three goals:

1. Fast iteration with a searchable preview shell.
1. Stable organization through sections and crate-level groups.
1. Good developer experience with built-in theming, locale switching, and optional dock layouts.

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
// build.rs
fn main() {
    es_fluent_build::track_i18n_assets();
}

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

The locale setup has four parts: add `es-fluent-build` as a build dependency and
track locale assets from `build.rs`, define the embedded i18n module in
library-reachable `src/i18n.rs`, derive the app language enum with `EsFluent`,
then call `init` and `change_locale` with the selected language.

Turn on the `dock` feature when you want a panel-based workspace instead of the gallery layout:

```toml
[dependencies]
gpui-storybook = { version = "*", features = ["dock"] }
```

Turn on the `mcp` feature when another process needs to list stories, open a
story by key, or capture the active story:

```toml
[dependencies]
gpui-storybook = { version = "*", features = ["mcp"] }
```

No constructor changes are required. `gpui_storybook::init(...)` installs the
MCP automation controller when the feature is enabled, and `Gallery::view(...)`
or `StoryWorkspace::view(...)` attach it automatically.
The six storybook tools publish closed, typed input and output schemas plus MCP
read-only, idempotence, destructive, and open-world annotations. Invalid,
missing, and unknown arguments return machine-readable structured errors.

Run an MCP-enabled example over stdio:

```bash
GPUI_STORYBOOK_MCP_STDIO=1 cargo run -p gpui-storybook-example-story --features mcp
```

Capture a story without starting an MCP client:

```bash
WGPU_CAPTURE_ROUTE=gpui-storybook-example-story-ButtonStory \
WGPU_CAPTURE_PATH=target/storybook-captures/button.png \
cargo run -p gpui-storybook-example-story --features mcp
```

`WGPU_CAPTURE_WIDTH` and `WGPU_CAPTURE_HEIGHT` must be set together and greater
than zero; they request a live window resize before capture. Captures are
cropped to the story view, excluding the sidebar and storybook header or dock
chrome. MCP capture results report the actual rendered pixel size, which can
differ on scaled or compositor-managed displays.

Sub-story routes use `story-key/substory-key`. Plain string sections derive
their slug from the visible title through `gpui_storybook::capture_substory(...)`;
sections passed a `#[derive(gpui_storybook::Substory)]` enum variant use the
variant's stable key instead. The built-in styled `section(...)` helper and
custom components built on `StorySectionBase` both do this automatically, so
the Button story can also be captured with routes such as
`gpui-storybook-example-story-ButtonStory/normal-button`,
`gpui-storybook-example-story-ButtonStory/button-with-icon`, and
`gpui-storybook-example-story-ButtonStory/with-progress`.

```rs
#[derive(gpui_storybook::Substory)]
enum ButtonSubstory {
    NormalButton,
    #[substory(title = "Button with Icon")]
    ButtonWithIcon,
    #[substory(title = "With Progress")]
    WithProgress,
}

gpui_storybook::section(ButtonSubstory::WithProgress)
```

Define a custom section component when you want application-specific layout or
chrome. Store `gpui_storybook::StorySectionBase::new(...)` on the component and
call `base.capture(...)` from its `RenderOnce` implementation after building
the component's own element tree.

## Choose a registration style

### Explicit stories with `#[story]`

Use this when the story needs its own state, focus management, or view wrapper.

```rs
use gpui::{App, Focusable, Render, Window};

#[gpui_storybook::story("Components")]
pub struct ButtonStory;

impl gpui_storybook::Story for ButtonStory {
    fn title(_: &App) -> String {
        "Button".into()
    }

    fn new_view(window: &mut Window, cx: &mut App) -> gpui::Entity<impl Render + Focusable> {
        Self::view(window, cx)
    }
}
```

`Story::title`, `Story::description`, and `ComponentStory` `title`/`description` expressions receive the GPUI `App` context, so story metadata can call `gpui_storybook::localize_message(cx, ...)`.

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

The macros store registered story and section labels as typed
`StoryName`/`StorySectionName` values in the inventory registry. This mostly
matters for manual registry integrations; normal story declarations can keep
using string literals or enum variants.

Each registered story also receives a stable `StoryKey` for automation and
capture. Macro-generated keys use `{crate-package-name}-{registered-story-name}`;
for example, `gpui-storybook-example-story-ButtonStory` or
`gpui-storybook-example-component-WelcomeCard`. Explicit `#[story]` entries use
the story struct name. `ComponentStory` entries use the component type name.
Duplicate macro-generated keys fail to build, and `generate_stories` rejects
duplicate keys from manual registry entries.

Generated containers keep that identity as typed `RegisteredStoryMetadata`.
Use `StoryContainer::registration_metadata()` or the `story_key()` /
`story_name()` accessors when integrations need the registered story identity.

For capture-addressable sections inside a story, derive `Substory` on a
fieldless enum and pass variants to `gpui_storybook::section(...)` for the
standard styled section, or store `gpui_storybook::StorySectionBase` in a
custom section component. The default capture key is the variant name in kebab
case; `#[substory(title = "...")]` changes only the visible title, and
`#[substory(key = "...")]` sets an explicit route key independent of the
variant name.

## Filter stories with `storybook.toml`

Put a `storybook.toml` next to the crate whose stories you want to group or filter:

```toml
group = "UI Kit"
allow = ["UI Kit", "Shared"]
disable_story = ["ExperimentalCardStory"]
```

- `group` is required when `storybook.toml` exists.
- Omitting `allow` means "only include this crate's own `group`".
- `allow = ["*"]` includes every group.
- `allow = []` includes none.
- `disable_story` matches the registered story type name.
- For `ComponentStory`, the registered story name is the component type name.
- `disable_story` does not use the full automation `StoryKey`.

At runtime, `generate_stories` uses the `storybook.toml` from the registered story crate whose package name matches the running binary.
