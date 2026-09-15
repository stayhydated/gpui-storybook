# gpui-storybook

[![CI](https://github.com/stayhydated/gpui-storybook/actions/workflows/ci.yml/badge.svg)](https://github.com/stayhydated/gpui-storybook/actions/workflows/ci.yml)
[![Codecov](https://codecov.io/github/stayhydated/gpui-storybook/graph/badge.svg)](https://codecov.io/github/stayhydated/gpui-storybook)
[![Book](https://img.shields.io/badge/book-online-blue)](https://stayhydated.github.io/gpui-storybook/book/)
[![crates.io](https://img.shields.io/crates/v/gpui-storybook.svg)](https://crates.io/crates/gpui-storybook)

GPUI Storybook previews components in a searchable gallery or dock workspace.
Register stateful stories or derive previews from components, then edit typed
controls, inspect actions, and run repeatable scenarios without navigating the
rest of your application.

## Register a preview

Use `#[story]` for a preview that owns GPUI state, focus, or action handlers.
Use `ComponentStory` when an existing component can render from example data:

```rust
#[derive(gpui_kit::IntoElement, gpui_storybook::ComponentStory)]
#[storybook(
    title = "Welcome Card",
    section = "Components",
    example = WelcomeCard::example(),
)]
pub struct WelcomeCard {
    #[storybook(control)]
    title: gpui_kit::SharedString,
    #[storybook(control)]
    highlighted: bool,
}
```

The component supplies `RenderOnce` and the `example()` constructor. Storybook
creates its focusable wrapper and uses the example's values as control reset
defaults. For explicit stories, implement `Story`, `Render`, and `Focusable`,
then derive `StoryControls`. Only fields marked with `#[storybook(control)]`
become live editors.

Initialize through the `gpui-storybook` facade, await preference readiness, and
open a window with `create_storybook_window` and `StorybookWindow::new`. Keep
the crate containing registrations linked from the binary.

## Explore stories

The title-bar **Layout** selector switches between **Gallery** and **Dock
workspace** and saves the choice per binary. Stories with the same title, group,
and section share a navigation entry; **Variant** selects a concrete preview.
Dock mode keeps selected variants in separate tabs.

The workbench provides:

- **Controls** for live field values and reset behavior;
- **Scenarios** for named interaction flows that start from a fresh story;
- **Theme** for session color edits and import/export;
- **Inspect** for the active story key and source location;
- **Actions** for commands and key bindings on an explicit story action scope.

Mobile, Tablet, and Desktop presets size the preview. Responsive mode provides
resize handles. Appearance, language, and layout preferences are scoped to a
stable `ConsumerId` for each Storybook binary.

## Optional features

| Feature | Default | Purpose |
|---|---:|---|
| `macros` | Yes | Re-export story registration and control derives |
| `inspector` | No | Open GPUI Inspector with story-root metadata |
| `performance` | No | Show GPUI frame timing histograms and the debug overlay |
| `mcp` | No | Expose Linux/macOS MCP tools, interaction, and PNG capture |

MCP clients can discover stable routes, set typed controls, inspect rendered
values, and capture story regions. Generic interaction requires
`GPUI_STORYBOOK_MCP_ALLOW_INTERACTION=1`; batches report partial failures and
are never retried automatically. Linux sessions use the Sway-backed
`gpui-storybook-launch` command. macOS sessions launch directly through Cargo.

Use `gpui-storybook-test` for isolated story tests, capture matrices, explicit
visual-baseline checks, and optional frame budgets. Each case constructs a fresh
GPUI context; capture support depends on the platform renderer.
