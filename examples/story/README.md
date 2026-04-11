# gpui-storybook-example-story

This example uses the original attribute-based story registration flow.

Pattern:

```rust
#[gpui_storybook::story(crate::StorySection::Buttons)]
pub struct ButtonStory;

impl gpui_storybook::Story for ButtonStory {
    fn title() -> String {
        "Button".into()
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render + Focusable> {
        Self::view(window, cx)
    }
}
```

Current config:

```toml
group = "gpui-storybook-example-story"
allow = ["gpui-storybook-example-story"]
```
