# Project Overview

ignore all folders matching "**/__crate_paths__/**"

`gpui-storybook` is a storybook-style workspace for building and inspecting GPUI components. It focuses on:

1. **Fast iteration**: Preview component variants without running the full app.
1. **Organization**: Group stories into sections with stable ordering.
1. **Developer experience**: Built-in theme, locale, and appearance controls.

## Architecture Documentation Index

| Item | Docs | Purpose |
| -------------------------------- | ------------------------------------------------------------------------ | -------------------------------------------------------------------------- |
| **Facade and Runtime** | | |
| `gpui-storybook` | [Architecture](crates/gpui-storybook/docs/ARCHITECTURE.md) | Facade crate, public entry points, and story discovery/filtering. |
| `gpui-storybook-core` | [Architecture](crates/gpui-storybook-core/docs/ARCHITECTURE.md) | UI runtime: gallery, dock workspace, theming, i18n, assets, and window chrome. |
| `gpui-storybook-components` | [README](crates/gpui-storybook-components/README.md) | Shared dock-sidebar UI components used by the runtime. |
| `gpui-storybook-toml` | [Architecture](crates/gpui-storybook-toml/docs/ARCHITECTURE.md) | `storybook.toml` loader and filtering schema. |
| **Macros** | | |
| `gpui-storybook-macros` | [Architecture](crates/gpui-storybook-macros/docs/ARCHITECTURE.md) | Proc macros for story registration, component-derived registration, and init hooks. |
| **Examples** | | |
| `gpui-storybook-example-story` | [README](examples/story/README.md) | Story-struct example app using `#[story]`. |
| `gpui-storybook-example-component` | [README](examples/component/README.md) | Component-attached example app using `#[derive(ComponentStory)]`. |

## Crate Descriptions

### Core Layers

- **`gpui-storybook`**: User-facing library. Re-exports core types and macros, and provides `init` and `generate_stories` entry points.
- **`gpui-storybook-core`**: Runtime UI. Implements `Gallery`, dock workspace support, `StoryContainer`, theming, locale wiring, window helpers, and asset loading.
- **`gpui-storybook-components`**: Shared dock-sidebar widgets (`StorySidebarItem`, `StoryDrag`) used by the runtime.
- **`gpui-storybook-toml`**: Loads crate-local `storybook.toml` config for grouping, allowlists, and disabled stories.

### Macros

- **`gpui-storybook-macros`**: Provides `#[story]`, `#[derive(ComponentStory)]`, and `#[story_init]` macros that register inventory entries at compile time.

### Examples

- **`gpui-storybook-example-story`**: Demonstrates the explicit story-struct workflow.
- **`gpui-storybook-example-component`**: Demonstrates attaching story registration directly to the component type.

## Development

- **Rust**: Use `cargo` for building, testing, and running Rust code.
- **Formatting**: `cargo fmt` and `taplo fmt` for TOML.
- **Checks**: `cargo check --workspace --all-features --exclude gpui-storybook-example-story --exclude gpui-storybook-example-component`.
- **Tests**: `cargo test --workspace --all-features`.
- **Example apps**:
  - `cargo run -p gpui-storybook-example-story`
  - `cargo run -p gpui-storybook-example-component`

## Skills

| Item | Link to llms.txt | Link to llms-full.txt | Purpose |
| -------------- | ---------------------------------------------------- | --------------------------------------------------------- | -------------------------- |
| **Crate** | | | |
| es-fluent | https://stayhydated.github.io/es-fluent/llms.txt | https://stayhydated.github.io/es-fluent/llms-full.txt | i18n |
| gpui-component | https://longbridge.github.io/gpui-component/llms.txt | https://longbridge.github.io/gpui-component/llms-full.txt | gpui radix-like components |
