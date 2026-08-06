# gpui-storybook

`gpui-storybook` is the public facade for adding a searchable Storybook window
to a GPUI application. It exposes initialization, gallery and window helpers,
story registration, typed controls, the live theme and Inspector workbench,
typed preferences, localization support, and optional dock and MCP integrations.

Most applications should use this crate rather than the lower-level workspace
crates.

## Features

| Feature | Default | Purpose |
|---|---:|---|
| `macros` | Yes | Re-export `#[story]`, `#[story_init]`, `StoryControls`, `ComponentStory`, and `Substory` |
| `dock` | No | Add the panel-based `StoryWorkspace` |
| `mcp` | No | Add MCP tools and PNG capture support |

Initialization is asynchronous: call `gpui_storybook::init`, await the returned
readiness task, and only then create the first story window.

Both the gallery and dock workspace include a right-side workbench. Derive
`StoryControls` on explicit story structs, or mark fields on a
`ComponentStory`, to get live field editors and reset behavior:

```rust
#[derive(gpui_storybook::StoryControls)]
struct ButtonStory {
    #[storybook(control)]
    disabled: bool,
}
```

The Theme tab edits every serialized theme color in memory. Native debug builds
can watch a consumer theme directory by setting `STORYBOOK_THEME_DIR` before
launch; Wasm supports in-app editing without filesystem watching.

See the [getting-started guide](../../book/src/getting_started.md), [story
guide](../../book/src/stories.md), [workbench guide](../../book/src/workbench.md),
and [API documentation](https://docs.rs/gpui-storybook/).
