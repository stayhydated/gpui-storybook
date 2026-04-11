# Architecture

## Purpose

`gpui-storybook` is a thin facade crate. It re-exports the core runtime types and the proc macros so downstream crates can depend on a single package.

## Key entry points

- `init`: Registers the current language and locale manager, then delegates to the core story initialization. It also executes any global init hooks registered via the inventory system.
- `generate_stories`: Collects `StoryEntry` inventory records, resolves each entry's group from its crate-local `storybook.toml`, then applies runtime `storybook.toml` filtering (`allow`, `disable_story`) before sorting by section/order and constructing `StoryContainer` entities.
- `create_new_window`: Re-export from the core crate for creating the storybook window.

## Data flow

1. Story types are registered at compile time through either `#[gpui_storybook::story]` or `#[derive(gpui_storybook::ComponentStory)]`.
1. `#[story]` expects an explicit story view type that implements `gpui_storybook::Story`.
1. `#[derive(ComponentStory)]` keeps the component type component-focused and generates an internal wrapper story around the component example expression.
1. App startup calls `gpui_storybook::init`, wiring language, locale, and core runtime setup.
1. `generate_stories` reads inventory entries, orders them, and returns story containers.
1. `Gallery::view` (core crate) renders the sidebar and active story content.

## Extension points

- `Story` trait for custom story definitions and view creation.
- `#[gpui_storybook::story]` for explicit story structs.
- `#[derive(gpui_storybook::ComponentStory)]` for component-attached story registration with generated wrapper views.
- `#[gpui_storybook::story_init]` for global initialization hooks.
- `macros` feature flag controls whether proc macros are re-exported.

## Dependencies

- `gpui-storybook-core` provides the runtime.
- `gpui-storybook-macros` is optional and re-exported behind the `macros` feature.
- `gpui-storybook-toml` parses crate-local discovery config from `storybook.toml`.
- `inventory` backs registration and discovery.
- `tracing` logs discovery details.
