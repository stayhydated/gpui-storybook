# gpui-storybook-toml

[![Codecov: gpui-storybook-toml][codecov-badge]][codecov]
[![crates.io: gpui-storybook-toml][crate-badge]][crate]

`gpui-storybook-toml` provides the typed `storybook.toml` schema, loader, and
filter evaluation for tools that need configuration without the GPUI runtime.
The [`gpui-storybook` facade][facade] selects the active configuration and
applies it during initialization and story discovery.

## Example

```toml
group = "UI Kit"
window_mode = "dock"
allow = ["UI Kit", "Shared"]
disable_story = ["ExperimentalCardStory"]

[overrides]
color_scheme = "dark"
theme = "Default Dark"
language = "en"
```

`group` is required when the file exists. An omitted `allow` includes the
file's own normalized group; `["*"]` includes every group, while `[]` includes
none. `disable_story` matches registered Rust type names rather than display
titles or automation keys.

[codecov-badge]: https://codecov.io/github/stayhydated/gpui-storybook/branch/master/graph/badge.svg?component=gpui-storybook-toml
[codecov]: https://codecov.io/github/stayhydated/gpui-storybook
[crate-badge]: https://img.shields.io/crates/v/gpui-storybook-toml.svg?label=gpui-storybook-toml
[crate]: https://crates.io/crates/gpui-storybook-toml
[facade]: https://crates.io/crates/gpui-storybook
