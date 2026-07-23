---
name: use-gpui-storybook
description: "Use when helping application developers adopt GPUI Storybook in an app: setting up a storybook binary, adding stories with #[story] or #[derive(ComponentStory)], configuring storybook.toml group/allow/disable_story behavior, choosing gallery versus dock mode, wiring locale initialization, enabling MCP automation/capture, or troubleshooting missing stories."
---

# Use GPUI Storybook

## Scope Boundary

Use this public skill for application-level GPUI Storybook integration:
setting up a storybook binary, adding stories, configuring `storybook.toml`,
choosing gallery or dock mode, wiring locale initialization, enabling MCP
automation/capture, and troubleshooting missing stories.

Do not use this skill for maintaining the GPUI Storybook implementation itself,
release workflows, repository architecture, or crate internals.

## Core Workflow

Start from the user-facing facade. Most application code uses
`gpui-storybook` plus one story registration style:

1. Inspect `Cargo.toml` for existing `gpui`, `gpui-component`,
   `gpui-storybook`, and feature setup.
2. Inspect any existing storybook binary, example app, `storybook.toml` files,
   section enums, and naming conventions.
3. Add `es-fluent-build` as a build dependency and `gpui-es-fluent` as a
   runtime dependency, define the embedded i18n module in library-reachable
   code, and call `es_fluent_build::track_i18n_assets()` from `build.rs` so
   locale asset changes trigger rebuilds.
4. Choose a stable, binary-specific `ConsumerId`, build typed
   `StorybookOptions`, call `gpui_storybook::init(...)`, and await its readiness
   task before creating the first story window.
5. Choose gallery mode for a focused story browser, or enable the `dock` feature
   and use the dock workspace when stories need docked panels.
6. Use explicit `#[story]` when the story needs its own GPUI view state, focus
   handle, actions, lifecycle, or wrapper UI.
7. Use `#[derive(ComponentStory)]` when the component can render from example
   data and storybook should generate the wrapper view.
8. Put `storybook.toml` next to the crate whose stories need a runtime group,
   filter, or launch-only presentation override.
9. Enable the `mcp` feature only when callers need external automation, MCP
   tools, or PNG capture.

## Reference Selection

This skill has no extra reference files. When exact details matter, prefer the
project's public README, crate documentation, examples, and source snippets over
memory.

## Implementation Rules

Use this shape for a storybook binary:

```rust
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

const CONSUMER_ID: &str = "my-app-storybook";

fn main() {
    let app = gpui_platform::application().with_assets(Assets);

    app.run(|cx| {
        let consumer_id = match ConsumerId::new(CONSUMER_ID) {
            Ok(consumer_id) => consumer_id,
            Err(error) => {
                tracing::error!(error = %error, "invalid Storybook consumer id");
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
            Err(error) => {
                tracing::error!(error = %error, "failed to initialize Storybook");
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

`define_i18n_module!` emits a private static named from the Cargo package in
upper snake case plus `_I18N_MODULE`. Reference it inside the same module before
`replace_with_language` so the consumer resources remain linked. Storybook owns
a separate shell localizer; when the consumer selects a locale the shell does
not embed, shell text uses embedded English while consumer messages remain in
the selected locale.

For the dock workspace, enable the `dock` feature and use
`create_dock_window` plus `StoryWorkspace::view(...)` instead of
`create_new_window` plus `Gallery::view(...)`.

### Preference contract

- Give each Storybook binary a distinct stable `ConsumerId`. The ID scopes the
  default persistent JSON path and the document's consumer identity.
- `StorybookOptions` defaults to `PersistenceMode::Persistent` at
  `.gpui-storybook/{consumer-id}.json` in the Cargo workspace or standalone
  package root. The directory generates a `.gitignore` containing `*`.
  Persistent storage shares one `preferences.schema.json`; temporary storage
  writes the same stable schema name beside its typed JSON document.
  `Temporary` owns unique files for the repository lifetime; `Disabled` keeps
  state in memory. Call `with_json_path(...)` only with persistent mode.
- Await the `init` readiness task before any gallery or dock window. This is
  what prevents a first frame using defaults before saved intent is applied.
- Treat `PreferenceState::saved` as user intent and `resolved` as effective
  presentation. Saved `System` choices remain `System`; resolved values explain
  the detected or fallback scheme/language/theme and source.
- `StorybookOptions::with_overrides(PreferenceOverrides { ... })` and the active
  runtime config's `[overrides]` table change only resolved presentation for
  the current launch; they never rewrite saved user intent. Programmatic values
  win field by field over TOML. Deterministic MCP capture or stdio values win
  over both.
- Appearance and language each offer `System`. Appearance follows live window
  appearance events. Language negotiates ordered device locales initially and
  re-detects on window activation. Explicit choices ignore later detection.
- Light and dark themes are independent slots. Selecting the inactive slot
  saves it without changing the current scheme.
- `PersistenceStatus` is storage-only. Static option errors are returned by
  `init`; storage and locale-application failures are reported through
  readiness/state diagnostics. A locale-adapter failure is retried on window
  activation. A failed save keeps the optimistic session value, shows a
  localized **Retry Save** notification action, and exposes generic **Retry
  Preferences** in the Preferences menu. Retrying a startup load failure
  reloads existing stored intent; only pending or failed user changes are
  upserted.
- Use `gpui_storybook::try_preference_state(cx)` when application code needs a
  read-only snapshot of saved/resolved state, status, or diagnostics.

For MCP automation or capture, enable the `mcp` feature. The gallery and dock
constructors stay the same after the options/readiness setup:
`gpui_storybook::init(...)` installs the automation controller, and
`Gallery::view(...)` or `StoryWorkspace::view(...)` attach it automatically.
Set `GPUI_STORYBOOK_MCP_STDIO=1` to serve MCP over stdio. Set
`WGPU_CAPTURE_ROUTE` to a story key and `WGPU_CAPTURE_PATH` to capture one story
during startup. A capture launch disables persistence and forces light,
`Default Light`, and fallback-language resolution. Stdio-only startup uses the
same deterministic presentation with temporary storage. Captures are cropped
to the story view, excluding the sidebar and storybook header or dock chrome.
Storybook MCP tools publish closed typed input/output schemas and structured
argument errors; use the advertised `key` and capture option properties instead
of sending additional arguments.
`WGPU_CAPTURE_WIDTH` and `WGPU_CAPTURE_HEIGHT` must be set together and greater
than zero; they request a live resize. Use the capture result's pixel
dimensions as the source of truth.
`WGPU_CAPTURE_FRAME`, when set, must be a one-based value greater than zero.

Use explicit `#[story]` when the story owns state:

```rust
#[gpui_storybook::story(crate::StorySection::Buttons)]
pub struct ButtonStory {
    focus_handle: gpui::FocusHandle,
}

impl ButtonStory {
    pub fn view(_: &mut gpui::Window, cx: &mut gpui::App) -> gpui::Entity<Self> {
        gpui::AppContext::new(cx, |cx| Self {
            focus_handle: cx.focus_handle(),
        })
    }
}

impl gpui::Focusable for ButtonStory {
    fn focus_handle(&self, _: &gpui::App) -> gpui::FocusHandle {
        self.focus_handle.clone()
    }
}

impl gpui::Render for ButtonStory {
    fn render(
        &mut self,
        _: &mut gpui::Window,
        _: &mut gpui::Context<Self>,
    ) -> impl gpui::IntoElement {
        gpui::div()
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

Story registration also emits a stable automation key:

- Explicit `#[story]`: `{crate-package-name}-{story-struct-name}`.
- `ComponentStory`: `{crate-package-name}-{component-type-name}`.

When code needs the registered identity from generated `StoryContainer` values,
prefer `registration_metadata()`, `story_key()`, or `story_name()` over manual
string field coordination.

For example, `gpui-storybook-example-story-ButtonStory` and
`gpui-storybook-example-component-WelcomeCard` are valid capture routes.
Sub-story routes use `story-key/substory-key`. Plain string sections use
title-derived slugs through `gpui_storybook::capture_substory(...)`; sections
passed a `#[derive(gpui_storybook::Substory)]` enum variant use the variant's
stable kebab-case key. For example:
`gpui-storybook-example-story-ButtonStory/with-progress`.
Use `gpui_storybook::section(...)` for the standard styled section. For custom
section components, store `gpui_storybook::StorySectionBase::new(...)` and call
`base.capture(...)` from `RenderOnce` after building the component's own layout
and chrome.

```rust
#[derive(gpui_storybook::Substory)]
enum ButtonSubstory {
    NormalButton,
    #[substory(title = "Button with Icon")]
    ButtonWithIcon,
    #[substory(title = "With Progress")]
    WithProgress,
}
```

The default capture key is the variant name in kebab case. Use `title` to
change display text without changing the route, and use `key` to set an
explicit lowercase ASCII route segment containing letters, numbers, or `-`.

Use `#[gpui_storybook::story_init]` for one-time setup that runs during
`gpui_storybook::init(...)`, after the core runtime is installed and before
preference readiness begins:

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

[overrides]
color_scheme = "dark"
theme = "Default Dark"
language = "en"
```

- `group` is required when `storybook.toml` exists.
- Omitting `allow` includes only the crate's own `group`.
- `allow = ["*"]` includes every group.
- `allow = []` includes none.
- `disable_story` matches the registered story type name exactly.
- For `ComponentStory`, `disable_story` uses the component type name, not the generated wrapper type.
- `disable_story` does not use the full automation story key.
- `[overrides]` and each field are optional.
- `color_scheme` accepts `"light"` or `"dark"` and bypasses live system
  appearance changes for the launch.
- `theme` names a registered theme for the effective color scheme.
- `language` is a BCP 47 tag from the consumer's typed embedded language set
  and bypasses system locale negotiation for the launch.
- `init` applies preference overrides and `generate_stories` applies filters
  from the `storybook.toml` in the registered story crate whose package name
  matches the running binary.
- Invalid runtime config or a language outside the typed embedded set makes
  `init` return `StorybookInitError`. An unavailable named theme uses the
  registered fallback and emits a resolution diagnostic.

If stories are unexpectedly missing, inspect runtime logs for discovered story
count, selected runtime config, group filtering, and `disable_story` matches.
Also confirm the crate containing story registrations is linked by the binary.
If the first frame uses defaults, confirm window construction happens only
after readiness. For preference failures, inspect `StorybookReady::diagnostics`
or `PreferenceState::diagnostics`; storage failures also appear as
`PersistenceStatus::Error` and expose Retry in the Preferences menu.
