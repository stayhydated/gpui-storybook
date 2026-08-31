mod actions;
mod custom_state;
mod density;
mod groups;
mod loading_disabled;
mod variants;

use std::{fmt, str::FromStr};

use gpui::{
    Action, App, AppContext as _, ClickEvent, Context, Entity, Focusable, InteractiveElement as _,
    IntoElement, ParentElement as _, Render, Styled as _, Window, px,
};
use gpui_component::v_flex;
use serde::Deserialize;

#[derive(Action, Clone, Deserialize, Eq, PartialEq)]
#[action(namespace = button_story, no_json)]
enum ButtonAction {
    Disabled,
    Loading,
    Selected,
    Compact,
}

enum ButtonDensity {
    Comfortable,
    Compact,
}

impl fmt::Display for ButtonDensity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Comfortable => "Comfortable",
            Self::Compact => "Compact",
        })
    }
}

impl FromStr for ButtonDensity {
    type Err = ();

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "Comfortable" => Ok(Self::Comfortable),
            "Compact" => Ok(Self::Compact),
            _ => Err(()),
        }
    }
}

#[derive(gpui_storybook::Substory)]
enum ButtonSubstory {
    NormalButton,
    #[substory(title = "Button with Icon")]
    ButtonWithIcon,
    #[substory(title = "With Progress")]
    WithProgress,
}

#[derive(Clone, Copy)]
struct ButtonControlState {
    disabled: bool,
    loading: bool,
    selected: bool,
    compact_control: bool,
    compact: bool,
    toggle_multiple: bool,
}

#[derive(gpui_storybook::StoryControls)]
#[gpui_storybook::story(crate::StorySection::Buttons)]
pub struct ButtonStory {
    focus_handle: gpui::FocusHandle,
    #[storybook(control(category = "State", description = "Disable every button example"))]
    disabled: bool,
    #[storybook(control(category = "State"))]
    loading: bool,
    #[storybook(control(category = "State"))]
    selected: bool,
    #[storybook(control(category = "Layout"))]
    compact: bool,
    #[storybook(control(category = "Layout", options = ["Comfortable", "Compact"]))]
    density: ButtonDensity,
    toggle_multiple: bool,
    #[storybook(control(
        min = 0.0,
        max = 32.0,
        step = 1.0,
        category = "Layout",
        description = "Outer padding around the story examples"
    ))]
    padding: f32,
}

impl ButtonStory {
    pub fn view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| Self {
            focus_handle: cx.focus_handle(),
            disabled: false,
            loading: false,
            selected: false,
            compact: false,
            density: ButtonDensity::Comfortable,
            toggle_multiple: false,
            padding: 0.0,
        })
    }

    fn control_state(&self) -> ButtonControlState {
        ButtonControlState {
            disabled: self.disabled,
            loading: self.loading,
            selected: self.selected,
            compact_control: self.compact,
            compact: self.compact || matches!(self.density, ButtonDensity::Compact),
            toggle_multiple: self.toggle_multiple,
        }
    }

    fn on_click(ev: &ClickEvent, _: &mut Window, _: &mut App) {
        tracing::debug!(event = ?ev, "story button clicked");
    }

    fn on_hover(hovered: &bool, _: &mut Window, _: &mut App) {
        tracing::debug!(hovered, "story button hover changed");
    }
}

impl gpui_storybook::Story for ButtonStory {
    fn title(_: &App) -> String {
        "Button".into()
    }

    fn new_view(window: &mut Window, cx: &mut App) -> Entity<Self> {
        Self::view(window, cx)
    }

    fn action_scope_focus_handle(&self, _: &App) -> Option<gpui::FocusHandle> {
        Some(self.focus_handle.clone())
    }
}

impl Focusable for ButtonStory {
    fn focus_handle(&self, _: &gpui::App) -> gpui::FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for ButtonStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.control_state();

        v_flex()
            .p(px(self.padding))
            .track_focus(&self.focus_handle)
            .on_action(
                cx.listener(|this, action: &ButtonAction, _, _| match action {
                    ButtonAction::Disabled => this.disabled = !this.disabled,
                    ButtonAction::Loading => this.loading = !this.loading,
                    ButtonAction::Selected => this.selected = !this.selected,
                    ButtonAction::Compact => this.compact = !this.compact,
                }),
            )
            .gap_6()
            .child(loading_disabled::controls(state, cx))
            .child(variants::normal(state, cx))
            .child(actions::with_icon(state))
            .child(loading_disabled::with_progress(cx))
            .child(variants::outline(state))
            .child(variants::dropdown_caret(state))
            .child(density::small(state))
            .child(density::xsmall(state))
            .child(groups::horizontal(state))
            .child(groups::vertical(state))
            .child(groups::toggle(state, cx))
            .child(density::icon_buttons(state))
            .child(density::small_icon_buttons(state))
            .child(custom_state::custom(state, cx))
    }
}
