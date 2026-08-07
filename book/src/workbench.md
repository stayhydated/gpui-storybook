# Use the workbench

The right-side workbench edits the selected story instance, previews the active
theme, and opens GPUI Inspector without rebuilding the application. It is
available in both the gallery and dock workspace; grouped stories expose an
explicit variant selector so edits target one concrete variant.

## Add controls to an explicit story

Derive `StoryControls` on the story struct and mark only fields that should be
editable:

```rust
#[derive(gpui_storybook::StoryControls)]
pub struct ButtonStory {
    focus_handle: gpui::FocusHandle,

    #[storybook(control(
        label = "Disabled",
        description = "Disable every button in the preview",
        category = "State"
    ))]
    disabled: bool,

    #[storybook(control(
        min = 0.0,
        max = 32.0,
        step = 1.0,
        category = "Layout"
    ))]
    padding: f32,
}
```

Keep implementing `Story`, `Render`, and `Focusable` as described in
[Write stories](stories.md). `Story::new_view` returns `Entity<Self>`, which
lets the generated control adapter update that exact GPUI entity.

Fields without `#[storybook(control...)]` remain private to the story. Control
registration is explicit, so only marked fields appear in the workbench.

## Add controls to a component story

Place the same field attributes on a `ComponentStory`:

```rust
#[derive(gpui::IntoElement, gpui_storybook::ComponentStory)]
#[storybook(example = WelcomeCard::example())]
pub struct WelcomeCard {
    #[storybook(control(category = "Content"))]
    title: gpui::SharedString,

    #[storybook(control(category = "State"))]
    highlighted: bool,
}
```

The generated wrapper evaluates `example = ...` for defaults, stores the
controlled fields, reconstructs the component during rendering, and overlays
the current values. **Reset** and **Reset all** therefore restore values from
the configured example rather than type defaults.

## Choose control types

The derive infers an editor from each marked field:

| Rust field | Default editor |
|---|---|
| `bool` | Checkbox |
| `i8` through `i64`, `isize`, `u8` through `u32`, `usize` | Number or range |
| `f32`, `f64` | Number or range |
| `String`, `SharedString` | Text |
| `Hsla` | Color picker |
| Enum-like type with `options` | Select |

Adding `min` or `max` selects a range editor. `step` controls its increment.
Bounds apply before the value reaches the story entity, so an out-of-range
automation request fails without changing the preview.

An enum-like control supplies the visible serialized choices and implements
`Display` and `FromStr`:

```rust
#[storybook(control(options = ["Primary", "Danger"]))]
intent: ButtonIntent,
```

Leave collections and application-specific types unmarked. Marking an
unsupported type as a control produces a compile diagnostic instead of silently
omitting it.

## Work with the preview

The workbench header provides settings shared by the selected story in that
window:

- **Responsive**, **Mobile**, **Tablet**, and **Desktop** viewport presets;
- theme, light, dark, and transparent canvas backgrounds;
- an optional alignment grid;
- explicit variant buttons for grouped stories.

The **Inspect** tab shows the active story key with a copy button. Select the
source location to open the story file with the system's configured application,
or choose **Open GPUI Inspector** to inspect the rendered element tree. Each
Storybook window owns its own selection, preview settings, and controls.

## Edit the active theme

Open the **Theme** tab to search every serialized `ThemeColor` field and edit it
with a color picker. Each edit replaces the active color set, rebuilds derived
theme tokens, and refreshes open windows. Use the row-level reset or **Reset
all** to return to the selected base theme.

**Copy export** copies the current color set as deterministic JSON. Put
compatible `ThemeColor` JSON on the clipboard and choose **Import clipboard**
to apply it as a session draft. Selecting a different base theme clears the
draft. Reloading the same base theme from disk rebases and reapplies current
session overrides.

For native debug builds, set `STORYBOOK_THEME_DIR` before launch to use a
consumer-owned directory as the custom-theme source:

```bash
STORYBOOK_THEME_DIR=./themes cargo run -p my-app-storybook
```

The directory is watched for create, modify, and remove events. The environment
override is the complete custom-theme directory for that process; when it is
unset, Storybook watches its bundled theme directory.

Wasm supports immediate in-app control and theme edits. Watching external files
in Wasm requires a separate development-server bridge.

## Inspect a story root

Choose **Open GPUI Inspector** in the **Inspect** tab, or use GPUI Component's
Inspector keyboard shortcut. The story root publishes its stable key, title,
source location, and available control keys through a custom inspector state.
The Inspector strip and Storybook workbench can remain open together.

## Verify the result

Open a controlled story and change one value. Only that story instance should
rerender, **Reset** should restore its example value, and switching stories or
variants should replace the displayed controls. In dock mode, close and reopen
the application to verify that workbench width, visibility, and selected tab
restore with the layout.

Live edits change serialized values and theme data. Changed Rust types or
component source still require recompilation.

## Troubleshoot

| Symptom | Likely cause | Action |
|---|---|---|
| The Controls tab is empty | No fields on the active variant are marked | Add `#[storybook(control)]` to supported fields and rebuild |
| A control derive fails | The marked field type is unsupported or attributes conflict | Use a supported type, enum `options`, or leave the field unmarked |
| Reset uses an unexpected value | The configured story constructor produced that default | Check `Story::new_view` or `ComponentStory`'s `example = ...` expression |
| External themes do not reload | The process is not a native debug build or the directory is wrong | Set `STORYBOOK_THEME_DIR` before launch and confirm the directory exists |
| Inspector opens without Storybook metadata | A nested element is selected | Select the `storybook-inspectable-*` story root |
