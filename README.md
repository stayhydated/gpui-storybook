# GPUI Storybook

[![CI][ci-badge]][ci]
[![Codecov][codecov-badge]][codecov]
[![Book][book-badge]][book]
[![crates.io: gpui-storybook][gpui-storybook-badge]][gpui-storybook-crate]

GPUI Storybook gives application developers a searchable gallery
for developing, inspecting, and exercising GPUI components outside
the rest of their application.

The workspace requires Rust 1.99 or newer and uses edition 2024.

## Overview

- Register stateful previews with `#[story]` or derive component previews with
  `ComponentStory`.
- Edit typed controls, inspect scoped actions, switch themes and viewports, and
  run repeatable scenarios from the workbench.
- Drive live Linux and macOS sessions through MCP, or exercise isolated stories
  with the portable headless test runner.
- Attach to an opted-in Android application, coordinate native navigation with
  embedded GPUI views, and capture the display or GPUI surface through ADB.

MCP servers consume an asynchronous automation backend. Shared contracts and
validation live in `gpui-storybook-automation`; core supplies the gallery backend.
Application-owned roots attach through `gpui-storybook-automation-gpui`.
The maintained Android example combines Jetpack Compose navigation and an
automatable native counter with embedded GPUI content,
while `gpui-storybook-mobile-host` serves its remote MCP connection through
bounded `adbutils-rs` device sockets, retained cleanup outcomes, and atomic PNG
publication. Device-owned session generations invalidate replaced surfaces.
AndroidX UI Automator qualifies native selectors, touch, IME input, lifecycle,
and screenshots on the declared API 36 emulator. Android arm64 has signed build
coverage; iOS has neutral contract compilation coverage.

Live captures size the story canvas to desktop, tablet, mobile, or explicit
pixel dimensions and crop gallery chrome from the PNG.

Linux Wayland window capture uses the GPUI Git patches pinned on the
[`wayland-render-image` branch](https://github.com/stayhydated/gpui-storybook/blob/wayland-render-image/Cargo.toml).
Applications using Storybook as a dependency must apply those patches in their
own workspace root.

## Crates

| Crate | Purpose | Source |
| --- | --- | --- |
| `gpui-storybook` | Application facade, initialization, registration, and public re-exports | [README](crates/gpui-storybook/README.md) |
| `gpui-storybook-automation` | Target-neutral contracts, validation, capabilities, and async host boundary | [README](crates/gpui-storybook-automation/README.md) |
| `gpui-storybook-automation-gpui` | Window-scoped instrumentation, frame execution, and native device coordination | [README](crates/gpui-storybook-automation-gpui/README.md) |
| `gpui-storybook-core` | Runtime shell and lower-level integration APIs | [README](crates/gpui-storybook-core/README.md) |
| `gpui-storybook-example-component` | Executable `ComponentStory` registration examples | [README](examples/component/README.md) |
| `gpui-storybook-example-embedded` | Application-owned Counter and Notes views | [README](examples/embedded/README.md) |
| `gpui-storybook-example-mobile` | Maintained Android native shell and device qualification | [README](examples/mobile/README.md) |
| `gpui-storybook-example-story` | Executable stateful `#[story]` registration examples | [README](examples/story/README.md) |
| `gpui-storybook-launch` | Linux headless Sway launcher for automation sessions | [README](crates/gpui-storybook-launch/README.md) |
| `gpui-storybook-mcp` | Typed live automation and story-region capture tools | [README](crates/gpui-storybook-mcp/README.md) |
| `gpui-storybook-mobile` | Opt-in device transport and exclusive operation admission | [README](crates/gpui-storybook-mobile/README.md) |
| `gpui-storybook-mobile-host` | Computer-owned direct ADB transport, remote MCP, and display capture | [README](crates/gpui-storybook-mobile-host/README.md) |
| `gpui-storybook-test` | Portable capture matrices, visual baselines, and frame budgets | [README](crates/gpui-storybook-test/README.md) |
| `gpui-storybook-toml` | Typed `storybook.toml` schema and loader | [README](crates/gpui-storybook-toml/README.md) |

[ci-badge]: https://github.com/stayhydated/gpui-storybook/actions/workflows/ci.yml/badge.svg?branch=master
[ci]: https://github.com/stayhydated/gpui-storybook/actions/workflows/ci.yml
[codecov-badge]: https://codecov.io/github/stayhydated/gpui-storybook/branch/master/graph/badge.svg
[codecov]: https://codecov.io/github/stayhydated/gpui-storybook
[book-badge]: https://img.shields.io/badge/Book-mdBook-blue
[book]: https://stayhydated.github.io/gpui-storybook/book/
[gpui-storybook-badge]: https://img.shields.io/crates/v/gpui-storybook.svg?label=gpui-storybook
[gpui-storybook-crate]: https://crates.io/crates/gpui-storybook
