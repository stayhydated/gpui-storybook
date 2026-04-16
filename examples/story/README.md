# gpui-storybook-example-story

This example uses the original attribute-based story registration flow.

Run it with:

```bash
cargo run -p gpui-storybook-example-story
```

Or with the dock workspace:

```bash
cargo run -p gpui-storybook-example-story --features dock
```

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
```

`allow` is intentionally omitted here, so the example includes only its own `group`.
