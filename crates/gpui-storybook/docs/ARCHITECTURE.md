# Architecture

## Purpose

`gpui-storybook` is a thin facade crate. It re-exports the core runtime types and (optionally) the proc macros so downstream crates can depend on a single package.

## Key entry points

- `init`: Registers the current language and locale manager, then delegates to the core story initialization. It also executes any global init hooks registered via the inventory system.
- `generate_stories`: Collects `StoryEntry` inventory records, sorts them by section and order, constructs `StoryContainer` entities, and applies section metadata.
- `create_new_window`: Re-export from the core crate for creating the storybook window.

## Data flow

1. Story structs are annotated with `#[gpui_storybook::story]` (macro feature) which registers inventory entries at compile time.
1. App startup calls `gpui_storybook::init`, wiring language, locale, and core runtime setup.
1. `generate_stories` reads inventory entries, orders them, and returns story containers.
1. `Gallery::view` (core crate) renders the sidebar and active story content.

## Extension points

- `Story` trait for story definitions and view creation.
- `#[gpui_storybook::story_init]` for global initialization hooks.
- `macros` feature flag controls whether proc macros are re-exported.

## Dependencies

- `gpui-storybook-core` provides the runtime.
- `gpui-storybook-macros` is optional and re-exported behind the `macros` feature.
- `inventory` backs registration and discovery.
- `tracing` logs discovery details.
