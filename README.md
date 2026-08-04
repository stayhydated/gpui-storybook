# GPUI Storybook

[![Build Status](https://github.com/stayhydated/gpui-storybook/actions/workflows/ci.yml/badge.svg)](https://github.com/stayhydated/gpui-storybook/actions/workflows/ci.yml)
[![Codecov](https://codecov.io/github/stayhydated/gpui-storybook/graph/badge.svg)](https://codecov.io/github/stayhydated/gpui-storybook)
[![Docs](https://docs.rs/gpui-storybook/badge.svg)](https://docs.rs/gpui-storybook/)
[![Crates.io](https://img.shields.io/crates/v/gpui-storybook.svg)](https://crates.io/crates/gpui-storybook)

GPUI Storybook is a searchable component preview shell for GPUI applications. It
supports stateful stories, component-derived stories, persistent appearance and
language preferences, an optional dock workspace, and optional MCP automation
and PNG capture.

## Try the examples

Run the explicit `#[story]` example:

```bash
cargo run -p gpui-storybook-example-story
```

Run the `#[derive(ComponentStory)]` example:

```bash
cargo run -p gpui-storybook-example-component
```

Add `--features dock` to either command to open the dock workspace.

## Start using Storybook

Most applications should depend on the `gpui-storybook` facade crate. Register
a component or a stateful story, initialize Storybook, await preference
readiness, and then open a gallery or dock window.

- [User guide](book/src/introduction.md)
- [Getting started](book/src/getting_started.md)
- [Story registration](book/src/stories.md)
- [Configuration](book/src/configuration.md)
- [Automation and capture](book/src/automation.md)
- [API documentation](https://docs.rs/gpui-storybook/)
