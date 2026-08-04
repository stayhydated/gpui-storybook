# Component story example

This package demonstrates `#[derive(ComponentStory)]` for components that can
render from example data while Storybook supplies the focusable wrapper.

Run the gallery:

```bash
cargo run -p gpui-storybook-example-component
```

Run the dock workspace:

```bash
cargo run -p gpui-storybook-example-component --features dock
```

The registrations live under `src/components` and show literal, computed, and
localized metadata. See [Write stories](../../book/src/stories.md) for the
derive contract and [Getting started](../../book/src/getting_started.md) for
application setup.
