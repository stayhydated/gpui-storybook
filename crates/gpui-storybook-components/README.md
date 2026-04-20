# gpui-storybook-components

[![Docs](https://docs.rs/gpui-storybook-components/badge.svg)](https://docs.rs/gpui-storybook-components/)
[![Crates.io](https://img.shields.io/crates/v/gpui-storybook-components.svg)](https://crates.io/crates/gpui-storybook-components)

Internal crate with shared dock-sidebar UI pieces used by `gpui-storybook-core`.

This crate primarily exists for runtime implementation reuse. Most applications should depend on [`gpui-storybook`](../gpui-storybook/README.md), and lower-level runtime integrations should usually depend on [`gpui-storybook-core`](../gpui-storybook-core/README.md) instead.

Current surface:

- `StorySidebarItem`: dock sidebar row rendering
- `StoryDrag`: drag preview used when dropping stories into the dock workspace
