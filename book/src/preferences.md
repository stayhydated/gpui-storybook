# Preferences

Storybook stores layout, appearance, and language intent per Storybook binary.
Give each binary a distinct stable `ConsumerId`, await initialization readiness,
and let the layout, appearance, theme, and language controls update the same
typed state used by application code.

## Scope preference storage

Pass the consumer ID, fallback language, and locale adapter to
`StorybookOptions::new`:

```rust
let options = gpui_storybook::StorybookOptions::new(
    gpui_storybook::ConsumerId::new("my-app-storybook")?,
    Languages::default(),
    i18n::apply_locale,
);
```

The default persistent document is:

```text
.gpui-storybook/{consumer-id}.json
```

Storybook places it at the Cargo workspace root, or at the package root for a
standalone crate. It also writes a shared `preferences.schema.json` and a
`.gitignore` that excludes the local preference directory.

## Choose a persistence mode

| Mode | Use it for | Storage behavior |
|---|---|---|
| `Persistent` | Normal interactive use | Reuses the consumer-scoped JSON document |
| `Temporary` | Isolated automation or disposable sessions | Uses unique temporary JSON and schema files |
| `Disabled` | Capture or environments without persistence | Keeps preferences in memory |

Select a mode before initialization:

```rust
let options = options.with_persistence(gpui_storybook::PersistenceMode::Disabled);
```

An explicit `with_json_path(...)` is valid only with `Persistent` mode.

## Distinguish intent from presentation

`PreferenceState::saved` preserves what the user chose:

- Gallery or Dock workspace layout;
- `System` or an explicit light/dark appearance;
- `System` or an explicit language;
- separate theme choices for light and dark appearance;
- scrollbar behavior.

`PreferenceState::resolved` reports the effective color scheme, theme, and
language after system detection, registered-theme availability, and launch
overrides. Each resolved value includes its source, and fallback decisions add
structured diagnostics.

The title-bar **Layout** select writes `StorybookWindowMode::Gallery` or
`StorybookWindowMode::Dock`. New windows use a launch-specific
`StorybookWindow::with_mode` value first, then the active `storybook.toml`
`window_mode`, then the saved value. TOML selects the initial layout without
rewriting the saved document; the title-bar select persists later user choices.
While the TOML key remains present, it continues to win over that saved value
for each new window.

Selecting a named light or dark theme saves that theme in its matching slot and
activates the same appearance, so the selected theme is visible immediately.
Storybook keeps the theme in the opposite slot for later appearance changes.
Select `System` from **Appearance** to resume following device appearance; each
system transition uses the saved theme for that light or dark slot. A
launch-only `color_scheme` override remains higher priority than menu changes.

Color edits in the workbench's **Theme** tab are session overrides layered over
the selected base theme; they do not rewrite saved preference intent. Selecting
a different base theme clears the draft. Reloading the same named theme from a
native watched directory rebases and reapplies the session overrides.

## Follow system changes

With `System` appearance, Storybook follows live window appearance changes.
With `System` language, it negotiates the ordered device locales during
startup and checks again when a Storybook window becomes active.

Explicit appearance or language choices ignore later system changes until the
user selects `System` again.

## Apply launch-only overrides

Programmatic and TOML overrides affect only resolved values for the current
launch:

```rust
let options = options.with_overrides(gpui_storybook::PreferenceOverrides {
    color_scheme: Some(gpui_storybook::SystemColorScheme::Dark),
    ..Default::default()
});
```

Precedence is:

1. MCP capture or stdio profile;
2. `StorybookOptions::with_overrides`;
3. active `storybook.toml` `[overrides]`;
4. saved intent and system detection;
5. registered fallback values.

No override rewrites the saved document.

## Read preference state

Use `gpui_storybook::try_preference_state(cx)` for a read-only snapshot. It
contains saved and resolved preferences, persistence status, and diagnostics.

`PersistenceStatus` describes storage only. Locale adapter failures appear in
diagnostics without changing a successful storage status.

## Recover from failures

Invalid static configuration makes `init` return `StorybookInitError`.
Repository open or load failures instead complete readiness with fallbacks,
`PersistenceStatus::Error`, and diagnostics so the window can still open.

A failed save leaves the optimistic session value active. Open windows expose a
**Retry Save** notification action. Locale adapter failures are retried when a
Storybook window becomes active.

Inspect `StorybookReady::diagnostics` immediately after readiness and
`PreferenceState::diagnostics` when diagnosing later changes.
