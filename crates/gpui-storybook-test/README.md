# gpui-storybook-test

[![CI](https://github.com/stayhydated/gpui-storybook/actions/workflows/ci.yml/badge.svg)](https://github.com/stayhydated/gpui-storybook/actions/workflows/ci.yml)
[![Codecov](https://codecov.io/github/stayhydated/gpui-storybook/graph/badge.svg)](https://codecov.io/github/stayhydated/gpui-storybook)
[![Book](https://img.shields.io/badge/book-online-blue)](https://stayhydated.github.io/gpui-storybook/book/)
[![crates.io](https://img.shields.io/crates/v/gpui-storybook-test.svg)](https://crates.io/crates/gpui-storybook-test)

`gpui-storybook-test` is the headless developer-tooling crate for GPUI
Storybook. It discovers `inventory` story registrations, creates a fresh
`gpui_kit::HeadlessAppContext` for each portable story, applies typed controls,
renders PNGs with the current platform headless renderer, and runs visual
baseline or capture-matrix jobs.

The crate is intended for debug tools, CI runners, and integration tests. A
fresh context is created for every capture, so story entities and story-local
globals are isolated from adjacent cases. On native targets, the runner installs
its Tokio bridge before initializing the core runtime and invoking linked
`#[story_init]` hooks. Hooks can use
`gpui_storybook_test::tokio_bridge::Tokio::handle(cx)` to spawn work on that
case's runtime. The `capture` feature is enabled by default. Enable
`performance` to collect GPUI window profiler histograms and enforce draw and
dirty-to-present budgets.

## Capture a story

The smallest runner looks up a registered story and saves one PNG:

```rust
use gpui_storybook_test::{CaptureRequest, HeadlessStoryRunner, StorybookTestError};

fn main() -> Result<(), StorybookTestError> {
    let runner = HeadlessStoryRunner::default();
    let mut request = CaptureRequest::new("my-stories-ButtonStory");
    request.output_path = Some("target/storybook/button.png".into());
    let report = runner.capture(request)?;
    println!("captured {}", report.output_path.unwrap().display());
    Ok(())
}
```

## Matrices and baselines

`BaselineStore` keeps comparison and update operations explicit: use
`BaselinePolicy::Check` in verification jobs and `BaselinePolicy::Update` only
when intentionally accepting new output. `CaptureMatrix` expands the selected
stories, routes, named viewports, presentation cases, themes, languages, and
named control sets into stable case IDs. Every expanded case is executed in a
fresh context and `MatrixReport` records each success or typed failure as
structured JSON-compatible data. Each matrix axis is encoded before the case ID
is joined, and generated request IDs represent serialized control maps with a
bounded digest while retaining the complete controls in reports. `output_dir`
encodes the complete case ID into one filename component, so distinct control
values and case labels cannot overwrite one another or create unbounded control
filenames. Root and substory routes use the core capture-region crop and scroll
helpers; `RunnerConfig::route_capture` can override that policy and own route
verification when an application needs a custom route surface. Stories without
a typed control target produce an empty control snapshot. Supplying a non-empty
control map to such a story remains a typed `ControlsUnavailable` failure.

## Application configuration

Built-in `light`, `dark`, `Default Light`, and `Default Dark` theme names use
GPUI Component's `Theme::change` automatically. Other theme and language
adapters are application-owned: install a `RunnerConfig::case_configurator`
callback to apply them to the fresh `App` before its first draw. A request that
names a theme or language requiring an adapter without that callback fails as
`CaseConfigurationRequired`, so a matrix cannot pass with only a changed label.
The callback also receives the live story entity for custom presentation setup.
Use `RunnerConfig::asset_source` when stories load embedded fonts, icons, or
images.

## Platform support

The runner uses GPUI's current-platform headless renderer. The published
`gpui-pre` 0.3.5 stack supports Metal capture on macOS. Use the repository's
`linux-headless-renderer` branch with the `stayhydated/zed` fork for Linux and FreeBSD
headless capture. Keep renderer- and font-specific baselines when CI spans
platforms whose raster output differs.
