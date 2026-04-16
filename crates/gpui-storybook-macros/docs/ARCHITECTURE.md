# Architecture

## Purpose

`gpui-storybook-macros` provides proc macros that register stories and init hooks with the inventory system.

## Macros

- `#[story]` registers a story struct as a `StoryEntry`:
  - Accepts no argument, a string literal section name, or an enum variant path.
  - For enum variants, the last path segment is used as the section label and the discriminant is used as `section_order`.
  - Emits an `inventory::submit!` block with crate metadata (`CARGO_PKG_NAME`, `CARGO_MANIFEST_DIR`), file/line metadata, and a `StoryContainer::panel` factory.
- `#[derive(ComponentStory)]` turns a component type into a registered story via a generated wrapper:
  - Targets non-generic structs.
  - Reads `#[storybook(title = ..., description = ..., section = ..., example = ...)]` helper attributes.
  - `section` accepts the same forms as `#[story]`: either a string literal or an enum variant path.
  - `title` and `description` accept expressions that convert into `String`.
  - Generates a hidden wrapper view that implements `Story`, `Render`, and `Focusable`.
  - The wrapper renders either `example = ...` or `<Component as Default>::default()` and registers that wrapper in inventory under the component type name, which is also the name matched by `disable_story`.
- `#[story_init]` registers an initialization function as an `InitEntry`:
  - Emits an `inventory::submit!` block with the function pointer and file/line metadata.

## Parsing and expansion

- `StoryArgs` parses `#[story(...)]` arguments into either a string literal or a `syn::Path`.
- `ComponentStoryArgs` parses `#[storybook(...)]` helper attributes for the derive macro.
- Shared registration helpers build the final `StoryEntry` token stream so the attribute and derive macros stay aligned.
- `component_story_impl` generates the internal wrapper type plus the final registration block.
- `story_impl` and `story_init_impl` expand input items and append their registration blocks.

## Tests

Snapshot tests validate macro output using `insta`, `quote`, and `prettyplease` to keep expansions stable.

## Dependencies

The macros expect the facade crate to export `__registry` and `__inventory`, so the generated code can refer to `gpui_storybook::__registry` consistently.
