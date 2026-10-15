# gpui-storybook-test

[![crates.io: gpui-storybook-test][crate-badge]][crate]

`gpui-storybook-test` provides portable headless execution for GPUI Storybook
registrations, aimed at integration tests and CI jobs that need isolated
captures, visual baselines, capture matrices, or frame budgets.

Requires Rust 1.99 or newer and GPUI Kit 0.7. Headless image capture uses a
working Wgpu adapter on Linux or the Metal renderer on macOS; the pinned GPUI
stack provides no Windows headless renderer.

## Overview

Each request creates a fresh `HeadlessAppContext`, initializes the core runtime
and linked `#[story_init]` hooks, constructs one registered story, and applies
typed controls and presentation before capture. Applications can provide assets,
initialization, theme and language adapters, and custom substory crop policy
through `RunnerConfig`.

`BaselinePolicy::Check` verifies accepted output, while
`BaselinePolicy::Update` makes baseline acceptance explicit. The optional
`performance` feature records GPUI profiler samples for draw and
dirty-to-present budgets.

Capture output depends on the current platform renderer, fonts, assets, and CI
hardware; keep accepted baselines scoped when those inputs differ. The
[runner API reference][api] describes baseline comparison and execution semantics.

## Features

The default feature set enables `capture`.

| Feature | Default | Purpose |
| --- | --- | --- |
| `capture` | Yes | Enables the core image-capture backend used by portable captures. |
| `performance` | No | Enables `gpui-pre/profiler` for frame samples and performance budgets. |

## Example

Plan mobile and desktop cases for every linked story without rendering a frame.
The test binary must retain the application library's inventory registrations;
an empty registry produces zero cases.

```rust
use gpui_storybook_test::{
    CaptureMatrix, StorybookTestError, ViewportCase, discover_stories_checked,
};

fn main() -> Result<(), StorybookTestError> {
    let stories = discover_stories_checked()?;
    let matrix = CaptureMatrix::new()
        .viewport(ViewportCase::mobile())
        .viewport(ViewportCase::desktop());
    let cases = matrix.expand(&stories)?;
    assert_eq!(cases.len(), stories.len() * 2);
    Ok(())
}
```

`HeadlessStoryRunner::run_matrix` executes those cases with a fresh application
context per case. The [maintained capture test][example] supplies Storybook assets
and a registered story to produce and verify a PNG.

[crate-badge]: https://img.shields.io/crates/v/gpui-storybook-test.svg?label=gpui-storybook-test
[crate]: https://crates.io/crates/gpui-storybook-test
[api]: https://docs.rs/gpui-storybook-test
[example]: https://github.com/stayhydated/gpui-storybook/blob/master/examples/story/tests/portable_capture.rs
