# gpui-storybook-macros

[![Docs](https://docs.rs/gpui-storybook-macros/badge.svg)](https://docs.rs/gpui-storybook-macros/)
[![Crates.io](https://img.shields.io/crates/v/gpui-storybook-macros.svg)](https://crates.io/crates/gpui-storybook-macros)

Proc macros for `gpui-storybook`.

Most applications should use these macros through [`gpui-storybook`](../gpui-storybook/README.md). Depend on this crate directly only when you intentionally want the proc-macro crate without the facade.

## Macro surface

### `#[story]`

Registers an explicit story type that implements `gpui_storybook::Story`.

```rs
#[gpui_storybook::story("Components")]
pub struct ButtonStory;
```

Use this flow when the story needs custom state, focus handling, or a wrapper view around the component being previewed.

The registered story name is the story struct name. Macro-generated registry
entries also include a stable automation key in the form
`{crate-package-name}-{story-struct-name}` and a duplicate-key marker so two
stories with the same generated key fail to build.

### `#[derive(ComponentStory)]`

Generates a story wrapper around a component type.

```rs
#[derive(gpui::IntoElement, gpui_storybook::ComponentStory)]
#[storybook(
    title = "Welcome Card",
    description = "Component-owned example data with a generated story wrapper",
    section = crate::StorySection::Intro,
    example = WelcomeCard::example(),
)]
pub struct WelcomeCard;
```

Supported `#[storybook(...)]` arguments:

- `title = ...`
- `description = ...`
- `section = ...`
- `example = ...`

`title` and `description` are emitted inside generated `Story` methods that receive `cx: &gpui::App`, so those expressions can call app-scoped localization helpers.

The registered story name is the component type name, not the hidden wrapper
type. The stable automation key uses
`{crate-package-name}-{component-type-name}`.

### `#[derive(Substory)]`

Generates stable capture metadata for fieldless enum variants that are passed
to `gpui_storybook::section(...)` or `gpui_storybook::StorySectionBase::new(...)`
for custom section components.

```rs
#[derive(gpui_storybook::Substory)]
enum ButtonSubstory {
    NormalButton,
    #[substory(title = "Button with Icon")]
    ButtonWithIcon,
    #[substory(key = "progress", title = "With Progress")]
    WithProgress,
}

gpui_storybook::section(ButtonSubstory::ButtonWithIcon);
let base = gpui_storybook::StorySectionBase::new(ButtonSubstory::WithProgress);
```

By default, the capture key is the variant name in kebab case and the visible
title is title case. Use `title` to change display text without changing the
capture route. Use `key` before renaming a variant when an existing route must
remain stable.

### `#[story_init]`

Registers one-time application setup that runs during `gpui_storybook::init(...)`.

```rs
#[gpui_storybook::story_init]
fn register_icons(cx: &mut gpui::App) {
    // global setup
}
```
