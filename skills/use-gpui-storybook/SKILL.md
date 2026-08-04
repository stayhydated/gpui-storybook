---
name: use-gpui-storybook
description: >-
  Integrate or troubleshoot GPUI Storybook in application repositories. Use
  when Codex needs to add a Storybook binary; register previews with #[story],
  #[derive(ComponentStory)], #[derive(Substory)], or #[story_init]; configure
  storybook.toml groups, filters, or launch overrides; choose gallery or dock
  mode; wire the typed es-fluent locale adapter and preference startup; enable
  MCP tools or PNG capture; or diagnose missing stories, unstable routes,
  preference readiness, or capture failures.
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
4. Create a stable, binary-specific `ConsumerId`, call
   `gpui_storybook::init`, await readiness, and only then open the gallery or
   dock window.
5. Update the manifest, entry point, locale assets, story modules, and
   `storybook.toml` together when the requested workflow crosses those
   surfaces.
6. Preserve the application's error handling, localization, section naming,
   feature-forwarding, and GPUI component conventions.

## Load only the needed reference

- Read [setup and configuration](references/setup-and-configuration.md) when
  adding the binary, locale adapter, preferences, gallery/dock mode, or
  `storybook.toml`.
- Read [story authoring](references/story-authoring.md) when adding or changing
  registrations, metadata, sections, substories, or one-time setup.
- Read [automation and capture](references/automation-and-capture.md) when
  enabling MCP, selecting routes, launching captures, or troubleshooting
  automation.

## Non-negotiable contracts

- Await preference readiness before the first window.
- Treat display labels and stable route keys as separate values.
- Match `disable_story` against the registered type name, not the display title
  or package-qualified key.
- Forward optional package features explicitly when users launch with
  `--features dock` or `--features mcp`.
- Send logs to standard error during MCP stdio sessions.
- Prefer current public APIs and examples; do not add compatibility wrappers for
  older pre-1.0 shapes unless the user requires them.
