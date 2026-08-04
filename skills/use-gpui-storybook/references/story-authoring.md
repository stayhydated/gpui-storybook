# Story authoring

Read this reference when adding registrations, metadata, sections, substories,
or one-time setup.

## Choose a registration

Use `#[gpui_storybook::story(...)]` when the preview owns GPUI state, a focus
handle, actions, lifecycle, or wrapper UI. Implement `Story`, `Render`, and
`Focusable`.

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
