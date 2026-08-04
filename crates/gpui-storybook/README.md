# gpui-storybook

`gpui-storybook` is the public facade for adding a searchable Storybook window
to a GPUI application. It exposes initialization, gallery and window helpers,
story registration, typed preferences, localization support, and optional dock
and MCP integrations.

Most applications should use this crate rather than the lower-level workspace
crates.

## Features

| Feature | Default | Purpose |
|---|---:|---|
| `macros` | Yes | Re-export `#[story]`, `#[story_init]`, `ComponentStory`, and `Substory` |
| `dock` | No | Add the panel-based `StoryWorkspace` |
| `mcp` | No | Add MCP tools and PNG capture support |

Initialization is asynchronous: call `gpui_storybook::init`, await the returned
readiness task, and only then create the first story window.

See the [getting-started guide](../../book/src/getting_started.md), [story
guide](../../book/src/stories.md), and [API documentation](https://docs.rs/gpui-storybook/).
