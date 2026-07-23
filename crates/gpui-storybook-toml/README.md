# gpui-storybook-toml

[![Docs](https://docs.rs/gpui-storybook-toml/badge.svg)](https://docs.rs/gpui-storybook-toml/)
[![Crates.io](https://img.shields.io/crates/v/gpui-storybook-toml.svg)](https://crates.io/crates/gpui-storybook-toml)

Public integration crate for loading `storybook.toml`.

Most applications do not need to depend on this crate directly. They should create a `storybook.toml` file and let [`gpui-storybook`](../gpui-storybook/README.md) consume it automatically. Depend on this crate directly when you are writing tooling or a custom runtime flow around the same config format.

Through `gpui-storybook`, `init` and `generate_stories` use the
`storybook.toml` from the registered story crate whose package name matches the
running binary.

## `storybook.toml` format

```toml
group = "UI Kit"
allow = ["UI Kit", "Shared"]
disable_story = ["ExperimentalCardStory"]

[overrides]
color_scheme = "dark"
theme = "Default Dark"
language = "en"
```

- `group`: required when the file exists
- `allow`: optional group allowlist
- omitting `allow`: only the crate's own `group` is included
- `allow = ["*"]`: includes every group
- `allow = []`: includes none
- `disable_story`: optional denylist by registered story type name
- when consumed through `gpui-storybook`, `ComponentStory` registers the component type name
- `[overrides]`: optional deterministic effective-presentation overrides
- `color_scheme`: optional `"light"` or `"dark"` override
- `theme`: optional registered theme name for the effective color scheme
- `language`: optional BCP 47 tag from the consumer's typed embedded language set

The facade applies TOML preference overrides during `init` without rewriting
saved intent. Programmatic `PreferenceOverrides` win field by field over TOML,
and deterministic MCP capture or stdio profiles win over both. Invalid runtime
config and languages outside the typed embedded set are initialization errors;
unavailable named themes use the registered fallback and emit a resolution
diagnostic.

## Direct API use

```rs
let config = gpui_storybook_toml::load_from_dir("examples/story")?
    .expect("storybook.toml should exist");

assert_eq!(config.group(), Some("gpui-storybook-example-story"));
```
