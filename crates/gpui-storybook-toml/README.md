# gpui-storybook-toml

[![Docs](https://docs.rs/gpui-storybook-toml/badge.svg)](https://docs.rs/gpui-storybook-toml/)
[![Crates.io](https://img.shields.io/crates/v/gpui-storybook-toml.svg)](https://crates.io/crates/gpui-storybook-toml)

Public integration crate for loading `storybook.toml`.

Most applications do not need to depend on this crate directly. They should create a `storybook.toml` file and let [`gpui-storybook`](../gpui-storybook/README.md) consume it automatically. Depend on this crate directly when you are writing tooling or a custom runtime flow around the same config format.

## `storybook.toml` format

```toml
group = "UI Kit"
allow = ["UI Kit", "Shared"]
disable_story = ["LegacyCardStory"]
```

- `group`: required when the file exists
- `allow`: optional group allowlist
- omitting `allow`: only the crate's own `group` is included
- `allow = ["*"]`: includes every group
- `allow = []`: includes none
- `disable_story`: optional denylist by registered story type name
- when consumed through `gpui-storybook`, `ComponentStory` registers the component type name

## Direct API use

```rs
let config = gpui_storybook_toml::load_from_dir("examples/story")?
    .expect("storybook.toml should exist");

assert_eq!(config.group(), Some("gpui-storybook-example-story"));
```
