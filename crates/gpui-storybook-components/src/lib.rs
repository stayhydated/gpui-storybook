//! Shared dock-sidebar UI primitives used by `gpui-storybook-core`.
//!
//! This crate is an implementation detail of the runtime. Applications should
//! usually depend on `gpui-storybook`, and lower-level runtime integrations
//! should usually depend on `gpui-storybook-core`.

mod sidebar;

pub use sidebar::{StoryDrag, StorySidebarItem};
