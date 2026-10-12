# gpui-storybook-macros

[![Codecov: gpui-storybook-macros][codecov-badge]][codecov]
[![crates.io: gpui-storybook-macros][crate-badge]][crate]

`gpui-storybook-macros` implements the registration and metadata macros used by
GPUI Storybook. Applications should normally enable the default `macros` feature
on the [`gpui-storybook` facade][facade].

Requires Rust 1.99 or newer. Generated code uses the facade
and its `gpui_storybook` dependency name.

## Overview

- `#[story]` registers a stateful story, while `ComponentStory` generates a
  focusable wrapper for a component.
- `StoryControls` turns marked fields into typed live editors; unmarked fields
  remain story-owned state.
- `Substory` creates stable section keys, and `#[story_init]` registers
  application setup that runs before preference readiness.
- Registration macros preserve Rustdocs, source provenance, and static control
  metadata for catalog export.

See the [macro API reference][api] and the [maintained registration examples][examples].

[codecov-badge]: https://codecov.io/gh/stayhydated/gpui-storybook/branch/master/graph/badge.svg?component=gpui-storybook-macros
[codecov]: https://codecov.io/gh/stayhydated/gpui-storybook
[crate-badge]: https://img.shields.io/crates/v/gpui-storybook-macros.svg?label=gpui-storybook-macros
[crate]: https://crates.io/crates/gpui-storybook-macros
[facade]: https://crates.io/crates/gpui-storybook
[api]: https://docs.rs/gpui-storybook-macros
[examples]: https://github.com/stayhydated/gpui-storybook/tree/master/examples
