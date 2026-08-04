# gpui-storybook-toml

`gpui-storybook-toml` parses a crate-local `storybook.toml` and evaluates its
group and story filters. It is a public integration crate for tools that need
the configuration schema without the GPUI runtime.

The facade crate selects the active runtime configuration and applies it during
initialization and story generation. Most applications should depend on
[`gpui-storybook`](../gpui-storybook/README.md) and configure it with:

```toml
group = "UI Kit"
allow = ["UI Kit", "Shared"]
disable_story = ["ExperimentalCardStory"]
```

See [Configure Storybook](../../book/src/configuration.md) for field semantics
and [docs.rs](https://docs.rs/gpui-storybook-toml/) for the loader API.
