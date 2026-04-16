# Architecture

## Purpose

`gpui-storybook` is the public facade crate. It re-exports the core runtime types and proc macros, owns story discovery, and applies `storybook.toml` filtering before handing stories to the UI runtime.

## Key entry points

- `init`: Registers the current language and locale manager, then delegates to the core story initialization. It also executes any global init hooks registered via the inventory system.
- `generate_stories`: Collects `StoryEntry` inventory records, resolves each entry's optional crate-local group from `storybook.toml`, loads the runtime `storybook.toml` either from the current binary crate or by searching upward from the working directory, then applies `allow` and `disable_story` filtering before sorting and constructing `StoryContainer` entities. `allow` matches the crate group when present and otherwise falls back to the story's declared section.
- `create_new_window` and `create_new_window_with_ui`: Re-export the standard storybook window helpers from the core crate.
- `create_dock_window`, `StoryWorkspace`, and `register_story_panels`: Available behind the `dock` feature for the docked workspace.

## Data flow

1. Story types are registered at compile time through either `#[gpui_storybook::story]` or `#[derive(gpui_storybook::ComponentStory)]`.
1. `#[story]` expects an explicit story view type that implements `gpui_storybook::Story`.
1. `#[derive(ComponentStory)]` keeps the component type component-focused and generates an internal wrapper story around the component example expression.
1. App startup calls `gpui_storybook::init`, wiring language, locale, and core runtime setup.
1. `generate_stories` reads inventory entries, merges optional crate-local grouping with runtime filtering config, preserves any declared story section beneath that group, orders the results, and returns story containers.
1. `Gallery::view` (core crate) renders the sidebar and active story content.

## Extension points

- `Story` trait for custom story definitions and view creation.
- `#[gpui_storybook::story]` for explicit story structs.
- `#[derive(gpui_storybook::ComponentStory)]` for component-attached story registration with generated wrapper views.
- `#[gpui_storybook::story_init]` for global initialization hooks.
- `StorybookWindowUi` for adding app-menu items and custom title-bar content to the standard window shell.
- `macros` feature flag controls whether proc macros are re-exported.
- `dock` feature flag controls dock workspace exports.

## Dependencies

- `gpui-storybook-core` provides the runtime.
- `gpui-storybook-macros` is optional and re-exported behind the `macros` feature.
- `gpui-storybook-toml` parses crate-local discovery config from `storybook.toml`.
- `inventory` backs registration and discovery.
- `tracing` logs discovery details.
