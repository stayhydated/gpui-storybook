# gpui-storybook-toml

[![CI](https://github.com/stayhydated/gpui-storybook/actions/workflows/ci.yml/badge.svg)](https://github.com/stayhydated/gpui-storybook/actions/workflows/ci.yml)
[![Codecov](https://codecov.io/github/stayhydated/gpui-storybook/graph/badge.svg)](https://codecov.io/github/stayhydated/gpui-storybook)
[![Book](https://img.shields.io/badge/book-online-blue)](https://stayhydated.github.io/gpui-storybook/book/)
[![crates.io](https://img.shields.io/crates/v/gpui-storybook-toml.svg)](https://crates.io/crates/gpui-storybook-toml)

`gpui-storybook-toml` parses a crate-local `storybook.toml`, exposes its typed
initial window mode and launch presentation overrides, and evaluates its group
and story filters. It is a public integration crate for tools that need the
configuration schema without the GPUI runtime.

The facade crate selects the active runtime configuration and applies it during
initialization and story generation. Most applications should depend on
[`gpui-storybook`](../gpui-storybook/README.md) and configure it with:

```toml
group = "UI Kit"
window_mode = "dock"
allow = ["UI Kit", "Shared"]
disable_story = ["ExperimentalCardStory"]
```

`window_mode` accepts `"gallery"` or `"dock"`. The facade uses it as the
initial mode when a window does not supply `StorybookWindow::with_mode`; the
title-bar selector can still change and save the user's later choice.

Omitting `allow` includes only this file's group. Use `allow = ["*"]` to
include every linked group or `allow = []` to include none. `disable_story`
matches registered type names, such as `WelcomeCard`, independently of display
titles and automation keys.
