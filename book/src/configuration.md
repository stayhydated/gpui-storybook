# Configure Storybook

Place `storybook.toml` beside the `Cargo.toml` of a crate that owns stories.
The file assigns that crate an outer group. The configuration selected for the
running Storybook binary also controls cross-crate filtering and launch-only
preference overrides.

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

The active configuration's `allow`, `disable_story`, and `[overrides]`
apply to the generated story set. When a linked story crate has no configured
group, its declared section is the group candidate used by `allow`.

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
language outside the typed set makes initialization fail with a
`StorybookInitError`. An unavailable but valid theme name falls back to the
registered theme for that scheme and produces a diagnostic.

## Choose gallery or dock mode

The default gallery uses `create_new_window` and `Gallery::view`:

```rust
gpui_storybook::create_new_window(
    "My App - Stories",
    |window, cx| {
        let stories = gpui_storybook::generate_stories(window, cx);
        gpui_storybook::Gallery::view(stories, None, window, cx)
    },
    cx,
);
```

For a dock workspace, enable and forward the feature from the Storybook package:

```toml
[features]
dock = ["gpui-storybook/dock"]
```

Then use `create_dock_window` and `StoryWorkspace::view`:

```rust
gpui_storybook::create_dock_window(
    "My App - Stories",
    |window, cx| {
        let stories = gpui_storybook::generate_stories(window, cx);
        gpui_storybook::StoryWorkspace::view(stories, window, cx)
    },
    cx,
);
```

Registration, filtering, preferences, and MCP routes are the same in both
window modes. Both modes include the right-side workbench. Gallery mode renders
it as a third resizable region.

Dock mode installs it as an open, collapsible right dock. The saved layout
includes its width, visibility, and selected tab. Layouts created before the
workbench schema are replaced with the current three-region default. Choose
**Reset layout** in the title bar to restore the left story sidebar, center
story tabs, and right workbench.
