# GPUI Storybook

[![CI][ci-badge]][ci]
[![Codecov][codecov-badge]][codecov]
[![Book][book-badge]][book]
[![crates.io: gpui-storybook][gpui-storybook-badge]][gpui-storybook-crate]

GPUI Storybook gives application developers a searchable gallery
for developing, inspecting, and exercising GPUI components outside
the rest of their application.

Requires Rust 1.99 or newer across the workspace. Native applications use GPUI
Kit 0.7.3; the capture integrations target published `gpui-pre =0.3.9`. Live MCP
sessions support Linux and macOS; the Sway launcher is Linux-only.

## Overview

- Register stateful previews with `#[story]` or derive component previews with
  `ComponentStory`.
- Edit typed controls, inspect scoped actions, switch themes and viewports, and
  run repeatable scenarios from the workbench.
- Drive live Linux and macOS sessions through MCP, or exercise isolated stories
  with the portable headless test runner.

Live captures size the story canvas to desktop, tablet, mobile, or explicit
pixel dimensions and crop gallery chrome from the PNG.

Linux Wayland window capture uses the GPUI Git patches pinned on the
[`wayland-render-image` branch][wayland-patches].
Applications using Storybook as a dependency must apply those patches in their
own workspace root.

## Crates

Start with [`gpui-storybook`][gpui-storybook-readme] for an application gallery. Use
`gpui-storybook-test` directly in portable integration tests; lower-level crates
serve custom hosts and tooling.

| Crate | Purpose |
| --- | --- |
| [`gpui-storybook`][gpui-storybook-readme] | Application facade for initialization, story discovery, and public re-exports. |
| [`gpui-storybook-core`][gpui-storybook-core-readme] | Lower-level gallery, workbench, preference UI, and automation runtime. |
| [`gpui-storybook-example-component`][gpui-storybook-example-component-readme] | Executable `ComponentStory` examples; not published. |
| [`gpui-storybook-example-story`][gpui-storybook-example-story-readme] | Executable stateful `#[story]` examples and portable capture fixture; not published. |
| [`gpui-storybook-launch`][gpui-storybook-launch-readme] | Linux command that owns the private Sway lifecycle for automation. |
| [`gpui-storybook-macros`][gpui-storybook-macros-readme] | Registration and control macro implementation, normally used through the facade. |
| [`gpui-storybook-mcp`][gpui-storybook-mcp-readme] | Typed live automation and story-region capture, normally enabled through the facade. |
| [`gpui-storybook-preferences`][gpui-storybook-preferences-readme] | Internal persistence and resolution engine; applications configure it through the facade. |
| [`gpui-storybook-test`][gpui-storybook-test-readme] | Direct integration-test entry point for capture matrices, visual baselines, and frame budgets. |
| [`gpui-storybook-toml`][gpui-storybook-toml-readme] | Configuration schema and loader for tools that do not need the GPUI runtime. |

[ci-badge]: https://github.com/stayhydated/gpui-storybook/actions/workflows/ci.yml/badge.svg?branch=master
[ci]: https://github.com/stayhydated/gpui-storybook/actions/workflows/ci.yml
[codecov-badge]: https://codecov.io/gh/stayhydated/gpui-storybook/branch/master/graph/badge.svg
[codecov]: https://codecov.io/gh/stayhydated/gpui-storybook
[book-badge]: https://img.shields.io/badge/Book-mdBook-blue
[book]: https://stayhydated.github.io/gpui-storybook/book/
[gpui-storybook-badge]: https://img.shields.io/crates/v/gpui-storybook.svg?label=gpui-storybook
[gpui-storybook-crate]: https://crates.io/crates/gpui-storybook
[gpui-storybook-readme]: https://github.com/stayhydated/gpui-storybook/blob/master/crates/gpui-storybook/README.md
[gpui-storybook-core-readme]: https://github.com/stayhydated/gpui-storybook/blob/master/crates/gpui-storybook-core/README.md
[gpui-storybook-example-component-readme]: https://github.com/stayhydated/gpui-storybook/blob/master/examples/component/README.md
[gpui-storybook-example-story-readme]: https://github.com/stayhydated/gpui-storybook/blob/master/examples/story/README.md
[gpui-storybook-launch-readme]: https://github.com/stayhydated/gpui-storybook/blob/master/crates/gpui-storybook-launch/README.md
[gpui-storybook-macros-readme]: https://github.com/stayhydated/gpui-storybook/blob/master/crates/gpui-storybook-macros/README.md
[gpui-storybook-mcp-readme]: https://github.com/stayhydated/gpui-storybook/blob/master/crates/gpui-storybook-mcp/README.md
[gpui-storybook-preferences-readme]: https://github.com/stayhydated/gpui-storybook/blob/master/crates/gpui-storybook-preferences/README.md
[gpui-storybook-test-readme]: https://github.com/stayhydated/gpui-storybook/blob/master/crates/gpui-storybook-test/README.md
[gpui-storybook-toml-readme]: https://github.com/stayhydated/gpui-storybook/blob/master/crates/gpui-storybook-toml/README.md
[wayland-patches]: https://github.com/stayhydated/gpui-storybook/blob/wayland-render-image/Cargo.toml
