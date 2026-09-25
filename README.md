# GPUI Storybook

[![CI][ci-badge]][ci]
[![Codecov][codecov-badge]][codecov]
[![Book][book-badge]][book]
[![crates.io: gpui-storybook][gpui-storybook-badge]][gpui-storybook-crate]

GPUI Storybook gives application developers a searchable gallery
for developing, inspecting, and exercising GPUI components outside
the rest of their application.

## Overview

- Register stateful previews with `#[story]` or derive component previews with
  `ComponentStory`.
- Edit typed controls, inspect scoped actions, switch themes and viewports, and
  run repeatable scenarios from the workbench.
- Drive live Linux and macOS sessions through MCP, or exercise isolated stories
  with the portable headless test runner.

## Crates

| Crate | Purpose | Source |
| --- | --- | --- |
| `gpui-storybook` | Application facade, initialization, registration, and public re-exports | [README](crates/gpui-storybook/README.md) |
| `gpui-storybook-core` | Runtime shell and lower-level integration APIs | [README](crates/gpui-storybook-core/README.md) |
| `gpui-storybook-launch` | Linux headless Sway launcher for automation sessions | [README](crates/gpui-storybook-launch/README.md) |
| `gpui-storybook-mcp` | Typed live automation and story-region capture tools | [README](crates/gpui-storybook-mcp/README.md) |
| `gpui-storybook-test` | Portable capture matrices, visual baselines, and frame budgets | [README](crates/gpui-storybook-test/README.md) |
| `gpui-storybook-toml` | Typed `storybook.toml` schema and loader | [README](crates/gpui-storybook-toml/README.md) |
| `gpui-storybook-example-component` | Executable `ComponentStory` registration examples | [README](examples/component/README.md) |
| `gpui-storybook-example-story` | Executable stateful `#[story]` registration examples | [README](examples/story/README.md) |

[ci-badge]: https://github.com/stayhydated/gpui-storybook/actions/workflows/ci.yml/badge.svg?branch=master
[ci]: https://github.com/stayhydated/gpui-storybook/actions/workflows/ci.yml
[codecov-badge]: https://codecov.io/github/stayhydated/gpui-storybook/branch/master/graph/badge.svg
[codecov]: https://codecov.io/github/stayhydated/gpui-storybook
[book-badge]: https://img.shields.io/badge/Book-mdBook-blue
[book]: https://stayhydated.github.io/gpui-storybook/book/
[gpui-storybook-badge]: https://img.shields.io/crates/v/gpui-storybook.svg?label=gpui-storybook
[gpui-storybook-crate]: https://crates.io/crates/gpui-storybook
