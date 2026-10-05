# gpui-storybook

[![Codecov: gpui-storybook][codecov-badge]][codecov]
[![crates.io: gpui-storybook][crate-badge]][crate]

`gpui-storybook` is the application-facing facade for building a searchable
GPUI component gallery with typed controls, scoped actions,
themes, viewports, and repeatable scenarios.

## Overview

- Register stateful previews with `#[story]`, derive component previews with
  `ComponentStory`, and organize stable substory routes with `Substory`.
- Initialize preferences and localization once, await readiness, and construct
  a `StorybookWindow`.
- Enable `mcp` for Linux/macOS live automation and capture, `inspector` for GPUI
  Inspector integration, or `performance` for frame telemetry.

The facade also exposes static registration catalogs for documentation and
tooling without constructing stories or opening a window. Localized consumer
metadata uses `try_localize_message(cx, &message)` and handles `Option<String>`
through the application's locale context.

The facade exposes `AutomationBackend`, `SharedAutomationBackend`, and portable
capability types. Standard initialization installs the gallery backend for MCP.
Embedded application roots use `gpui-storybook-automation-gpui`; maintained Android
integration uses its reusable device coordinator, fresh session generations,
direct ADB transport, atomic PNG publication, and
AndroidX native qualification harness.
The Android example exposes Jetpack Compose controls alongside embedded GPUI
through advertised native actions, semantic values, and accessibility test tags.

Live captures size the story canvas to a named viewport or explicit pixel
dimensions and crop gallery chrome from the PNG.

Linux Wayland window capture requires the GPUI Git patches from the
[`wayland-render-image` branch](https://github.com/stayhydated/gpui-storybook/blob/wayland-render-image/Cargo.toml)
in the application's workspace root.

[codecov-badge]: https://codecov.io/github/stayhydated/gpui-storybook/branch/master/graph/badge.svg?component=gpui-storybook
[codecov]: https://codecov.io/github/stayhydated/gpui-storybook
[crate-badge]: https://img.shields.io/crates/v/gpui-storybook.svg?label=gpui-storybook
[crate]: https://crates.io/crates/gpui-storybook
