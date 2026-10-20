# gpui-storybook

[![Codecov: gpui-storybook][codecov-badge]][codecov]
[![crates.io: gpui-storybook][crate-badge]][crate]

`gpui-storybook` is the application-facing facade for building a searchable
GPUI component gallery with typed controls, scoped actions,
themes, viewports, and repeatable scenarios.

Requires Rust 1.99 or newer and GPUI Kit 0.8.0. The optional `mcp` feature
supports Linux and macOS; other targets reject that feature.

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

Live captures size the story canvas to a named viewport or explicit pixel
dimensions and crop gallery chrome from the PNG.

Linux Wayland window capture requires the GPUI Git patches from the
[`wayland-render-image` branch][wayland-patches]
in the application's workspace root.

## Features

The default feature set enables `macros`.

| Feature | Default | Purpose |
| --- | --- | --- |
| `inspector` | No | Forwards the core Inspector feature and re-exports `StoryInspectorState`. |
| `macros` | Yes | Re-exports registration, control, substory, and initialization macros. |
| `mcp` | No | Enables live MCP serving and re-exports automation/capture helpers on Linux and macOS. |
| `performance` | No | Forwards core GPUI profiler histograms and the Perf-tab debug frame overlay. |

## Example

A tooling binary can export the static registrations without opening a GPUI
window. Keep its story-bearing application library linked so the inventory
contains those stories; an unregistered binary exports an empty `stories` array.

```rust
use gpui_storybook::{StoryCatalogExportError, static_story_catalog_json_pretty};

fn main() -> Result<(), StoryCatalogExportError> {
    let catalog = static_story_catalog_json_pretty()?;
    println!("{catalog}");
    Ok(())
}
```

The JSON contains stable story keys, source locations, Rustdocs, and control
shape metadata. Runtime titles and control defaults are resolved by live stories.
See the [catalog API][catalog-api] and the [maintained story library][example].

[codecov-badge]: https://codecov.io/gh/stayhydated/gpui-storybook/branch/master/graph/badge.svg?component=gpui-storybook
[codecov]: https://codecov.io/gh/stayhydated/gpui-storybook
[crate-badge]: https://img.shields.io/crates/v/gpui-storybook.svg?label=gpui-storybook
[crate]: https://crates.io/crates/gpui-storybook
[wayland-patches]: https://github.com/stayhydated/gpui-storybook/blob/wayland-render-image/Cargo.toml
[catalog-api]: https://docs.rs/gpui-storybook/latest/gpui_storybook/fn.static_story_catalog_json_pretty.html
[example]: https://github.com/stayhydated/gpui-storybook/blob/master/examples/story/src/lib.rs
