# gpui-storybook-macros

`gpui-storybook-macros` implements GPUI Storybook's registration macros:

- `#[story]` registers a stateful story.
- `#[derive(StoryControls)]` generates typed live editors for marked story fields.
- `#[derive(ComponentStory)]` generates a story wrapper for a component.
- `#[derive(Substory)]` creates stable capture keys for sections inside a story.
- `#[story_init]` registers one-time application setup.

Most applications should enable the default `macros` feature on
[`gpui-storybook`](../gpui-storybook/README.md) instead of depending on this
crate directly. Macro expansions reference the facade as `gpui_storybook`, so
direct macro users still need that dependency name available.

Controls are opt-in. Supported fields include `bool`, `i8` through `i64`,
`isize`, `u8` through `u32`, `usize`, `f32`, `f64`, `String`, `SharedString`,
and `Hsla`. Enum-like fields use explicit string options and implement
`Display` plus `FromStr`:

```rust
#[derive(gpui_storybook::StoryControls)]
struct ButtonStory {
    #[storybook(control(category = "State"))]
    disabled: bool,
    #[storybook(control(min = 0.0, max = 32.0, step = 1.0))]
    padding: f32,
    #[storybook(control(skip))]
    focus_handle: gpui::FocusHandle,
}
```

The same field attributes work directly on a `ComponentStory`. An explicitly
controlled unsupported type produces a compile error; leave it unmarked or use
`control(skip)`.

See [Write stories](../../book/src/stories.md) for supported patterns and
[Use the workbench](../../book/src/workbench.md) for runtime behavior, and
[docs.rs](https://docs.rs/gpui-storybook-macros/) for the macro API.
