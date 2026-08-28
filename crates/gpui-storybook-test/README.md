# gpui-storybook-test

`gpui-storybook-test` is the headless developer-tooling crate for GPUI
Storybook. It discovers `inventory` story registrations, creates a fresh
`gpui::HeadlessAppContext` for each portable story, applies typed controls,
renders PNGs with the current platform headless renderer, and runs visual
baseline or capture-matrix jobs.

The crate is intended for debug tools, CI runners, and integration tests. A
fresh context is created for every capture, so story entities and story-local
globals are isolated from adjacent cases. The `capture` feature is enabled by
default. Enable `performance` to collect GPUI's window profiler histograms and
enforce draw and dirty-to-present budgets:

```toml
[dev-dependencies]
gpui-storybook-test = { version = "0.5", features = ["performance"] }
```

The smallest runner looks up a registered story and saves one PNG:

```rust,no_run
use gpui_storybook_test::{CaptureRequest, HeadlessStoryRunner};

let runner = HeadlessStoryRunner::default();
let mut request = CaptureRequest::new("my-stories-ButtonStory");
request.output_path = Some("target/storybook/button.png".into());
let report = runner.capture(request)?;
println!("captured {}", report.output_path.unwrap().display());
# Ok::<(), gpui_storybook_test::StorybookTestError>(())
```

`BaselineStore` keeps comparison and update operations explicit: use
`BaselinePolicy::Check` in verification jobs and `BaselinePolicy::Update` only
when intentionally accepting new output. `CaptureMatrix` expands the selected
stories, routes, named viewports, presentation cases, themes, languages, and
named control sets into stable case IDs. Every expanded case is executed in a
fresh context and `MatrixReport` records each success or typed failure as
structured JSON-compatible data. Root and substory routes use the core
capture-region crop and scroll helpers; `RunnerConfig::route_capture` can
override that crop policy when an application needs a custom route surface.

Built-in `light`, `dark`, `Default Light`, and `Default Dark` theme names use
GPUI Component's `Theme::change` automatically. Other theme and language
adapters are application-owned: install a `RunnerConfig::case_configurator`
callback to apply them to the fresh `App` before its first draw. A request that
names a theme or language requiring an adapter without that callback fails as
`CaseConfigurationRequired`, so a matrix cannot pass with only a changed label.
The callback also receives the live story entity for custom presentation setup.
Use `RunnerConfig::asset_source` when stories load embedded fonts, icons, or
images.

The runner uses GPUI's current-platform headless renderer: Metal on macOS and
the Linux headless renderer on Linux and FreeBSD. Other targets fail with the
typed renderer-unavailable error. Keep renderer- and font-specific baselines
when CI spans platforms whose raster output differs.
