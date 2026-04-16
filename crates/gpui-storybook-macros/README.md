# gpui-storybook-macros

Proc macros for `gpui-storybook`.

Exports:

- `#[story]` to register an explicit story type that implements `gpui_storybook::Story`.
- `#[derive(ComponentStory)]` to generate and register an internal story wrapper around a component.
- `#[story_init]` to register one-time global initialization hooks.

`ComponentStory` supports `#[storybook(title = ..., description = ..., section = ..., example = ...)]`.

Most applications use these macros through the `gpui-storybook` facade crate.

Architecture notes: [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)
