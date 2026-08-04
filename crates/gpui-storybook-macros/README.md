# gpui-storybook-macros

`gpui-storybook-macros` implements GPUI Storybook's registration macros:

- `#[story]` registers a stateful story.
- `#[derive(ComponentStory)]` generates a story wrapper for a component.
- `#[derive(Substory)]` creates stable capture keys for sections inside a story.
- `#[story_init]` registers one-time application setup.

Most applications should enable the default `macros` feature on
[`gpui-storybook`](../gpui-storybook/README.md) instead of depending on this
crate directly. Macro expansions reference the facade as `gpui_storybook`, so
direct macro users still need that dependency name available.

See [Write stories](../../book/src/stories.md) for supported patterns and
[docs.rs](https://docs.rs/gpui-storybook-macros/) for the macro API.
