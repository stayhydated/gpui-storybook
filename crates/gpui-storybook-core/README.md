# gpui-storybook-core

[![Codecov: gpui-storybook-core][codecov-badge]][codecov]
[![crates.io: gpui-storybook-core][crate-badge]][crate]

`gpui-storybook-core` provides the runtime shell and integration APIs for
developers building a custom GPUI Storybook host. Applications using the
standard initialization and discovery flow should use [`gpui-storybook`][facade].

## Overview

The crate owns the gallery layout, story containers, controls and
workbench state, theme and preference UI, viewport presentation, localization,
and the gallery automation controller implementing `AutomationBackend`. Its optional `capture`, `inspector`, and
`performance` features expose the corresponding lower-level runtime surfaces.

Scenario execution, semantic targets and values, navigation, control mutation,
and capture share the same frame-aware automation model used by the facade,
MCP integration, and portable test runner.

Portable control and interaction records and validation come from
`gpui-storybook-automation`. Core owns GPUI entity adapters, story construction,
and desktop image rendering. Reusable instrumentation and frame execution live
in `gpui-storybook-automation-gpui`, with app/window ownership for rendered data.

Live capture waits for the requested canvas dimensions to fit the visible
story pane, then crops gallery chrome from the PNG.

[codecov-badge]: https://codecov.io/github/stayhydated/gpui-storybook/branch/master/graph/badge.svg?component=gpui-storybook-core
[codecov]: https://codecov.io/github/stayhydated/gpui-storybook
[crate-badge]: https://img.shields.io/crates/v/gpui-storybook-core.svg?label=gpui-storybook-core
[crate]: https://crates.io/crates/gpui-storybook-core
[facade]: https://crates.io/crates/gpui-storybook
