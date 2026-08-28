# Setup and configuration

Read this reference when adding or changing a Storybook binary, locale adapter,
preferences, window mode, or `storybook.toml`.

## Startup sequence

Use the facade's startup order:

1. Build the native GPUI application with
   `gpui_platform::application().with_assets(gpui_storybook::Assets)`.
2. Construct a stable `ConsumerId` unique to the Storybook binary.
3. Construct typed `StorybookOptions` with the fallback language and locale
   adapter.
4. Call `gpui_storybook::init` and handle `StorybookInitError`.
5. Await the returned `Task<StorybookReady>`.
6. Inspect readiness diagnostics.
7. Generate stories and construct the first gallery or dock window.

Opening the window before step 5 can render a first frame with default
preferences.

On Linux, run MCP and startup-capture sessions through
`gpui-storybook-launch`, which owns Sway's wlroots headless lifecycle. This
preserves the normal Wayland-backed application path while providing an
in-memory compositor; see the automation reference for the command and runtime
packages.

## Locale contract

Keep these pieces aligned:

- call `es_fluent_build::track_i18n_assets()` from `build.rs`;
- define the embedded module and `#[es_fluent_language]` enum in
  library-reachable code;
- reference the private embedded-module static inside the locale adapter;
- call `gpui_es_fluent::replace_with_language`;
- pass the enum's fallback and adapter to `StorybookOptions::new`.

The private static name is the Cargo package name in upper snake case followed
by `_I18N_MODULE`.

## Window selection

Use `create_new_window` plus `Gallery::view` for the default browser.

For the dock workspace, forward the feature and use `create_dock_window` plus
`StoryWorkspace::view`:

```toml
[features]
dock = ["gpui-storybook/dock"]
```

Both modes include the right workbench. Gallery uses a third resizable region.
Dock mode persists the right dock's width, visibility, and selected tab; use
**Reset layout** in the title bar to restore the current default layout.

The Actions tab reads the selected story's opt-in
`Story::action_scope_focus_handle`, excludes nested-control and Storybook
shell/root actions, and resolves effective bindings for that explicit scope. A
story without an action scope exposes no inferred actions. Its sticky toolbar's
**Reset** action recreates the active story and rebinds that scope before the
next dispatch. Forward the performance feature when the binary should add
native window timing histograms and GPUI's debug frame overlay:

```toml
[features]
performance = ["gpui-storybook/performance"]
```

Forward the Inspector feature when the Storybook package should expose the GPUI
Inspector button and story-root metadata:

```toml
[features]
inspector = ["gpui-storybook/inspector"]
```

The workbench edits controls on the active concrete variant. Viewport,
selection, and action/performance inspection state belong to that Storybook
window.
Theme edits are session overrides on the process-global GPUI Component
theme: they rebuild derived tokens and refresh open windows without changing
saved preference intent. **Copy export** and **Import clipboard** exchange a
complete `ThemeColor` JSON object. Selecting a different base theme clears the
draft, while reloading the same named theme reapplies its session overrides.

The **Inspect** tab always shows the story key and source location. With
`inspector` enabled, its button dispatches GPUI Component's Inspector toggle and
the inspectable story root publishes its key, title, source location, and
control keys. Live control and theme edits change serialized runtime values;
changed Rust types or component source require recompilation.

For consumer theme development in a native debug build, set
`STORYBOOK_THEME_DIR` before launch. The path becomes the process's complete
custom-theme directory and is watched for external changes. Wasm keeps in-app
theme edits but needs a separate development bridge for filesystem changes.

## Configuration rules

Put `storybook.toml` beside the story crate's `Cargo.toml`:

```toml
group = "UI Kit"
allow = ["UI Kit", "Shared"]
disable_story = ["ExperimentalCardStory"]

[overrides]
color_scheme = "dark"
theme = "Default Dark"
language = "en"
```

Apply these semantics:

- `group` is required when the file exists.
- Omitted `allow` includes only the file's own normalized group.
- `allow = ["*"]` includes every group.
- `allow = []` includes no groups.
- `disable_story` matches the registered type name exactly.
- Component registrations use the component type, not the generated wrapper.
- The active runtime config belongs to the registered story package whose name
  matches the running binary.
- Programmatic overrides win field by field over TOML.
- MCP deterministic overrides win over programmatic and TOML values.
- Overrides change resolved presentation without rewriting saved intent.

Invalid static configuration makes `init` return `StorybookInitError`.
Unavailable registered theme names fall back with a diagnostic.

## Preference contract

Persistent mode stores `.gpui-storybook/{consumer-id}.json` at the workspace
or standalone package root. `Temporary` uses isolated temporary files.
`Disabled` keeps state in memory. Use `with_json_path` only with persistent
mode.

Treat `PreferenceState::saved` as user intent and `resolved` as effective
presentation. Selecting a named light or dark theme saves it in the matching
slot and activates that appearance immediately; selecting `System` afterward
resumes device appearance changes with both saved theme slots. Launch-only
appearance overrides remain higher priority. Use `try_preference_state` for a
read-only snapshot.
