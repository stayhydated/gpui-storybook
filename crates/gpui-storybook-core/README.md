# gpui-storybook-core

[![Codecov: gpui-storybook-core][codecov-badge]][codecov]
[![crates.io: gpui-storybook-core][crate-badge]][crate]

`gpui-storybook-core` provides the runtime shell and integration APIs for
developers building a custom GPUI Storybook host. Applications using the
standard initialization and discovery flow should use [`gpui-storybook`][facade].

Requires Rust 1.99 or newer and GPUI Kit 0.7.

## Overview

The crate owns the gallery layout, story containers, controls and
workbench state, theme and preference UI, viewport presentation, localization,
and the shared automation controller.

See the [runtime API reference][api] for frame-aware automation, scenario
execution, and capture semantics shared by the facade, MCP integration, and test
runner.

## Features

No features are enabled by default.

| Feature | Default | Purpose |
| --- | --- | --- |
| `capture` | No | Enables platform image capture, PNG handling, and GPUI platform test support. |
| `inspector` | No | Enables GPUI Kit Inspector integration and story-root inspector metadata. |
| `performance` | No | Enables `gpui-pre/profiler` for frame telemetry and the Perf workbench. |

## Example

Custom catalog tooling can inspect linked story registrations without creating
views or initializing a display:

```rust
use gpui_storybook_core::catalog::static_story_catalog;

fn main() {
    let catalog = static_story_catalog();
    for story in catalog.entries() {
        println!("{}: {}", story.key, story.source.file);
    }
}
```

This prints a stable key and source file for each registered story. The host must
link its story-bearing library; an empty registry prints nothing.

[codecov-badge]: https://codecov.io/gh/stayhydated/gpui-storybook/branch/master/graph/badge.svg?component=gpui-storybook-core
[codecov]: https://codecov.io/gh/stayhydated/gpui-storybook
[crate-badge]: https://img.shields.io/crates/v/gpui-storybook-core.svg?label=gpui-storybook-core
[crate]: https://crates.io/crates/gpui-storybook-core
[facade]: https://crates.io/crates/gpui-storybook
[api]: https://docs.rs/gpui-storybook-core
