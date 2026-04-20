# Architecture: gpui-storybook-macros

## Purpose

`gpui-storybook-macros` translates user-authored story declarations into `inventory` registrations that the facade crate can discover at runtime.

The crate has no runtime behavior of its own. Its job is to emit code that targets the facade crate's hidden `__inventory` and `__registry` re-exports in a stable way.

## Macro contracts

### `#[story]`

- input must be a struct item
- accepts either no argument, a string literal section, or an enum variant path
- expands the struct unchanged and appends an `inventory::submit!` block for `StoryEntry`
- registers `create_fn` as `StoryContainer::panel::<StoryType>`
- records `CARGO_PKG_NAME`, `CARGO_MANIFEST_DIR`, `file!()`, and `line!()` for later discovery and diagnostics

Section handling:

- string literal: stored as `section`
- enum variant path: last path segment becomes the section label
- enum variant path: the path is cast to `usize` and stored as `section_order`

### `#[derive(ComponentStory)]`

- input must be a non-generic struct
- reads helper attributes from `#[storybook(...)]`
- supported keys are `title`, `description`, `section`, and `example`

The derive generates a hidden wrapper type named `__{Component}ComponentStoryView` that:

- stores a `FocusHandle`
- implements `Focusable`
- implements `Render` by rendering either `example = ...` or `<Component as Default>::default()`
- implements `Story` so the wrapper can be materialized through `StoryContainer::panel`

Defaults:

- title: `struct_name` with a trailing `Story` suffix removed, then converted with `heck::ToTitleCase`
- description: empty string
- example: `<Component as Default>::default()`

Registration name:

- the submitted `StoryEntry.name` is the original component type name
- this keeps `disable_story = ["ComponentName"]` aligned with the public type the user wrote, not the generated wrapper type

### `#[story_init]`

- input must be a function item
- expands the function unchanged and appends an `inventory::submit!` block for `InitEntry`
- records function pointer, function name, file, and line

## Shared expansion helpers

The crate is intentionally organized around a small shared token pipeline:

- `StoryArgs` parses `#[story(...)]`
- `ComponentStoryArgs` parses repeated `#[storybook(...)]` keys
- `parse_section_expr` normalizes `section = ...` values for the derive
- `section_tokens` converts a parsed section into `(section, section_order)` token pairs
- `registration_tokens` builds the final `StoryEntry` submission block used by both `#[story]` and `ComponentStory`

That shared helper path is the main guard against drift between attribute-based and derive-based story registration.

## Coupling to the facade crate

Generated code always refers to:

- `gpui_storybook::__inventory`
- `gpui_storybook::__registry`
- `gpui_storybook::StoryContainer`
- `gpui_storybook::Story`

This means:

- the facade crate, not this proc-macro crate, owns the stable public expansion path
- direct use of `gpui-storybook-macros` still expects `gpui_storybook` to be present under that crate name

## Tests

Snapshot tests validate macro expansion with `insta`, `prettyplease`, and `quote`. The tests focus on emitted wrapper types, registration blocks, and string-expression handling so expansion changes stay reviewable.
