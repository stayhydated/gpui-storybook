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

pub fn apply_locale(
    language: Languages,
    cx: &mut gpui::App,
) -> Result<(), gpui_es_fluent::EmbeddedInitError> {
    let _linked_module = &MY_APP_I18N_MODULE;
    gpui_es_fluent::replace_with_language(cx, language)
}

// src/lib.rs
pub mod i18n;

// src/main.rs
use my_app::i18n::{self, Languages};
use gpui_storybook::{Assets, ConsumerId, Gallery, StorybookOptions};

fn main() {
    let app = gpui_platform::application().with_assets(Assets);

    app.run(|cx| {
        let consumer_id = match ConsumerId::new("my-app-storybook") {
            Ok(consumer_id) => consumer_id,
            Err(_) => {
                cx.quit();
                return;
            },
        };
        let options = StorybookOptions::new(
            consumer_id,
            Languages::default(),
            i18n::apply_locale,
        );
        let readiness = match gpui_storybook::init(cx, options) {
            Ok(readiness) => readiness,
            Err(_) => {
                cx.quit();
                return;
            },
        };

        cx.spawn(async move |cx| {
            let ready = readiness.await;
            if !ready.diagnostics.is_empty() {
                tracing::warn!(
                    persistence_status = ?ready.persistence_status,
                    diagnostics = ?ready.diagnostics,
                    "Storybook initialized with preference diagnostics"
                );
            }
            cx.update(|cx| {
                gpui_storybook::create_new_window("My App - Stories", |window, cx| {
                    let stories = gpui_storybook::generate_stories(window, cx);
                    Gallery::view(stories, None, window, cx)
                }, cx);
            });
        }).detach();
    });
}
```

The locale setup has four parts: add `es-fluent-build` as a build dependency and
track locale assets from `build.rs`, define the embedded i18n module in
library-reachable `src/i18n.rs`, derive the app language enum with `EsFluent`,
then pass its fallback and GPUI locale adapter through `StorybookOptions`. Keep
an explicit same-module reference to the private static emitted by
`define_i18n_module!`; its name is the Cargo package name in upper snake case
plus `_I18N_MODULE`. This keeps the consumer module linked before
`replace_with_language` installs its manager. Storybook owns a separate shell
localizer and falls back to its embedded English resources when the consumer
selects a locale the shell does not embed.
`init` returns a readiness task; await it before constructing the first window
so saved appearance and language intent is applied before the first frame.

## Local preferences

`StorybookOptions::new` requires a stable `ConsumerId`. Give every Storybook
binary its own ID: persistent documents and their default paths are
consumer-scoped, so IDs isolate unrelated apps while remaining stable across
launches.

Storybook keeps user intent separate from effective presentation:

- `PreferenceState::saved` retains `System` or explicit appearance/language
  intent, independent light and dark theme slots, and scrollbar behavior.
- `PreferenceState::resolved` reports the effective scheme, theme, language,
  source, and fallback diagnostics after system detection, registry
  availability, and deterministic overrides are applied.
- `System` appearance follows live window appearance changes. `System`
  language negotiates ordered device locales at startup and re-detects them
  when a Storybook window becomes active. Explicit choices ignore later system
  changes.
- Changing the inactive light or dark theme slot saves that slot without
  changing the current scheme; it becomes effective when the scheme changes.

`StorybookOptions::with_overrides(PreferenceOverrides { ... })` and the active
runtime config's `[overrides]` table change only the resolved presentation for
that launch. Overrides never rewrite saved user intent. Programmatic values win
field by field over `storybook.toml`; deterministic MCP capture or stdio values
win over both.

Persistent storage is the default and writes
`.gpui-storybook/{consumer-id}.json` plus one shared
`.gpui-storybook/preferences.schema.json` at the Cargo workspace root, or at the
package root for a standalone crate. Storybook creates
`.gpui-storybook/.gitignore` with `*` so the local state stays out of Git.
`PersistenceMode::Temporary` uses a unique JSON file and generated schema
removed with the repository, while `Disabled` keeps only in-memory session
state. Use `StorybookOptions::with_json_path(...)` for an explicit persistent
JSON location. The generated schema exposes named consumer ID, theme ID, and
BCP 47 language-tag definitions with descriptions and validation constraints.

Configuration errors detected before preference loading make `init` return
`StorybookInitError`. Repository open or load failures instead complete
readiness with `PersistenceStatus::Error` and diagnostics so the app can still
open with fallbacks. Locale-adapter failures add diagnostics without changing
the storage-only persistence status and are retried when a window becomes
active. Optimistic menu changes remain active for the session after a save
failure; open windows show a localized **Retry Save** notification action. The
Preferences menu exposes the current loading/saving/error state and a generic
**Retry Preferences** action. Retrying a startup load failure reloads existing
stored intent; only an actual dirty/save failure retries an upsert. Use
`gpui_storybook::try_preference_state` for a read-only snapshot.

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

The same `StorybookOptions` initialization installs the MCP automation
controller when the feature is enabled, and `Gallery::view(...)` or
`StoryWorkspace::view(...)` attach it automatically.
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

Capture startup uses a deterministic, non-persistent profile: light appearance,
the registered `Default Light` theme, the configured fallback language, and
disabled preference storage. Stdio-only MCP startup uses the same presentation
with temporary storage, so automation does not overwrite interactive choices.

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
use gpui::{
    App, AppContext as _, Context, Entity, FocusHandle, Focusable, IntoElement,
    ParentElement as _, Render, Window, div,
};

#[gpui_storybook::story("Components")]
pub struct ButtonStory {
    focus_handle: FocusHandle,
}

impl ButtonStory {
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| Self {
            focus_handle: cx.focus_handle(),
        })
    }
}

impl Focusable for ButtonStory {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for ButtonStory {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().child("Button preview")
    }
}

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

Use `#[gpui_storybook::story_init]` for initialization that should run once
during `gpui_storybook::init(...)`, after the core runtime is installed and
before preference readiness begins.

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

## Configure stories and preferences with `storybook.toml`

Put a `storybook.toml` next to the crate whose stories and launch presentation
you want to configure:

```toml
group = "UI Kit"
allow = ["UI Kit", "Shared"]
disable_story = ["ExperimentalCardStory"]

[overrides]
color_scheme = "dark"
theme = "Default Dark"
language = "en"
```

- `group` is required when `storybook.toml` exists.
- Omitting `allow` means "only include this crate's own `group`".
- `allow = ["*"]` includes every group.
- `allow = []` includes none.
- `disable_story` matches the registered story type name.
- For `ComponentStory`, the registered story name is the component type name.
- `disable_story` does not use the full automation `StoryKey`.
- `[overrides]` and each of its fields are optional.
- `color_scheme` accepts `"light"` or `"dark"` and bypasses live system
  appearance changes for the launch.
- `theme` names a registered theme for the effective color scheme.
- `language` is a BCP 47 tag from the application's typed embedded language
  set and bypasses system locale negotiation for the launch.

At runtime, `init` applies preference overrides and `generate_stories` applies
story filters from the `storybook.toml` in the registered story crate whose
package name matches the running binary. Invalid runtime config or a language
outside the typed embedded set makes `init` return `StorybookInitError`; an
unavailable named theme uses the registered fallback and reports a resolution
diagnostic.
