# gpui-storybook-core

[![Codecov: gpui-storybook-core][codecov-badge]][codecov]
[![crates.io: gpui-storybook-core][crate-badge]][crate]

`gpui-storybook-core` provides the runtime shell and integration APIs for
developers building a custom GPUI Storybook host. Applications using the
standard initialization and discovery flow should use [`gpui-storybook`][facade].

## Overview

The crate owns gallery and dock layouts, story containers, controls and
workbench state, theme and preference UI, viewport presentation, localization,
and the shared automation controller. Its optional `capture`, `inspector`, and
`performance` features expose the corresponding lower-level runtime surfaces.

Scenario execution, semantic targets and values, navigation, control mutation,
and capture share the same frame-aware automation model used by the facade,
MCP integration, and portable test runner.

[codecov-badge]: https://codecov.io/github/stayhydated/gpui-storybook/branch/master/graph/badge.svg?component=gpui-storybook-core
[codecov]: https://codecov.io/github/stayhydated/gpui-storybook
[crate-badge]: https://img.shields.io/crates/v/gpui-storybook-core.svg?label=gpui-storybook-core
[crate]: https://crates.io/crates/gpui-storybook-core
[facade]: https://crates.io/crates/gpui-storybook
