# gpui-storybook-example-component

This example uses the component-attached derive macro with components that are unrelated to the `examples/story` set.

Current components:

- `WelcomeCard`: an editorial callout card.
- `SignalBoard`: a custom dashboard strip.
- `FieldNotes`: a stack of annotated note cards.

Pattern:

```rust
use gpui::{IntoElement, RenderOnce};

#[derive(IntoElement, gpui_storybook::ComponentStory)]
#[storybook(
    title = "Welcome Card",
    section = crate::StorySection::Intro,
    example = WelcomeCard::example(),
)]
pub struct WelcomeCard {
    // component data only
}

impl RenderOnce for WelcomeCard {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        // component render only
    }
}
```

The derive macro generates the story wrapper, focus handling, and inventory registration. The component crate only defines the component itself and, when needed, an `example = ...` constructor.

Current config:

```toml
group = "gpui-storybook-example-component"
allow = ["gpui-storybook-example-component"]
```
