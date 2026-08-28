# Use the workbench

The right-side workbench edits the selected story instance, previews the active
theme, shows its key and source location, and inspects selected-story GPUI
actions without rebuilding the application.
It is available in both the gallery and dock workspace; grouped stories expose
a **Variant** select so edits target one concrete variant. Gallery mode renders
only that member, while dock mode opens selected members as independent tabs.

Enable and forward the opt-in Inspector feature to add the **Open GPUI
Inspector** button and story-root metadata:

```toml
[features]
inspector = ["gpui-storybook/inspector"]
```

Launch the Storybook package with `--features inspector` when using that button.
The Inspect tab remains available without the feature.

Forward the opt-in performance feature to add GPUI window histograms and its
debug frame overlay:

```toml
[features]
performance = ["gpui-storybook/performance"]
```

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
- a **Variant** select for stories that share one navigation entry.

Every viewport is centered inside a bordered frame. **Mobile**, **Tablet**, and
**Desktop** lock the frame to their preset dimensions. **Responsive** provides
width, height, and corner resize handles; when selected after a fixed preset, it
starts at that preset's size. The initial Responsive view uses the available
preview area. Resizing or hiding the story navigation and workbench sidebars keeps
the canvas centered within the visible main pane. Use the left and right panel
icons immediately before the top-bar appearance settings button to toggle those
sidebars. Responsive reserves a small, symmetric resize gutter that keeps every
handle reachable, even when the frame is larger than the visible pane.

The **Inspect** tab shows the active story key with a copy button. Select the
source location to open the story file with the system's configured application.
With `inspector` enabled, choose **Open GPUI Inspector** to inspect the rendered
element tree. Each Storybook window owns its own selection, preview settings,
and controls.

## Run story scenarios

Open **Scenarios** to see workflows declared by the selected story. Each row
shows the stable scenario key, description, and ordered named steps. **Run
fresh** recreates the concrete story entity, rebinds the workbench control
target and focus handle, applies initial controls and presentation, executes the
steps, evaluates exact semantic postconditions, and optionally captures a PNG.
The standard `gpui_storybook::init` path installs this live in-process runner,
so **Run fresh** works in an ordinary application launch. The `mcp` feature
connects remote tools and capture support to the same controller. The sticky
toolbar keeps **Reset** available while scenario rows scroll. It recreates the
selected story at its constructor defaults, rebinds the same runtime adapters,
and clears the displayed run result without executing a scenario.

During a run, the panel marks steps as running or queued. A completed run shows
passed, failed, and unexecuted steps plus its postcondition count and capture
path. Runtime input is never retried after a partial failure. Switching back to
Controls after completion edits the recreated story instance.

## Inspect actions and key bindings

Select a story, then open **Actions**. The tab uses the story's opt-in
`Story::action_scope_focus_handle` instead of its primary `Focusable` handle or
the window's current focus. Track the action-scope handle on the root element
that installs the page or component action handlers:

```rust
fn action_scope_focus_handle(&self, _: &gpui::App) -> Option<gpui::FocusHandle> {
    Some(self.action_scope_focus_handle.clone())
}

// In Render::render:
div()
    .track_focus(&self.action_scope_focus_handle)
    .on_action(cx.listener(Self::handle_page_action))
    .child(Input::new(&self.input))
```

Use a separate handle when the primary story focus belongs to a nested input.
The tab keeps only default-buildable actions available at the explicit root
scope after excluding actions also exposed through Storybook's workbench/root
path. Nested input actions, actions from other components, and Storybook shell
commands stay out of the list. A story without an action scope shows no inferred
actions.

Each row includes action documentation, its registered JSON argument schema,
and every effective key binding resolved for the action scope. **Dispatch**
sends the action directly to the same scope through GPUI's normal action
dispatcher. The sticky toolbar keeps **Reset** available while action rows
scroll; Reset recreates the selected story and rebinds its action scope before
the next dispatch.

Parameterized actions whose type cannot be constructed from an empty object do
not appear in GPUI's available-action list. Use typed story scenarios or the
automation action step when a specific argument payload is part of the example.

See
[`ActionsAndScenariosStory`](../../examples/story/src/stories/actions_scenarios_story.rs)
for a complete example where Buttons, contextual shortcuts, Actions-tab
dispatch, and two reusable scenarios share the same three GPUI actions and
rendered semantic state.

## Inspect window performance

Launch with `--features performance` and open **Perf**. The tab reads GPUI's
native histograms for draw duration, dirty-to-present latency, intervals between
animated presentations, and input-to-frame latency. Each metric reports sample
count plus p50, p95, p99, and maximum duration. **Refresh** takes a new snapshot;
**Overlay** cycles GPUI's hidden, minimal, and full frame overlay modes.

These cumulative window metrics help explain an interactive preview. Enforce
story-isolated budgets with the portable test runner so shell rendering and
earlier stories do not contaminate a regression result.

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

The **Inspect** tab provides the story key and source location by default. Build
with the `inspector` feature, then choose **Open GPUI Inspector** or use GPUI
Component's Inspector keyboard shortcut. The story root publishes its stable
key, title, source location, and available control keys through a custom
inspector state. The Inspector strip and Storybook workbench can remain open
together.

## Verify the result

Open a controlled story and change one value. Only that story instance should
rerender, **Reset** should restore its example value, and switching the
**Variant** select should replace the preview and displayed controls. In dock
mode, select two variants and verify that each remains available in its own tab,
then close and reopen the application to verify that workbench width, visibility,
and selected tab restore with the layout.

Live edits change serialized values and theme data. Changed Rust types or
component source still require recompilation.

## Troubleshoot

| Symptom | Likely cause | Action |
|---|---|---|
| The Controls tab is empty | No fields on the active variant are marked | Add `#[storybook(control)]` to supported fields and rebuild |
| A control derive fails | The marked field type is unsupported or attributes conflict | Use a supported type, enum `options`, or leave the field unmarked |
| Reset uses an unexpected value | The configured story constructor produced that default | Check `Story::new_view` or `ComponentStory`'s `example = ...` expression |
| External themes do not reload | The process is not a native debug build or the directory is wrong | Set `STORYBOOK_THEME_DIR` before launch and confirm the directory exists |
| The Open GPUI Inspector button is unavailable | The Storybook package was launched without Inspector support | Forward `gpui-storybook/inspector` and launch with `--features inspector` |
| Inspector opens without Storybook metadata | A nested element is selected | Select the `storybook-inspectable-*` story root |
| A scenario is missing | The active story did not declare it, or a component derive omitted `scenarios = ...` | Implement `Story::scenarios()` or pass the component scenario expression and rebuild |
| A scenario fails after some steps | A runtime handler, semantic postcondition, or capture failed | Inspect the named step result and error; fix the story or scenario and start a new fresh run |
| An expected action is absent | The story did not expose and track an action-scope handle, or the action needs arguments to build | Implement `Story::action_scope_focus_handle`, track it on the handler-owning root, or use a typed scenario for parameterized actions |
| The Perf tab is absent | The Storybook package was launched without GPUI profiler instrumentation | Forward `gpui-storybook/performance` and launch with `--features performance` |
