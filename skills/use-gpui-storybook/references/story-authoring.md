# Story authoring

Read this reference when adding registrations, metadata, sections, substories,
or one-time setup.

## Choose a registration

Use `#[gpui_storybook::story(...)]` when the preview owns GPUI state, a focus
handle, actions, lifecycle, or wrapper UI. Implement `Story`, `Render`, and
`Focusable`, and derive or implement `StoryControls`.

Use `#[derive(gpui_storybook::ComponentStory)]` when the component implements
`IntoElement` and Storybook can generate its wrapper. The derive supports
`title`, `description`, `section`, and `example`.

```rust
#[derive(gpui::IntoElement, gpui_storybook::ComponentStory)]
#[storybook(
    title = "Welcome Card",
    section = crate::StorySection::Intro,
    example = WelcomeCard::example(),
)]
pub struct WelcomeCard {
    // component fields
}
```

The derive accepts non-generic structs. Without `example`, the wrapper uses
`Default::default()`. Title and description expressions have
`cx: &gpui::App` in scope and may call `localize_message`.

## Expose story-root actions

An explicit story opts into the Actions workbench with
`Story::action_scope_focus_handle`. Track the returned handle on the root
element that installs the story's action handlers:

```rust
fn action_scope_focus_handle(&self, _: &gpui::App) -> Option<gpui::FocusHandle> {
    Some(self.action_scope_focus_handle.clone())
}

// In Render::render:
gpui::div()
    .track_focus(&self.action_scope_focus_handle)
    .on_action(cx.listener(Self::handle_page_action))
```

Keep this handle separate from `Focusable::focus_handle` when the primary
interaction focus belongs to a nested input or another child component. The
workbench queries and dispatches only through the explicit root scope; a story
without one exposes no inferred actions.

Use `examples/story/src/stories/actions_scenarios_story.rs` as the complete
reference for Buttons, contextual key bindings, Actions-tab dispatch, and
story-owned scenarios sharing one GPUI command model.

## Add live controls

Derive `StoryControls` on an explicit story and keep `Story::new_view` typed as
`Entity<Self>`:

```rust
#[derive(gpui_storybook::StoryControls)]
struct ButtonStory {
    #[storybook(control(category = "State"))]
    disabled: bool,
    #[storybook(control(min = 0.0, max = 32.0, step = 1.0))]
    padding: f32,
    focus_handle: gpui::FocusHandle,
}
```

On a `ComponentStory`, place the same attributes on component fields. The
generated wrapper obtains reset defaults from `example = ...`, reconstructs the
example during rendering, and overlays current values.

Supported inferred types are `bool`, `i8` through `i64`, `isize`, `u8` through
`u32`, `usize`, `f32`, `f64`, `String`, `SharedString`, and `Hsla`. Enum-like
types use string `options` and implement `Display` plus `FromStr`. Leave
collections and application-specific types unmarked; only fields with
`#[storybook(control...)]` are registered. An explicitly controlled unsupported
field is a compile error.

## Organize stories

Both registration styles accept string sections or enum variants. Use a
`#[repr(usize)]` enum when stable section ordering matters; string sections
sort alphabetically.

Use `#[gpui_storybook::story_init]` for application setup that runs once after
the core runtime is installed and before preference readiness begins.

## Declare story-owned scenarios

Implement `Story::scenarios()` on an explicit story when one repeatable flow
belongs with that story. Compose `StoryScenario` from initial typed controls and
presentation, ordered `StoryScenarioStep` values, exact semantic
postconditions, and an optional capture. Every run recreates the concrete story
entity and rebinds its control target and focus before dispatch.

For a component story, return the same `Vec<StoryScenario>` from an associated
function and pass it to the derive:

```rust
#[derive(gpui::IntoElement, gpui_storybook::ComponentStory)]
#[storybook(
    example = WelcomeCard::example(),
    scenarios = WelcomeCard::scenarios(),
)]
struct WelcomeCard {
    // ...
}
```

Keep scenario keys unique within a story and step names diagnostic. Prefer
semantic targets and exact `storybook_value` postconditions to coordinate
assertions. Treat runs as destructive and non-idempotent: report partial
progress, then start a new fresh run after fixing the cause; never resume or
automatically retry input. The standard `gpui_storybook::init` path installs
the live in-process runner used by the Scenarios tab. Enable `mcp` when remote
tools or capture need to share that controller. In the workbench, **Run fresh**
executes from a recreated story; the Scenarios toolbar keeps **Reset** visible
while rows scroll so users can recreate constructor defaults and clear the last
result without executing a scenario.

## Export static registration documentation

Use `static_story_catalog()` or `static_story_catalog_json_pretty()` in a
tooling binary that links the story-bearing crate. Registration macros capture
story Rustdocs, stable identity and source provenance, and static marked-control
shape without constructing a story or opening a window. Output ordering is
deterministic. Use the live runtime catalog and control reads for localized copy
or constructor-derived default values.

## Preserve route identity

Macro-generated base keys have this form:

```text
{cargo-package-name}-{registered-type-name}
```

Explicit stories use the story struct name. Component stories use the component
type name. Display title and description changes do not change the key.

Use `#[derive(gpui_storybook::Substory)]` for stable sections within a story:

```rust
#[derive(gpui_storybook::Substory)]
enum ButtonSubstory {
    NormalButton,
    #[substory(title = "Button with Icon")]
    ButtonWithIcon,
    #[substory(key = "progress", title = "With Progress")]
    WithProgress,
}
```

Variant names produce kebab-case keys. `title` changes display text only;
`key` changes the route segment. Pass the variant to `section(...)` or
`StorySectionBase::new(...)`.

## Diagnose missing stories

Check that:

- the binary links the story-bearing library and module;
- the active `allow` includes the crate group or section;
- `disable_story` does not contain the registered type name;
- generated and manual registrations do not reuse a key;
- the route came from `storybook_list_stories` or registration logs rather
  than a display label.
