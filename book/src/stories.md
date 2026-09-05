# Write stories

Stories register at compile time and become visible when their crate is linked
into the Storybook binary. Choose the registration style based on who should own
preview state; both styles participate in the same grouping, filtering, and
automation system.

## Register a stateful story

Use `#[story]` when the preview needs a GPUI entity, focus handle, actions, or
custom wrapper UI:

```rust
use gpui_kit::{
    App, AppContext as _, Context, Entity, FocusHandle, Focusable,
    InteractiveElement as _, IntoElement, ParentElement as _, Render, Window, div,
};

#[derive(gpui_storybook::StoryControls)]
#[gpui_storybook::story("Components")]
pub struct ButtonStory {
    focus_handle: FocusHandle,
}

impl ButtonStory {
    fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| Self {
            focus_handle: cx.focus_handle(),
        })
    }
}

impl Focusable for ButtonStory {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for ButtonStory {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .track_focus(&self.focus_handle)
            .child("Button preview")
    }
}

impl gpui_storybook::Story for ButtonStory {
    fn title(_: &App) -> String {
        "Button".into()
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<Self> {
        Self::view(window, cx)
    }

    fn action_scope_focus_handle(&self, _: &App) -> Option<FocusHandle> {
        Some(self.focus_handle.clone())
    }
}
```

`Story::title` supplies the visible label. Implement `description` when the
gallery should show supporting text. `action_scope_focus_handle` is an explicit
opt-in for the Actions workbench. Track it on the element that installs the
story's action handlers. If the primary `Focusable` handle belongs to an input
or another nested control, store and track a separate root handle for the
action scope.

## Register a component directly

Use `ComponentStory` when Storybook can construct the component and provide
the focusable wrapper:

```rust
use gpui_kit::{App, IntoElement, RenderOnce, SharedString, Window};

#[derive(IntoElement, gpui_storybook::ComponentStory)]
#[storybook(
    title = "Welcome Card",
    description = "A preview built from representative data",
    section = crate::StorySection::Intro,
    example = WelcomeCard::example(),
)]
pub struct WelcomeCard {
    title: SharedString,
}

impl WelcomeCard {
    fn example() -> Self {
        Self {
            title: "Component registration".into(),
        }
    }
}

impl RenderOnce for WelcomeCard {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        self.title
    }
}
```

`ComponentStory` accepts a non-generic struct. If `example = ...` is omitted,
the generated wrapper constructs the component with `Default::default()`.
`title` and `description` accept expressions evaluated with
`cx: &gpui_kit::App` in scope, so metadata can call
`gpui_storybook::localize_message(cx, ...)`.

## Expose live controls

For an explicit story, derive `StoryControls` and mark individual fields:

```rust
#[derive(gpui_storybook::StoryControls)]
struct ButtonStory {
    #[storybook(control)]
    disabled: bool,
    #[storybook(control(min = 0.0, max = 32.0, step = 1.0))]
    padding: f32,
    focus_handle: gpui_kit::FocusHandle,
}
```

For a `ComponentStory`, put `#[storybook(control...)]` on component fields.
Fields without the attribute are not registered as controls.
The generated wrapper captures their defaults from `example = ...` and
overlays live values whenever it renders. See [Use the workbench](workbench.md)
for supported field types, reset behavior, theme editing, and preview tools.

## Order stories with sections

Both registration styles accept a string or enum-variant section. String
sections sort alphabetically. Enum discriminants provide stable numeric order:

```rust
#[derive(Clone, Copy)]
#[repr(usize)]
pub enum StorySection {
    Basics = 1,
    Components = 2,
    Patterns = 3,
}

#[derive(gpui_storybook::StoryControls)]
#[gpui_storybook::story(crate::StorySection::Components)]
pub struct ButtonStory;
```

Use the same expression with the derive form:

```rust
#[storybook(section = crate::StorySection::Patterns)]
```

A crate-level `group` from `storybook.toml` appears outside the story's
section in navigation. See [Configure Storybook](configuration.md).

Stories with the same visible title, group, and section share one navigation
entry. Give each concrete story a distinct, concise description; the workbench
uses it as the **Variant** select label. Gallery mode renders one selected
member, while dock mode keeps selected members in independent tabs.

## Run one-time setup

Register application setup that must run after the core runtime is installed
but before preference loading begins:

```rust
#[gpui_storybook::story_init]
fn register_icons(cx: &mut gpui_kit::App) {
    // Register application assets or other GPUI globals.
}
```

Initialization functions must be linked into the Storybook binary like story
registrations.

## Use stable story routes

Each macro-generated story has an automation key:

```text
{cargo-package-name}-{registered-type-name}
```

An explicit story uses its story struct name. A component story uses the
component type name, not its generated wrapper. Display titles, descriptions,
and localization do not change the key.

For example:

```text
my-app-storybook-ButtonStory
```

Use `storybook_list_stories` or startup registration logs as the source of
truth for active keys.

## Declare reusable scenarios

Keep repeatable interaction with the story that owns it. An explicit story
returns `StoryScenario` values from `Story::scenarios()`:

```rust
fn scenarios() -> Vec<gpui_storybook::StoryScenario> {
    use gpui_storybook::{
        StoryInteractionPostcondition, StoryInteractionStep,
        StoryScenario, StoryScenarioStep,
    };

    vec![StoryScenario::new("submit", "Submit the form")
        .step(StoryScenarioStep::new(
            "Click submit",
            StoryInteractionStep::ClickTarget {
                target_key: "submit".into(),
                button: Default::default(),
                click_count: 1,
                modifiers: Default::default(),
            },
        ))
        .postcondition(
            StoryInteractionPostcondition::new("form-state", serde_json::json!("saved"))
                .json_pointer("/status"),
        )]
}
```

A scenario can set initial typed controls and presentation, execute ordered
named steps, assert exact route-local semantic values or JSON Pointers, and
request one final PNG. The Scenarios workbench tab and MCP both use the shared
interaction executor. Every invocation recreates the concrete story entity and
rebinds its controls and focus before applying the scenario, so repeated runs
start from constructor defaults. A partial destructive run is reported and is
never resumed or retried. Standard `gpui_storybook::init` installs the live
runner used by the workbench; on Linux and macOS, the `mcp` feature connects
remote tools and capture support to that controller.

For a component-derived story, expose an expression that returns the same
vector:

```rust
#[derive(gpui_storybook::ComponentStory, gpui_kit::IntoElement)]
#[storybook(
    example = WelcomeCard::example(),
    scenarios = WelcomeCard::scenarios(),
)]
struct WelcomeCard {
    // ...
}
```

Scenario keys must be unique within their story. Titles, descriptions, and
step names are display copy; automation selects the stable story and scenario
keys.

## Export static autodocs

`#[story]` and `#[derive(ComponentStory)]` capture declaration Rustdocs and the
static shape of marked controls in each inventory registration. A tooling binary
can export every linked registration without initializing GPUI or constructing
a story:

```rust
use my_story_crate as _;

fn main() -> Result<(), gpui_storybook::StoryCatalogExportError> {
    println!("{}", gpui_storybook::static_story_catalog_json_pretty()?);
    Ok(())
}
```

Entries are sorted by stable story key and source provenance. JSON includes the
registered name and section, source crate/file/line, Rust documentation, and
control keys, labels, descriptions, categories, editor kinds, numeric bounds,
and choices. Localized titles and descriptions plus constructor-derived control
defaults require a live `App`, so query the runtime catalog and controls when
those values matter.

## Add captureable sections inside a story

Derive `Substory` for stable section routes:

```rust
#[derive(gpui_storybook::Substory)]
enum ButtonSubstory {
    NormalButton,
    #[substory(title = "Button with Icon")]
    ButtonWithIcon,
    #[substory(key = "progress", title = "With Progress")]
    WithProgress,
}

let section = gpui_storybook::section(ButtonSubstory::WithProgress);
```

The default key is the variant name in kebab case. `title` changes only the
visible text; `key` sets the route segment. The example above creates:

```text
my-app-storybook-ButtonStory/progress
```

String titles passed to `section(...)` receive title-derived slugs. For custom
section layout, store a `StorySectionBase` and call its `capture` method
after building the section element.

## Troubleshoot missing stories

Check these conditions in order:

1. The Storybook binary links the crate and module containing the registration.
2. The active configuration allows the story's crate group or section.
3. `disable_story` does not contain the exact registered type name.
4. No duplicate macro-generated story key exists in the same package.
5. Manual registrations do not reuse an existing key.
