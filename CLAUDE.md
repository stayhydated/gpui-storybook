# Project Overview

ignore all folders matching "**/\_\_crate_paths/**"

`gpui-storybook` is a storybook-style workspace for building and inspecting GPUI components. It focuses on:

1. **Fast iteration**: Preview component variants without running the full app.
1. **Organization**: Group stories into sections with stable ordering.
1. **Developer experience**: Built-in theme, locale, and appearance controls.

## Architecture Documentation Index

| Crate | Link to Architecture Doc | Purpose |
| --- | --- | --- |
| **Core** | | |
| `gpui-storybook` | [Architecture](crates/gpui-storybook/docs/ARCHITECTURE.md) | Facade crate, entry point, and story discovery. |
| `gpui-storybook-core` | [Architecture](crates/gpui-storybook-core/docs/ARCHITECTURE.md) | UI runtime: gallery, story panels, theming, i18n, assets. |
| **Macros** | | |
| `gpui-storybook-macros` | [Architecture](crates/gpui-storybook-macros/docs/ARCHITECTURE.md) | Proc macros for story registration and init hooks. |
| **Examples** | | |
| `gpui-storybook-example` | | End-to-end sample app and stories. |

## Crate Descriptions

### Core Layers

- **`gpui-storybook`**: User-facing library. Re-exports core types and macros, and provides `init` and `generate_stories` entry points.
- **`gpui-storybook-core`**: Runtime UI. Implements `Gallery`, `StoryContainer`, theming, locale wiring, and asset loading.

### Macros

- **`gpui-storybook-macros`**: Provides `#[story]` and `#[story_init]` macros that register inventory entries at compile time.

### Examples

- **`gpui-storybook-example`**: Demonstrates a full GPUI app wired to the storybook runtime, including language setup.

## Development

- **Rust**: Use `cargo` for building, testing, and running Rust code.
- **Formatting**: `cargo fmt` and `taplo fmt` for TOML.
- **Checks**: `cargo check --workspace --all-features --exclude gpui-storybook-example`.
- **Tests**: `cargo test --workspace --all-features`.
- **Example app**: `cargo run -p gpui-storybook-example`.
