# Write stories

Stories register at compile time and become visible when their crate is linked
into the Storybook binary. Choose the registration style based on who should own
preview state; both styles participate in the same grouping, filtering, and
automation system.

## Register a stateful story

Use `#[story]` when the preview needs a GPUI entity, focus handle, actions, or
custom wrapper UI:

```rust
use gpui::{
    App, AppContext as _, Context, Entity, FocusHandle, Focusable, IntoElement,
    ParentElement as _, Render, Window, div,
};

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
        div().child("Button preview")
    }
}

impl gpui_storybook::Story for ButtonStory {
    fn title(_: &App) -> String {
        "Button".into()
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<impl Render + Focusable> {
        Self::view(window, cx)
    }
}
```

`Story::title` supplies the visible label. Implement `description` when the
gallery should show supporting text.

## Register a component directly

Use `ComponentStory` when Storybook can construct the component and provide
the focusable wrapper:

```rust
use gpui::{App, IntoElement, RenderOnce, SharedString, Window};

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
`cx: &gpui::App` in scope, so metadata can call
`gpui_storybook::localize_message(cx, ...)`.

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

#[gpui_storybook::story(crate::StorySection::Components)]
pub struct ButtonStory;
```

Use the same expression with the derive form:

```rust
#[storybook(section = crate::StorySection::Patterns)]
```

A crate-level `group` from `storybook.toml` appears outside the story's
section in navigation. See [Configure Storybook](configuration.md).

## Run one-time setup

Register application setup that must run after the core runtime is installed
but before preference loading begins:

```rust
#[gpui_storybook::story_init]
fn register_icons(cx: &mut gpui::App) {
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
