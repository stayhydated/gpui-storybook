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
tooling without constructing stories or opening a window.

[codecov-badge]: https://codecov.io/github/stayhydated/gpui-storybook/branch/master/graph/badge.svg?component=gpui-storybook
[codecov]: https://codecov.io/github/stayhydated/gpui-storybook
[crate-badge]: https://img.shields.io/crates/v/gpui-storybook.svg?label=gpui-storybook
[crate]: https://crates.io/crates/gpui-storybook
