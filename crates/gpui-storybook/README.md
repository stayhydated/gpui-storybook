# gpui-storybook

`gpui-storybook` is the public facade for adding a searchable Storybook window
to a GPUI application. It exposes initialization, gallery and window helpers,
story registration, typed controls, the live controls, theme, inspect, and actions
workbench, typed preferences, localization support, and optional GPUI Inspector,
dock, and MCP integrations.

Most applications should use this crate rather than the lower-level workspace
crates.

## Features

| Feature | Default | Purpose |
|---|---:|---|
| `macros` | Yes | Re-export `#[story]`, `#[story_init]`, `StoryControls`, `ComponentStory`, and `Substory` |
| `dock` | No | Add the panel-based `StoryWorkspace` |
| `inspector` | No | Add the GPUI Inspector button, UI, and story-root metadata |
| `mcp` | No | Add MCP tools, opt-in in-process interaction, and PNG capture support |
| `performance` | No | Add GPUI window timing histograms and the debug frame overlay to the workbench |

Initialization is asynchronous: call `gpui_storybook::init`, await the returned
readiness task, and only then create the first story window.

Both the gallery and dock workspace include a right-side workbench. Derive
`StoryControls` on explicit story structs, or mark fields on a
`ComponentStory`, to get live field editors and reset behavior. Only fields
marked with `#[storybook(control...)]` are registered:

```rust
#[derive(gpui_storybook::StoryControls)]
struct ButtonStory {
    #[storybook(control)]
    disabled: bool,
}
```

Implement `Story::scenarios()` for explicit stories, or pass
`scenarios = Component::scenarios()` to `ComponentStory`, to publish reusable
named interaction flows. The Scenarios workbench tab and MCP list/run tools use
the same executor. Every scenario recreates its concrete story before applying
controls, presentation, steps, exact semantic postconditions, and optional
capture. Normal `gpui_storybook::init` installs the live in-process runner, so
**Run fresh** works in a standard `cargo run`; the `mcp` feature adds remote
tools and capture support to that same controller. The sticky Scenarios
toolbar's **Reset** action recreates the story at its constructor defaults and
clears the last run result without executing a scenario.

`static_story_catalog()` and the JSON export helpers read linked registration
metadata without constructing a story or opening GPUI. The deterministic output
contains stable keys, section/source provenance, declaration Rustdocs, and
static control kinds, labels, bounds, and options. Localized runtime copy and
constructor-derived defaults remain in the live catalog.

The preview canvas stays centered inside a visible frame. **Mobile**, **Tablet**,
and **Desktop** use locked preset dimensions; **Responsive** exposes resize
handles and starts from the dimensions of the fixed preset selected immediately
before it. The canvas remains centered within the visible main pane as the
sidebars change width or visibility; dedicated left and right panel icons sit in
the top bar immediately before the appearance settings button. Responsive
frames keep a small, symmetric resize gutter so every edge and corner handle
remains reachable, including when the frame is larger than the visible pane.

The Theme tab edits every serialized theme color in memory. Native debug builds
can watch a consumer theme directory by setting `STORYBOOK_THEME_DIR` before
launch; Wasm supports in-app editing without filesystem watching. Choosing a
named base theme also activates its registered light or dark appearance, while
Storybook remembers the theme in the opposite slot for later appearance
changes.

The Inspect tab always shows the selected story's key and source. With
`inspector` enabled, it also opens GPUI Component's Inspector and publishes the
selected story's key, title, source, and control keys to that Inspector.

The Actions tab follows the selected story's opt-in
`Story::action_scope_focus_handle` and lists default-buildable actions,
documentation, argument schemas, and effective key bindings on that explicit
page/component root. Actions from nested inputs and the Storybook shell/root
stay out of the list. Dispatch targets the same scope even while the workbench
is focused; stories without a scope expose no inferred actions. The sticky
Actions toolbar's **Reset** action recreates the active story before the next
dispatch. With `performance` enabled, the Perf tab shows frame and input-latency
percentiles and controls GPUI's debug frame overlay.

The explicit example's
[`ActionsAndScenariosStory`](../../examples/story/src/stories/actions_scenarios_story.rs)
shows Buttons, contextual shortcuts, the Actions tab, and scenarios dispatching
the same GPUI commands.

With `mcp` enabled, set both `GPUI_STORYBOOK_MCP_STDIO=1` and
`GPUI_STORYBOOK_MCP_ALLOW_INTERACTION=1` to advertise generic focus, keyboard,
registered-action, story-relative pointer, scroll, frame-wait, and atomic
post-interaction capture tools. The interaction gate is separate because a
story action can have arbitrary application effects. Typed controls remain the
preferred reproducible input contract. Wrap important controls with
`StorybookElementExt::storybook_target()` so their GPUI IDs become stable keys
and MCP can list their live bounds and use `storybook_click_target` or a
`click_target` step without screen coordinates. Wrap Serde-serializable rendered
state with `StorybookElementExt::storybook_value(&state)` so
`storybook_read_value` and `storybook_wait_for_value` can inspect application
postconditions without a screenshot. Storybook derives a readable label from the ID; the
`_as` variants accept explicit keys and labels for opaque elements or localized
copy. MCP route, target, value, and control inputs use the explicit
`story_key`, `target_key`, `value_key`, and `control_key` field names. Initial
tool calls wait for the standard live host with a bounded startup deadline.
Capture dimensions size
the story region without replacing the surrounding gallery or dock layout. On
Linux, install `gpui-storybook-launch`; `storybook_capture_launch_env` emits
that Sway-backed command so the normal Wayland application can run without a
physical display. The returned PNG is cropped to the story region and excludes
the mounted Storybook chrome. Closing MCP stdin terminates the GUI process and
lets the launcher stop its compositor.

See the [getting-started guide](../../book/src/getting_started.md), [story
guide](../../book/src/stories.md), [workbench guide](../../book/src/workbench.md),
the [automation guide](../../book/src/automation.md), and [API
documentation](https://docs.rs/gpui-storybook/). Use the public-integration
[`gpui-storybook-test`](../gpui-storybook-test/README.md) crate and the
[portable-testing guide](../../book/src/portable_testing.md) for fresh headless
stories, capture matrices, visual baselines, and frame budgets.
