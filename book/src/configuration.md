# Configure Storybook

Place `storybook.toml` beside the `Cargo.toml` of a crate that owns stories.
The file assigns that crate an outer group. The configuration selected for the
running Storybook binary also controls cross-crate filtering and launch-only
presentation choices.

## Configure groups and filters

```toml
group = "UI Kit"
allow = ["UI Kit", "Shared"]
disable_story = ["ExperimentalCardStory"]
```

| Field | Required | Behavior |
|---|---:|---|
| `group` | Yes | Labels stories registered by this crate |
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

The active configuration's `allow`, `disable_story`, and `[overrides]` apply to
the running Storybook. When a linked story crate has no configured group, its
declared section is the group candidate used by `allow`.

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

Unknown fields, an invalid color scheme, an invalid theme identifier, or a
language outside the typed set make initialization fail with a
`StorybookInitError`. An unavailable but valid theme name falls back to the
registered theme for that scheme and produces a diagnostic.

## Create the Storybook window

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

The window hosts a searchable gallery, a story canvas, and a resizable
workbench. Use the title-bar controls and **Reset layout** to restore the left
story sidebar, centered story canvas, and right workbench. Existing
application-owned title-bar items remain beside the standard controls.

Registration, filtering, preferences, and MCP routes are shared across the
window. The workbench includes the Controls, Theme, Inspect, and Actions tabs.
The opt-in `performance` feature adds the Perf tab; the opt-in `inspector`
feature adds GPUI Inspector activation and story-root metadata.
