# gpui-storybook-example

This example demonstrates crate-level story discovery config using `storybook.toml`.

Current config:

```toml
group = "gpui-storybook-example"
allow = ["gpui-storybook-example"]
disable_story = ["TableStory"]
```

Behavior:

- All discovered stories are grouped under `gpui-storybook-example`.
- `allow` targets group identifiers (not story names); this example explicitly allows its own group.
- `TableStory` is still compiled but excluded from discovery via `disable_story`.
