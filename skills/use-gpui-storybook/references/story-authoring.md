# Story authoring

Read this reference when adding registrations, metadata, sections, substories,
or one-time setup.

## Choose a registration

Use `#[gpui_storybook::story(...)]` when the preview owns GPUI state, a focus
handle, actions, lifecycle, or wrapper UI. Implement `Story`, `Render`, and
`Focusable`, and derive or implement `StoryControls`.

Use `#[derive(gpui_storybook::ComponentStory)]` when the component implements
`IntoElement` and Storybook can generate its wrapper. The derive supports
`title`, `description`, `section`, and `example`.

```rust
#[derive(gpui::IntoElement, gpui_storybook::ComponentStory)]
#[storybook(
    title = "Welcome Card",
    section = crate::StorySection::Intro,
    example = WelcomeCard::example(),
)]
pub struct WelcomeCard {
    // component fields
}
```

The derive accepts non-generic structs. Without `example`, the wrapper uses
`Default::default()`. Title and description expressions have
`cx: &gpui::App` in scope and may call `localize_message`.

## Add live controls

Derive `StoryControls` on an explicit story and keep `Story::new_view` typed as
`Entity<Self>`:

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

On a `ComponentStory`, place the same attributes on component fields. The
generated wrapper obtains reset defaults from `example = ...`, reconstructs the
example during rendering, and overlays current values.

Supported inferred types are `bool`, signed integers through `i64`, unsigned
integers through `u32`, `usize`, `f32`, `f64`, `String`, `SharedString`, and
`Hsla`. Enum-like types use string `options` and implement `Display` plus
`FromStr`. Leave collections and application-specific types unmarked or use
`control(skip)`; an explicitly controlled unsupported field is a compile error.

## Organize stories

Both registration styles accept string sections or enum variants. Use a
`#[repr(usize)]` enum when stable section ordering matters; string sections
sort alphabetically.

Use `#[gpui_storybook::story_init]` for application setup that runs once after
the core runtime is installed and before preference readiness begins.

## Preserve route identity

Macro-generated base keys have this form:

```text
{cargo-package-name}-{registered-type-name}
```

Explicit stories use the story struct name. Component stories use the component
type name. Display title and description changes do not change the key.

Use `#[derive(gpui_storybook::Substory)]` for stable sections within a story:

```rust
#[derive(gpui_storybook::Substory)]
enum ButtonSubstory {
    NormalButton,
    #[substory(title = "Button with Icon")]
    ButtonWithIcon,
    #[substory(key = "progress", title = "With Progress")]
    WithProgress,
}
```

Variant names produce kebab-case keys. `title` changes display text only;
`key` changes the route segment. Pass the variant to `section(...)` or
`StorySectionBase::new(...)`.

## Diagnose missing stories

Check that:

- the binary links the story-bearing library and module;
- the active `allow` includes the crate group or section;
- `disable_story` does not contain the registered type name;
- generated and manual registrations do not reuse a key;
- the route came from `storybook_list_stories` or registration logs rather
  than a display label.
