# gpui-storybook-macros

[![Codecov: gpui-storybook-macros][codecov-badge]][codecov]
[![crates.io: gpui-storybook-macros][crate-badge]][crate]

`gpui-storybook-macros` implements the registration and metadata macros used by
GPUI Storybook. Applications should normally enable the default `macros` feature
on the [`gpui-storybook` facade][facade].

## Overview

- `#[story]` registers a stateful story, while `ComponentStory` generates a
  focusable wrapper for a component.
- `StoryControls` turns marked fields into typed live editors; unmarked fields
  remain story-owned state.
- `Substory` creates stable section keys, and `#[story_init]` registers
  application setup that runs before preference readiness.
- Registration macros preserve Rustdocs, source provenance, and static control
  metadata for catalog export.

Macro expansions use the facade's `gpui_storybook` dependency name.

[codecov-badge]: https://codecov.io/github/stayhydated/gpui-storybook/branch/master/graph/badge.svg?component=gpui-storybook-macros
[codecov]: https://codecov.io/github/stayhydated/gpui-storybook
[crate-badge]: https://img.shields.io/crates/v/gpui-storybook-macros.svg?label=gpui-storybook-macros
[crate]: https://crates.io/crates/gpui-storybook-macros
[facade]: https://crates.io/crates/gpui-storybook
