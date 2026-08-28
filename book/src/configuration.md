# Configure Storybook

Place `storybook.toml` beside the `Cargo.toml` of a crate that owns stories.
The file assigns that crate an outer group. The configuration selected for the
running Storybook binary also controls cross-crate filtering and launch-only
window and presentation choices.

## Configure groups and filters

```toml
group = "UI Kit"
window_mode = "dock"
allow = ["UI Kit", "Shared"]
disable_story = ["ExperimentalCardStory"]
```

| Field | Required | Behavior |
|---|---:|---|
| `group` | Yes | Labels stories registered by this crate |
| `window_mode` | No | Chooses `"gallery"` or `"dock"` as the initial window layout |
| `allow` | No | Selects groups visible in the running Storybook |
| `disable_story` | No | Hides exact registered type names |

When `allow` is omitted, only the configuration's own normalized `group` is
included. Other forms are explicit:

```toml
# Include every linked group.
allow = ["*"]

# Include no groups.
allow = []
```

Entries are trimmed before group comparison. Use nonempty group names so a
crate has a usable navigation and filtering identity.

`disable_story` compares exact registered type names. Use `ButtonStory` for
an explicit story and `WelcomeCard` for a component story. Do not use the
package-qualified automation key or a localized display title.

## Understand runtime selection

Storybook loads configuration from every linked story crate to obtain its
`group`. It selects the active runtime configuration from the registered story
crate whose Cargo package name matches the running binary name.

The active configuration's `window_mode`, `allow`, `disable_story`, and
`[overrides]` apply to the running Storybook. When a linked story crate has no
configured group, its declared section is the group candidate used by `allow`.

Keep the Storybook binary name aligned with its package name when you want that
package's file to be selected automatically.

## Override launch presentation

Use `[overrides]` for deterministic presentation during a launch:

```toml
group = "UI Kit"

[overrides]
color_scheme = "dark"
theme = "Default Dark"
language = "en"
```

Every override field is optional.

- `color_scheme` accepts `"light"` or `"dark"`.
- `theme` names a registered theme for the effective color scheme.
- `language` is a BCP 47 tag present in the application's typed embedded
  language set.

Overrides change resolved presentation without rewriting saved user intent.
Values supplied with `StorybookOptions::with_overrides` win field by field
over TOML. MCP capture and stdio profiles win over both. See
[Preferences](preferences.md) for the full precedence model.

Unknown fields, an invalid window mode or color scheme, an invalid theme
identifier, or a language outside the typed set make initialization fail with a
`StorybookInitError`. An unavailable but valid theme name falls back to the
registered theme for that scheme and produces a diagnostic.

## Choose gallery or dock mode

Create one standard window from the generated stories:

```rust
gpui_storybook::create_storybook_window(
    "My App - Stories",
    |window, cx| {
        let stories = gpui_storybook::generate_stories(window, cx);
        gpui_storybook::StorybookWindow::new(stories)
    },
    cx,
);
```

The title-bar **Layout** select switches between **Gallery** and **Dock
workspace** without rebuilding the application. The selected
`StorybookWindowMode` is saved in the consumer preference document and becomes
the initial mode for later windows when neither code nor TOML supplies one.
Existing application-owned title-bar items remain beside the layout selector.

To configure the initial mode for the running binary, set a top-level key in
its active `storybook.toml`:

```toml
group = "UI Kit"
window_mode = "dock"
```

This changes the initial layout without locking the current window. The
title-bar selector remains available and persists later choices; while the TOML
key remains present, it continues to win over that saved preference for new
windows.

To choose a launch-specific initial mode, add `with_mode` to the returned window
specification:

```rust
gpui_storybook::create_storybook_window(
    "My App - Stories",
    |window, cx| {
        let stories = gpui_storybook::generate_stories(window, cx);
        gpui_storybook::StorybookWindow::new(stories)
            .with_mode(gpui_storybook::StorybookWindowMode::Dock)
    },
    cx,
);
```

Initial-mode precedence is:

1. `StorybookWindow::with_mode` on that window;
2. active `storybook.toml` `window_mode`;
3. the saved consumer preference.

Registration, filtering, preferences, and MCP routes are the same in both
window modes. Both modes include the Controls, Theme, Inspect, and Actions
workbench tabs. The opt-in `performance` feature adds the Perf tab; the opt-in
`inspector` feature adds GPUI Inspector activation and story-root metadata.
Gallery mode renders the workbench as a third resizable
region.

Dock mode installs it as an open, collapsible right dock. The saved layout
includes its width, visibility, and selected tab. Layouts created before the
workbench schema are replaced with the current three-region default. Choose
**Reset layout** in the title bar to restore the left story sidebar, center
story tabs, and right workbench.
