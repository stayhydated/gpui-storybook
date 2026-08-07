# Storybook Workbench Plan

## Goal

Add a right-side workbench that makes `gpui-storybook` behave more like
Storybook. The workbench will provide per-field story controls, live theme
editing, and GPUI Inspector integration while supporting immediate rerendering
of control and theme changes.

In this plan, **hot reload** means updating control values and theme data
without recompiling the application. Hot-reloading changed Rust types or
component source code would require a separate dynamic-library or
process-restart system and is outside this project's initial scope.

## Feasibility

The proposed workbench is feasible with the current architecture and pinned
dependencies.

| Capability | Feasibility | Notes |
| --- | --- | --- |
| Right-side workbench | High | The pinned dock library supports `set_right_dock`; the current layout already marks the right edge as collapsible. |
| Per-field controls | High | Proc-macro-generated metadata is required because Rust supports downcasting but not general field reflection. |
| GPUI Inspector integration | High | The inspector feature is already enabled. It can be toggled and extended with custom inspector state. |
| Live global theme editing | High | GPUI Component themes are mutable and serializable; refreshing windows applies changes immediately. |
| Native theme-file watching | High | An existing watcher already runs in debug-native builds. |
| Wasm live controls and themes | High | In-memory changes work; filesystem watching requires a dev-server or WebSocket bridge. |
| Rust source hot reload | Separate project | This is not part of the initial workbench implementation. |

The design follows Storybook's model: args are serializable values that
rerender a story, while ArgTypes describe controls, bounds, options, and
categories. See [Storybook Args](https://storybook.js.org/docs/writing-stories/args)
and [ArgTypes](https://storybook.js.org/docs/api/arg-types).

## Proposed Architecture

```text
StoryEntry
    └── StoryContainer
            ├── typed story Entity<S>
            └── ControlTarget
                    │
                    ▼
        window-scoped WorkbenchState
            ├── Controls → mutate Entity<S> → rerender
            ├── Theme    → mutate global Theme → refresh windows
            └── Inspect  → toggle GPUI Inspector / expose story metadata
```

The workbench state must be window-scoped rather than process-global so that
multiple Storybook windows can independently select and edit stories.

### Controls Model

Introduce a small typed model in `gpui-storybook-core`:

- `ControlValue`: boolean, integer, float, text, color, choice, and later JSON.
- `ControlKind`: checkbox, number/range, text, color picker, select, and custom.
- `ControlSpec`: key, label, description, category, default, bounds, and options.
- An object-safe `ControlTarget` trait for reading, writing, and resetting values.
- Typed generated adapters that hold `Entity<S>` and call `Entity::update`.

A heterogeneous `Rc<dyn ControlTarget>` is appropriate at this runtime
boundary because GPUI state is main-thread-oriented and different story types
must share one collection.

Rust's `Any` only supports type checks and downcasting; it cannot enumerate
fields. Proc macros receive the struct's token representation and can generate
the required field adapters. See the [Rust `Any`
documentation](https://doc.rust-lang.org/std/any/index.html) and [procedural
macro reference](https://doc.rust-lang.org/reference/procedural-macros.html).

## Implementation Plan

### Phase 0: Feasibility Spike

Build one internal vertical slice before stabilizing the public API:

1. Add a temporary right dock panel.
2. Control one boolean or number field on the explicit `ButtonStory`.
3. Control one field on a `#[derive(ComponentStory)]` example.
4. Change one theme color through `ColorPicker`.
5. Add an **Open Inspector** button.
6. Verify that the Inspector and workbench can coexist in one window.

Acceptance criteria:

- Edits rerender immediately.
- Reset restores defaults.
- Switching stories updates the panel.
- Both registration styles compile and behave consistently.
- Inspector activation does not corrupt the dock layout.

### Phase 1: Controls Domain and Story Plumbing

Extend `crates/gpui-storybook-core/src/registry.rs` and
`crates/gpui-storybook-core/src/story/components.rs`:

1. Attach an optional `ControlTarget` to each `StoryContainer`.
2. Add a window-scoped `WorkbenchState` that tracks the active story.
3. Notify that state from both dock activation and gallery selection.
4. Add reset-one and reset-all operations.
5. Consider tightening `Story::new_view` to return `Entity<Self>`; current
   activation behavior already assumes that the returned entity can represent
   `Self`.
6. Define typed errors for invalid values, unknown controls, and range
   violations.
7. Cover value conversion, reset behavior, invalid setters, and independent
   window state with focused unit tests.

The workspace is pre-1.0, so this should be a clean forward-only API rather
than a compatibility layer around an obsolete controls model.

### Phase 2: Right-Side Workbench UI

Create a reusable `StoryWorkbench` with three tabs:

- **Controls** for active-story values.
- **Theme** for global theme editing.
- **Inspect** for GPUI Inspector integration and story metadata.

For dock mode:

1. Add `set_right_dock(...)` in
   `crates/gpui-storybook-core/src/dock_gallery.rs`.
2. Register the new panel alongside the existing story panels.
3. Bump the persisted dock schema from version 5 to version 6 so restored
   layouts acquire the new panel.
4. Persist width, visibility, and selected tab.
5. Keep the right edge collapsible and provide an explicit reset-layout path.

The exact pinned dependency already exposes right-dock installation and
persistence in its [DockArea
implementation](https://github.com/longbridge/gpui-component/blob/181cf147d1d68ff33bae60abb58a24917658142e/crates/ui/src/dock/mod.rs).

For normal gallery mode, add a third resizable region to the existing
`h_resizable` layout in `crates/gpui-storybook-core/src/gallery.rs`.

The UI must also define behavior for a story group containing multiple
variants. The initial implementation should display controls for the active
variant and make that active variant explicit in the workbench header.

### Phase 3: Macro-Generated Field Controls

Add an opt-in controls derive or helper attributes rather than attempting
runtime reflection.

For explicit stories, use:

```rust
#[derive(StoryControls)]
struct ButtonStory {
    #[storybook(control)]
    disabled: bool,

    #[storybook(control(min = 0.0, max = 32.0, step = 1.0))]
    padding: f32,

    focus_handle: FocusHandle,
}
```

Control registration is explicit. Fields without `#[storybook(control...)]`
remain story-only state.

For `ComponentStory`, extend the generated wrapper:

1. Evaluate the configured example to obtain defaults.
2. Store controllable field values on the wrapper.
3. Recreate the example during rendering.
4. Replace its controllable fields with current values.
5. Render the resulting component.

Initially infer controls for:

- `bool`
- integer and floating-point values
- `String` and `SharedString`
- `Hsla`
- enums with explicitly supplied options

Collections and application-specific types remain unmarked unless they have a
custom editor.
An explicitly requested but unsupported control must produce a compile
diagnostic rather than silently disappearing.

Update the inline macro tests and matching `insta` snapshots in the same
change. Migrate representative stories in both example applications so each
registration style remains executable documentation.

### Phase 4: Theme Editor and Hot Reload

The Theme tab should edit a session draft layered over the selected base theme.

1. Serialize `ThemeColor` into a deterministic list of named colors.
2. Render a searchable color-picker row for every serialized color.
3. After an edit:
   - replace `Theme.colors`,
   - rebuild `Theme.tokens` from the new colors,
   - call `refresh_windows()`.
4. Support reset-color, reset-all, import, and export.
5. Reset the draft intentionally when the user selects a different base theme.
6. When the same theme reloads from disk, rebase and reapply the session draft.
7. Test that every serialized theme color has a UI row so new upstream color
   fields cannot be omitted silently.

Rebuilding tokens is essential because components can read either direct theme
colors or derived tokens.

For native development, extend the watcher already initialized in
`crates/gpui-storybook-core/src/story/themes.rs` to watch a
consumer-configured theme directory. Use debounced atomic writes and a content
hash to prevent save/watch feedback loops. The pinned [ThemeRegistry
watcher](https://github.com/longbridge/gpui-component/blob/181cf147d1d68ff33bae60abb58a24917658142e/crates/ui/src/theme/registry.rs)
already handles recursive create, modify, and remove events.

The configuration mechanism for a consumer theme directory should be decided
during this phase. If it changes `storybook.toml`, update its schema, resolver,
tests, both example configurations, and all synchronized configuration
documentation.

For Wasm, retain immediate in-app editing. External file hot reload must be
documented as requiring a later dev-server bridge.

### Phase 5: GPUI Inspector Integration

Do not copy or instantiate GPUI's private Inspector implementation.

Instead:

1. Dispatch the existing inspector toggle action from the Inspect tab.
2. Display the active story key with a copy action and link the source location
   to the system's configured application.
3. Wrap the story root in a custom inspectable element.
4. Register a `StoryInspectorState` renderer so selecting the story root in
   GPUI Inspector exposes Storybook-specific metadata.
5. Verify the Inspector's own right-side strip and the workbench remain usable
   when open simultaneously.

The pinned inspector already registers custom element state and provides
platform shortcuts in [gpui-component's inspector
integration](https://github.com/longbridge/gpui-component/blob/181cf147d1d68ff33bae60abb58a24917658142e/crates/ui/src/inspector.rs).

### Phase 6: Automation and Additional Storybook Features

After the controls model is stable:

- Add MCP operations to read, set, and reset controls so captures are
  reproducible.
- Allow serialized args in capture requests or route state.
- Add viewport sizing and presets.
- Add background and grid selection.
- Consider URL or state serialization for shareable story configurations.

These features should reuse `ControlSpec` and `ControlValue` rather than
introducing a second automation-specific value model.

## Cross-Cutting Acceptance Criteria

- Editing a control rerenders only the corresponding story instance.
- Reset-one and reset-all restore values derived from the story example.
- Story selection updates the workbench in both gallery and dock modes.
- Multiple windows keep independent active-story and control state.
- Every theme color is editable, and derived theme tokens stay synchronized.
- Native external theme edits are applied without restart or watcher loops.
- Wasm supports live in-app controls and theme editing.
- The built-in GPUI Inspector remains available through its keyboard shortcut
  and the workbench action.
- Restored dock layouts include the right workbench after the layout-version
  migration.
- No documentation claims that changed Rust component source is hot-reloaded.

## Validation

Run focused validation as each phase lands:

```bash
cargo test -p gpui-storybook-core --all-features --locked
cargo test -p gpui-storybook-macros --locked
cargo check --manifest-path examples/story/Cargo.toml --features dock --locked
cargo check --manifest-path examples/component/Cargo.toml --features dock --locked
just fmt
just clippy
just test
```

Also perform manual native smoke tests for:

- control editing and reset,
- switching stories and variants,
- workbench resizing and dock restoration,
- simultaneous workbench and Inspector use,
- external theme-file reload,
- opening multiple Storybook windows.

Validate Wasm separately for in-app control and theme changes. Filesystem watch
behavior is not an applicable Wasm acceptance criterion until a development
bridge exists.

## Documentation Synchronization

Because this changes public story registration, dock behavior, theme behavior,
and potentially MCP capture semantics, update the following surfaces alongside
implementation:

- `README.md`
- `crates/gpui-storybook/README.md`
- `crates/gpui-storybook-core/README.md` and affected Rustdocs
- `crates/gpui-storybook-macros/README.md`, macro Rustdocs, tests, and snapshots
- `examples/story/README.md` and representative stories
- `examples/component/README.md` and representative components
- `skills/use-gpui-storybook/SKILL.md`
- matching chapters under `book/src` and `book/src/SUMMARY.md`
- catalog copy in `web/src/lib.rs`
- MCP README and automation Rustdocs if control operations are exposed through
  MCP

If theme-directory configuration is added to `storybook.toml`, also update
`crates/gpui-storybook-toml/README.md`, its schema and tests, and both example
`storybook.toml` files.

Build generated publication artifacts through `cargo xtask`; do not edit them
directly.
