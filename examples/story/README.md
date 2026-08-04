# Explicit story example

This package demonstrates the `#[story]` workflow for previews that own GPUI
state, focus, actions, or custom wrapper UI.

Run the gallery:

```bash
cargo run -p gpui-storybook-example-story
```

Run the dock workspace:

```bash
cargo run -p gpui-storybook-example-story --features dock
```

The registrations live under `src/stories`; `ButtonStory` also demonstrates
stable `Substory` capture routes. See [Write
stories](../../book/src/stories.md) for the registration contract and
[Automation and capture](../../book/src/automation.md) for MCP usage.
