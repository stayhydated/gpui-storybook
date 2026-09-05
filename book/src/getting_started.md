# Getting started

This guide adds a gallery-style Storybook binary to an existing GPUI workspace.
When setup is complete, running the package opens a window containing every
linked and enabled story plus the right-side workbench.

## Prerequisites

You need:

- an existing GPUI application or workspace;
- a native application entry point built with `gpui_platform`;
- embedded Fluent resources and a typed language enum;
- compatible revisions of GPUI, GPUI Component, and the Fluent integration
  crates.

GPUI Storybook version 0.5 targets Rust 1.98 and edition 2024.

## Add the Storybook package dependencies

Add `gpui-storybook` and the locale integration used by your application. In a
workspace that centralizes dependency versions, the relevant package entries
look like this:

```toml
[dependencies]
es-fluent.workspace = true
es-fluent-lang.workspace = true
es-fluent-manager-embedded.workspace = true
gpui.workspace = true
gpui-es-fluent.workspace = true
gpui-storybook.workspace = true
gpui_platform.workspace = true
rust-embed.workspace = true
strum.workspace = true
tracing.workspace = true

[build-dependencies]
es-fluent-build.workspace = true
```

Use explicit versions or Git revisions instead when the application does not
inherit workspace dependencies. Keep all GPUI-related crates on compatible
revisions.

Forward the opt-in Inspector feature from the Storybook binary when that
development surface is useful:

```toml
[features]
inspector = ["gpui-storybook/inspector"]
```

Launch that package with `--features inspector` to add the GPUI Inspector button
and Storybook story-root metadata to GPUI Component's Inspector. The Inspect
workbench tab remains available without this feature.

Forward GPUI profiler instrumentation when the Storybook binary should expose
the Perf workbench tab:

```toml
[features]
performance = ["gpui-storybook/performance"]
```

Launch with `--features performance` to inspect frame and input-latency
percentiles and control GPUI's debug frame overlay.

## Wire the locale adapter

Track locale assets from the Storybook package's `build.rs`:

```rust
fn main() {
    es_fluent_build::track_i18n_assets();
}
```

Define the embedded module and language enum in library-reachable code. For a
package named `my-app-storybook`, `src/i18n.rs` can contain:

```rust
use es_fluent::EsFluent;
use es_fluent_lang::es_fluent_language;
use strum::EnumIter;

es_fluent_manager_embedded::define_i18n_module!();

#[es_fluent_language]
#[derive(Clone, Copy, Debug, EnumIter, EsFluent, PartialEq)]
pub enum Languages {}

pub fn apply_locale(
    language: Languages,
    cx: &mut gpui_kit::App,
) -> Result<(), gpui_es_fluent::EmbeddedInitError> {
    let _linked_module = &MY_APP_STORYBOOK_I18N_MODULE;
    gpui_es_fluent::replace_with_language(cx, language)
}
```

The generated private static uses the Cargo package name converted to upper
snake case, followed by `_I18N_MODULE`. Reference it in the same module before
calling `replace_with_language` so the consumer's resources remain linked.

Expose the locale module and the modules containing story registrations from
the package library:

```rust
pub mod i18n;
pub mod stories;
```

## Initialize before opening a window

Create one stable `ConsumerId` for the Storybook binary. Call `init`, await
the returned readiness task, and then construct the Storybook window:

```rust
use gpui_storybook::{Assets, ConsumerId, StorybookOptions, StorybookWindow};
use my_app_storybook::i18n::{self, Languages};

fn main() {
    let app = gpui_kit::application().with_assets(Assets);

    app.run(|cx| {
        let options = StorybookOptions::new(
            ConsumerId::new("my-app-storybook").expect("valid consumer id"),
            Languages::default(),
            i18n::apply_locale,
        );
        let readiness =
            gpui_storybook::init(cx, options).expect("valid Storybook configuration");

        cx.spawn(async move |cx| {
            let ready = readiness.await;
            if !ready.diagnostics.is_empty() {
                tracing::warn!(
                    diagnostics = ?ready.diagnostics,
                    "Storybook initialized with preference diagnostics"
                );
            }

            cx.update(|cx| {
                gpui_storybook::create_storybook_window(
                    "My App - Stories",
                    |window, cx| {
                        let stories = gpui_storybook::generate_stories(window, cx);
                        StorybookWindow::new(stories)
                    },
                    cx,
                );
            });
        })
        .detach();
    });
}
```

Awaiting readiness prevents the first frame from briefly using default
appearance or language values before saved preferences load. Handle
`StorybookInitError` instead of using `expect` in an application that needs
graceful startup recovery.

Use the title-bar **Layout** select to switch between Gallery and Dock
workspace. The consumer-scoped preference makes that selection the initial
layout on the next launch. Set top-level `window_mode = "gallery"` or
`window_mode = "dock"` in the active `storybook.toml` when the binary needs a
configured initial layout instead.

## Run the binary

Run the package that owns the entry point:

```bash
cargo run -p my-app-storybook
```

The `mcp` feature supports Linux and macOS and produces a compile-time error on
Windows. Linux MCP and startup-capture sessions use the same application under
Sway's wlroots headless backend. macOS uses GPUI's native image renderer. The
[automation guide](automation.md) covers the platform launch commands.

The window should list every linked story allowed by the active
`storybook.toml`. Continue with [Write stories](stories.md) if the gallery is
empty, then [Use the workbench](workbench.md) to add live controls.

## Troubleshooting startup

| Symptom | Action |
|---|---|
| The first frame uses the wrong theme or language | Create the first window only after the readiness task completes |
| No stories appear | Ensure the binary references the library containing the registrations, then check `allow` and `disable_story` |
| `init` returns a configuration error | Fix the active `storybook.toml`, language set, consumer ID, or persistence/path combination named by the error |
| Consumer text is not localized | Confirm `build.rs` tracks i18n assets and the generated embedded-module static is referenced in the locale adapter |
