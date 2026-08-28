---
name: use-gpui-storybook
description: >-
  Integrate or troubleshoot GPUI Storybook in application repositories. Use
  when Codex needs to add a Storybook binary; register previews with #[story],
  #[derive(ComponentStory)], #[derive(Substory)], or #[story_init]; configure
  storybook.toml groups, filters, or launch overrides; choose gallery or dock
  mode; add typed story controls or use the controls/theme/action workbench,
  opt-in performance telemetry, and opt-in Inspector integration;
  wire the typed es-fluent locale adapter and preference startup; enable MCP
  tools, opt-in in-process interaction, or PNG capture; or diagnose missing
  stories, unstable routes, preference readiness, control failures,
  scenario failures, interaction failures, visual baseline failures, or capture
  failures.
---

# Integrate GPUI Storybook

## Workflow

1. Inspect the application package manifest, Storybook entry point, locale
   module, `storybook.toml`, section types, and existing component patterns.
2. Use the `gpui-storybook` facade. Reach for `-core`, `-macros`, `-toml`,
   or `-mcp` directly only when the application intentionally owns a lower-level
   integration.
3. Keep the story-bearing library linked from the binary so inventory
   registrations are retained.
4. Build native apps with `gpui_platform::application()` and the facade's
   embedded assets.
5. Create a stable, binary-specific `ConsumerId`, call
   `gpui_storybook::init`, await readiness, and only then open the gallery or
   dock window.
6. Update the manifest, entry point, locale assets, story modules, and
   `storybook.toml` together when the requested workflow crosses those
   surfaces.
7. Preserve the application's error handling, localization, section naming,
   feature-forwarding, and GPUI component conventions.

## Load only the needed reference

- Read [setup and configuration](references/setup-and-configuration.md) when
  adding the binary, locale adapter, preferences, gallery/dock mode, or
  `storybook.toml`.
- Read [story authoring](references/story-authoring.md) when adding or changing
  registrations, metadata, controls, sections, substories, or one-time setup.
- Read [automation and capture](references/automation-and-capture.md) when
  enabling MCP, selecting routes, launching captures, adding portable headless
  tests or visual baselines, or troubleshooting automation.

## Non-negotiable contracts

- Await preference readiness before the first window.
- Treat display labels and stable route keys as separate values.
- Keep control metadata and values on the typed story entity; use
  `StoryControls` and `#[storybook(control...)]` rather than a second value model.
- Keep reusable interaction in `Story::scenarios()` or a component derive's
  `scenarios = ...` expression. Scenario runs recreate the story and use the
  shared executor; do not resume or retry partial runs.
- Match `disable_story` against the registered type name, not the display title
  or package-qualified key.
- Forward optional package features explicitly when users launch with
  `--features dock`, `--features inspector`, `--features performance`, or
  `--features mcp`.
- Use the Actions workbench tab for GPUI actions exposed through the selected
  story's opt-in `Story::action_scope_focus_handle`. Track that handle on the
  handler-owning root and keep it separate from nested input focus. The tab
  excludes child-control and Storybook shell/root actions and dispatches back
  to the explicit scope. Enable `performance` only when the Storybook binary should
  compile GPUI's profiler and expose window histograms plus the debug frame
  overlay.
- Send logs to standard error during MCP stdio sessions.
- Retain the MCP automation handle for the host lifetime. Tool completion
  does not request shutdown; stdin, cancellation, or application policy ends
  the host explicitly.
- Run Linux MCP and startup-capture sessions through
  `gpui-storybook-launch`; keep the application on its normal Wayland platform
  backend and use `GPUI_STORYBOOK_SWAY` for a private Sway executable.
- Require `GPUI_STORYBOOK_MCP_ALLOW_INTERACTION=1` before generic focus,
  keyboard, action, semantic-target, pointer, scroll, or frame-wait automation.
  Keep typed controls as the preferred reproducible input, then wrap important
  visible controls with `StorybookElementExt::storybook_target()` so stable
  route-local GPUI IDs become semantic keys before falling back to coordinates.
- Wrap machine-readable rendered state with
  `StorybookElementExt::storybook_value(&state)` and use
  `storybook_read_value` or bounded `storybook_wait_for_value` calls for
  postconditions. Use the `_as` variants
  when keys or labels must be explicit. Reserve capture for assertions about
  visual presentation.
- Use `story_key`, `target_key`, `value_key`, and `control_key` in MCP inputs.
  Let the initial tool call await the standard host instead of polling an empty
  story catalog.
- Treat interaction batches as potentially destructive and non-idempotent.
  Rediscover runtime actions after each launch and never retry partial batches
  automatically.
- Treat paired dimensions and named viewports as story-region capture sizes;
  keep the gallery or dock chrome mounted around that region.
- Use `gpui-storybook-test` for isolated test and CI cases. Keep story
  registrations linked, pass the application's assets and initialization, make
  baseline check versus update policy explicit, and require a case configurator
  for non-built-in named themes or named language axes.
- Keep fixed viewport frames centered and locked. Responsive mode alone is
  resizable and inherits the immediately previous fixed preset's dimensions.
- Keep viewport state scoped to each Storybook window.
- Make named theme selection activate the theme's matching light or dark
  appearance while preserving the opposite saved theme slot. Selecting `System`
  appearance resumes device-driven transitions between those slots.
- Keep the canvas centered within the visible main pane as the story-navigation
  and workbench sidebars change; expose left and right panel icons immediately
  before the top-bar appearance settings button for toggling those panels. Keep
  a symmetric gutter around Responsive frames so resize handles remain reachable
  when the frame exceeds the visible pane.
- Prefer current public APIs and examples; do not add compatibility wrappers for
  older pre-1.0 shapes unless the user requires them.
