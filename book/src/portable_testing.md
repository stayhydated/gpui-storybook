# Portable testing and visual baselines

`gpui-storybook-test` runs registered stories in fresh
`gpui_kit::HeadlessAppContext` instances. Use it for story-isolated integration
tests, PNG capture, visual baselines, capture matrices, and optional GPUI frame
budgets without opening the gallery or dock shell.

Add it as a development dependency. Enable `performance` only when tests need
GPUI profiler samples and budgets:

```toml
[dev-dependencies]
gpui-storybook-test = { version = "0.5", features = ["performance"] }
```

Keep the crate that contains the story registrations linked from the test
binary. Inventory discovery can only see registrations that the linker retains.

## Capture one fresh story

The runner constructs the real registered story, applies typed controls and
presentation, settles the requested frames, and captures the story route:

```rust,no_run
use gpui_storybook_test::{CaptureRequest, HeadlessStoryRunner};

let runner = HeadlessStoryRunner::default();
let mut request = CaptureRequest::new("my-stories-ButtonStory");
request.output_path = Some("target/storybook/button.png".into());

let report = runner.capture(request)?;
assert_eq!(report.story.key, "my-stories-ButtonStory");
# Ok::<(), gpui_storybook_test::StorybookTestError>(())
```

Every request gets a new app, window, story entity, and set of GPUI globals.
On native targets, the fresh app installs `gpui_tokio` before the core runtime
and linked `#[story_init]` hooks, so those hooks can call
`gpui_tokio::Tokio::spawn` or `gpui_tokio::Tokio::handle`.
Use `runner.open(request)` when a test needs to update the live app, set a
control, advance the test clock, or inspect a runtime story snapshot before
capturing. A story without a typed control target reports an empty control
snapshot; applying a non-empty control map still fails with
`ControlsUnavailable`.

Pass the application's normal `AssetSource` through `RunnerConfig` when stories
load embedded fonts, icons, or images. Use the application initializer for
consumer-owned GPUI globals. The built-in `light`, `dark`, `Default Light`, and
`Default Dark` theme names apply GPUI Component's matching mode directly.
Other named themes and all named languages require a case configurator; the
runner fails the case instead of attaching an unapplied label to a capture.

## Run a capture matrix

`CaptureMatrix` expands the Cartesian product of stories, root or substory
routes, viewports, canvas backgrounds, themes, languages, and named typed-control
sets. Stable case IDs drive output paths, baseline paths, and structured
reports. Each matrix axis is encoded before the case ID is joined. Generated
request IDs use a bounded digest for serialized controls while reports retain
the complete typed values, and `output_dir` encodes each complete case ID as one
filename component so distinct values and labels remain distinct without
creating unbounded control filenames:

```rust,no_run
use gpui_storybook_test::{
    BaselinePolicy, BaselineStore, BaselineTolerance, CaptureMatrix,
    HeadlessStoryRunner, PresentationCase, ViewportCase,
};

let runner = HeadlessStoryRunner::default();
let matrix = CaptureMatrix::new()
    .story("my-stories-ButtonStory")
    .viewport(ViewportCase::mobile())
    .viewport(ViewportCase::desktop())
    .presentation(PresentationCase::light())
    .presentation(PresentationCase::dark())
    .output_dir("target/storybook/actual");
let baselines = BaselineStore::new("tests/storybook-baselines");

let report = runner.run_matrix(
    &matrix,
    Some(&baselines),
    BaselinePolicy::check(BaselineTolerance::exact()),
)?;
assert!(report.passed, "{report:#?}");
# Ok::<(), gpui_storybook_test::StorybookTestError>(())
```

Root and substory captures use Storybook's rendered capture-region registry, so
the saved image excludes any surrounding Storybook shell. A requested substory
must have rendered through `capture_substory` or `capture_substory_with_key`;
missing routes are explicit failures. For an application-owned route surface,
install `RunnerConfig::route_capture`; that callback owns route verification and
cropping without requiring a matching core capture-region entry.

## Keep baseline updates intentional

Baseline policy is always explicit:

- `BaselinePolicy::Ignore` captures without reading or writing baselines.
- `BaselinePolicy::Check` compares dimensions and pixel metrics using a typed
  tolerance. Missing and mismatched images remain report outcomes.
- `BaselinePolicy::Update` creates or replaces the accepted PNG. Use it only in
  an intentional baseline-update command, never as a CI fallback.

A matrix keeps running after a case failure and records typed failure data for
every case. Fail the enclosing test or command when `MatrixReport::passed` is
false.

## Enforce frame budgets

With the `performance` feature, add `PerformanceOptions` to a request or matrix.
The runner collects GPUI's native draw and dirty-to-present histograms inside
the fresh story window, then can enforce minimum sample counts and p95 or
maximum-duration budgets. These measurements are better suited to regression
checks than the cumulative gallery Perf tab because each case starts with an
isolated window and its own histograms.

Treat timing thresholds as platform-specific test policy. Pin the renderer,
fonts, assets, viewport, theme, language, controls, and CI hardware class before
comparing either pixels or frame timing.

## Platform expectations

The runner asks `gpui_platform` for the current headless renderer. The current
GPUI stack supplies Metal headless rendering on macOS and the Linux headless
renderer on Linux and FreeBSD; unsupported targets return a typed
renderer-unavailable error. Visual baselines are renderer- and font-sensitive,
so keep separate accepted images when CI spans platforms with materially
different output.
