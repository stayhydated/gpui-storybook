# gpui-storybook-core

[![Docs](https://docs.rs/gpui-storybook-core/badge.svg)](https://docs.rs/gpui-storybook-core/)
[![Crates.io](https://img.shields.io/crates/v/gpui-storybook-core.svg)](https://crates.io/crates/gpui-storybook-core)

`gpui-storybook-core` contains the runtime UI for the storybook experience: the gallery, story panels, theming, i18n wiring, and asset loading.

Most users should depend on `gpui-storybook` instead, which re-exports the types you need and enables macros by default.

## Installation

```toml
[dependencies]
gpui-storybook-core = "0.5"
```

## Usage

```rust
use gpui_storybook_core::{Gallery, assets::Assets, story::create_new_window};
```

This crate expects you to wire language selection, locale updates, and story registration via the facade crate or your own wrappers.
