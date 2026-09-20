---
name: use-gpui-storybook
description: >-
  Integrate or troubleshoot GPUI Storybook in application repositories. Use for
  story registration, typed controls and scenarios, storybook.toml, preference
  and locale startup, MCP automation, PNG capture, and portable visual tests.
---

# Integrate GPUI Storybook

## Start from the application

Inspect the package manifest, Storybook entry point, locale module,
`storybook.toml`, and existing stories. Use the `gpui-storybook` facade unless
the application owns a lower-level runtime or tooling integration. Preserve its
error handling, localization, section naming, and feature forwarding.

Keep the story-bearing library linked from the binary so inventory
registrations are retained. Build native applications with
`gpui_kit::application()` and the facade's embedded assets. Create a stable,
binary-specific `ConsumerId`, call `gpui_storybook::init`, await readiness, and
then open the window.

## Choose the relevant reference

- [Setup and configuration](references/setup-and-configuration.md): binary
  startup, locale adapter, preferences, gallery/dock mode, optional workbench
  features, and `storybook.toml`.
- [Story authoring](references/story-authoring.md): registration style,
  metadata, typed controls, action scopes, scenarios, sections, and substories.
- [Automation and capture](references/automation-and-capture.md): MCP tools,
  platform launch commands, semantic interaction, captures, portable tests,
  visual baselines, and automation failures.

## Preserve the integration contracts

- Await preference readiness before constructing the first window.
- Keep display labels separate from stable route keys. `disable_story` matches
  the registered type name.
- Keep control metadata and values on the typed story entity. Only marked
  fields become controls; component defaults come from the configured example.
- Use `StorybookWindow::new` with `create_storybook_window`. Initial layout
  precedence is per-window `with_mode`, active TOML `window_mode`, then the
  saved preference. The title-bar selector can change and save the later choice.
- Keep preview and selection state scoped to the Storybook window. Duplicate
  titles within one group and section are concrete variants behind one
  navigation entry; dock mode opens selected members in independent tabs.
- Expose Actions-tab commands through `Story::action_scope_focus_handle` on the
  handler-owning root. Keep that handle separate from nested input focus.
- Keep reusable flows in `Story::scenarios()` or the component derive's
  `scenarios` expression. Every run recreates the story; never resume or
  automatically retry a partial input sequence.
- Forward `inspector`, `performance`, and `mcp` only when needed by the
  requested integration. MCP supports Linux and macOS; Linux sessions use
  `gpui-storybook-launch`, and macOS sessions launch directly through Cargo.
- Send MCP logs to standard error and retain its automation handle for the host
  lifetime. Generic remote input requires
  `GPUI_STORYBOOK_MCP_ALLOW_INTERACTION=1` and can invoke application effects.
- Use typed controls and semantic targets for reproducible input. Expose
  rendered state through `storybook_value` for bounded postcondition checks;
  use PNG captures for visual assertions.
- Use `gpui-storybook-test` for isolated cases. Keep baseline checking and
  acceptance explicit, and supply assets, initialization, and theme/language
  adapters required by the application.

After edits, build the affected Storybook package with the requested features.
Exercise the changed route, control, scenario, or capture when the target
platform is available. Report checks that could not run and their limits.
