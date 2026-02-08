# Architecture

## Purpose

`gpui-storybook-macros` provides proc macros that register stories and init hooks with the inventory system.

## Macros

- `#[story]` registers a story struct as a `StoryEntry`:
  - Accepts no argument, a string literal section name, or an enum variant path.
  - For enum variants, the last path segment is used as the section label and the discriminant is used as `section_order`.
  - Emits an `inventory::submit!` block with file/line metadata and a `StoryContainer::panel` factory.
- `#[story_init]` registers an initialization function as an `InitEntry`:
  - Emits an `inventory::submit!` block with the function pointer and file/line metadata.

## Parsing and expansion

- `StoryArgs` parses the attribute argument into either a string literal or a `syn::Path`.
- `story_impl` and `story_init_impl` expand the input items and append the registration blocks.

## Tests

Snapshot tests validate macro output using `insta`, `quote`, and `prettyplease` to keep expansions stable.

## Dependencies

The macros expect the facade crate to export `__registry` and `__inventory`, so the generated code can refer to `gpui_storybook::__registry` consistently.
