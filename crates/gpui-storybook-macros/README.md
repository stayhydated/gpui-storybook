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
    focus_handle: gpui::FocusHandle,
}
```

The same field attributes work directly on a `ComponentStory`. An explicitly
controlled unsupported type produces a compile error. Only marked fields are
registered, so leave other fields unmarked.

`ComponentStory` accepts `scenarios = expression` when the component owns a
`Vec<gpui_storybook::StoryScenario>` declaration. The generated wrapper exposes
that vector through `Story::scenarios()`, matching explicit stateful stories:

```rust
#[derive(gpui::IntoElement, gpui_storybook::ComponentStory)]
#[storybook(
    example = WelcomeCard::example(),
    scenarios = WelcomeCard::scenarios(),
)]
struct WelcomeCard {
    // ...
}
```

Both registration macros attach declaration Rustdocs and static marked-control
metadata to the inventory entry. `gpui_storybook::static_story_catalog()` can
export those keys, source locations, docs, editor kinds, bounds, and options
without constructing the generated or explicit story.

See [Write stories](../../book/src/stories.md) for supported patterns and
[Use the workbench](../../book/src/workbench.md) for runtime behavior, and
[docs.rs](https://docs.rs/gpui-storybook-macros/) for the macro API.
