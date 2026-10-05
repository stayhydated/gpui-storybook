# gpui-storybook-test

[![crates.io: gpui-storybook-test][crate-badge]][crate]

`gpui-storybook-test` provides portable headless execution for GPUI Storybook
registrations, aimed at integration tests and CI jobs that need isolated
captures, visual baselines, capture matrices, or frame budgets.

## Overview

Each request creates a fresh `HeadlessAppContext`, initializes the core runtime
and linked `#[story_init]` hooks, constructs one registered story, and applies
typed controls and presentation before capture. Applications can provide assets,
initialization, theme and language adapters, and custom substory crop policy
through `RunnerConfig`.

Rendered regions and semantic registrations belong to each app/window context.
Fresh contexts and frames isolate identical route keys across matrix cases.

`BaselinePolicy::Check` verifies accepted output, while
`BaselinePolicy::Update` makes baseline acceptance explicit. The optional
`performance` feature records GPUI profiler samples for draw and
dirty-to-present budgets.

Baseline comparisons use raw RGBA channels. Channel tolerance affects the
differing-pixel count; mean absolute error still includes every channel delta.

Capture output depends on the current platform renderer, fonts, assets, and CI
hardware; keep accepted baselines scoped when those inputs differ.

[crate-badge]: https://img.shields.io/crates/v/gpui-storybook-test.svg?label=gpui-storybook-test
[crate]: https://crates.io/crates/gpui-storybook-test
