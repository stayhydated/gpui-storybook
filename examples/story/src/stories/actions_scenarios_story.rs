use gpui_kit::component::{
    ActiveTheme as _, Disableable as _, Sizable as _, StyledExt as _,
    button::{Button, ButtonVariants as _},
    group_box::{GroupBox, GroupBoxVariants as _},
    h_flex, v_flex,
};
use gpui_kit::{
    Action as _, App, AppContext as _, Context, Entity, Focusable, InteractiveElement as _,
    IntoElement, KeyBinding, ParentElement as _, Render, Styled as _, Window, div,
};
use gpui_storybook::{
    StoryInteractionPostcondition, StoryInteractionStep, StoryScenario, StoryScenarioStep,
    StorybookElementExt as _,
};
use serde::Serialize;

const ACTION_CONTEXT: &str = "ActionsAndScenariosStory";

/// Increases the story counter unless updates are paused.
#[derive(gpui_kit::Action, Clone, Debug, Default, Eq, PartialEq)]
#[action(namespace = actions_scenarios_story)]
pub struct IncreaseCount;

/// Pauses or resumes counter updates.
#[derive(gpui_kit::Action, Clone, Debug, Default, Eq, PartialEq)]
#[action(namespace = actions_scenarios_story)]
pub struct TogglePaused;

/// Restores the counter and pause state without clearing dispatch history.
#[derive(gpui_kit::Action, Clone, Debug, Default, Eq, PartialEq)]
#[action(namespace = actions_scenarios_story)]
pub struct ResetCount;

#[gpui_storybook::story_init]
fn init_actions_scenarios_story(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("ctrl-shift-up", IncreaseCount, Some(ACTION_CONTEXT)),
        KeyBinding::new("ctrl-shift-p", TogglePaused, Some(ACTION_CONTEXT)),
        KeyBinding::new("ctrl-shift-r", ResetCount, Some(ACTION_CONTEXT)),
    ]);
}

#[derive(Debug, Eq, PartialEq, Serialize)]
struct ActionScenarioState {
    count: u32,
    paused: bool,
    dispatch_count: u32,
    last_action: &'static str,
}

impl Default for ActionScenarioState {
    fn default() -> Self {
        Self {
            count: 0,
            paused: false,
            dispatch_count: 0,
            last_action: "Ready",
        }
    }
}

impl ActionScenarioState {
    fn increase_count(&mut self) {
        self.dispatch_count = self.dispatch_count.saturating_add(1);
        if self.paused {
            self.last_action = "Increase ignored while paused";
        } else {
            self.count = self.count.saturating_add(1);
            self.last_action = "Increased count";
        }
    }

    fn toggle_paused(&mut self) {
        self.dispatch_count = self.dispatch_count.saturating_add(1);
        self.paused = !self.paused;
        self.last_action = if self.paused { "Paused" } else { "Resumed" };
    }

    fn reset_count(&mut self) {
        self.dispatch_count = self.dispatch_count.saturating_add(1);
        self.count = 0;
        self.paused = false;
        self.last_action = "Reset count";
    }
}

/// Demonstrates one command model shared by visible controls, key bindings,
/// the Actions workbench tab, and repeatable story scenarios.
#[derive(gpui_storybook::StoryControls)]
#[gpui_storybook::story(crate::StorySection::Automation)]
pub struct ActionsAndScenariosStory {
    focus_handle: gpui_kit::FocusHandle,
    state: ActionScenarioState,
}

impl ActionsAndScenariosStory {
    fn new(cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            state: ActionScenarioState::default(),
        }
    }

    fn increase_count(&mut self, _: &IncreaseCount, _: &mut Window, cx: &mut Context<Self>) {
        self.state.increase_count();
        cx.notify();
    }

    fn toggle_paused(&mut self, _: &TogglePaused, _: &mut Window, cx: &mut Context<Self>) {
        self.state.toggle_paused();
        cx.notify();
    }

    fn reset_count(&mut self, _: &ResetCount, _: &mut Window, cx: &mut Context<Self>) {
        self.state.reset_count();
        cx.notify();
    }
}

impl gpui_storybook::Story for ActionsAndScenariosStory {
    fn title(_: &App) -> String {
        "Actions and scenarios".to_owned()
    }

    fn description(_: &App) -> String {
        "Dispatch the same commands from the canvas, keyboard, Actions tab, or reusable scenarios."
            .to_owned()
    }

    fn new_view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(Self::new)
    }

    fn action_scope_focus_handle(&self, _: &App) -> Option<gpui_kit::FocusHandle> {
        Some(self.focus_handle.clone())
    }

    fn scenarios() -> Vec<StoryScenario> {
        vec![
            StoryScenario::new("count-and-pause", "Count and pause")
                .description("Increase three times, then pause further updates.")
                .step(StoryScenarioStep::new(
                    "Increase to one",
                    StoryInteractionStep::DispatchAction {
                        name: IncreaseCount.name().to_owned(),
                        args: None,
                    },
                ))
                .step(StoryScenarioStep::new(
                    "Increase to two",
                    StoryInteractionStep::DispatchAction {
                        name: IncreaseCount.name().to_owned(),
                        args: None,
                    },
                ))
                .step(StoryScenarioStep::new(
                    "Increase to three",
                    StoryInteractionStep::DispatchAction {
                        name: IncreaseCount.name().to_owned(),
                        args: None,
                    },
                ))
                .step(StoryScenarioStep::new(
                    "Pause updates",
                    StoryInteractionStep::DispatchAction {
                        name: TogglePaused.name().to_owned(),
                        args: None,
                    },
                ))
                .postcondition(
                    StoryInteractionPostcondition::new(
                        "actions-scenarios-state",
                        serde_json::json!(3),
                    )
                        .json_pointer("/count"),
                )
                .postcondition(
                    StoryInteractionPostcondition::new(
                        "actions-scenarios-state",
                        serde_json::json!(true),
                    )
                    .json_pointer("/paused"),
                )
                .postcondition(
                    StoryInteractionPostcondition::new(
                        "actions-scenarios-state",
                        serde_json::json!(4),
                    )
                        .json_pointer("/dispatch_count"),
                ),
            StoryScenario::new("paused-command", "Paused command")
                .description("Pause updates, then show that the increase command is handled without changing the counter.")
                .step(StoryScenarioStep::new(
                    "Pause updates",
                    StoryInteractionStep::DispatchAction {
                        name: TogglePaused.name().to_owned(),
                        args: None,
                    },
                ))
                .step(StoryScenarioStep::new(
                    "Attempt an increase",
                    StoryInteractionStep::DispatchAction {
                        name: IncreaseCount.name().to_owned(),
                        args: None,
                    },
                ))
                .postcondition(
                    StoryInteractionPostcondition::new(
                        "actions-scenarios-state",
                        serde_json::json!(0),
                    )
                        .json_pointer("/count"),
                )
                .postcondition(
                    StoryInteractionPostcondition::new(
                        "actions-scenarios-state",
                        serde_json::json!(true),
                    )
                    .json_pointer("/paused"),
                )
                .postcondition(
                    StoryInteractionPostcondition::new(
                        "actions-scenarios-state",
                        serde_json::json!("Increase ignored while paused"),
                    )
                    .json_pointer("/last_action"),
                ),
        ]
    }
}

impl Focusable for ActionsAndScenariosStory {
    fn focus_handle(&self, _: &App) -> gpui_kit::FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for ActionsAndScenariosStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let increase_focus = self.focus_handle.clone();
        let toggle_focus = self.focus_handle.clone();
        let reset_focus = self.focus_handle.clone();
        let status_color = if self.state.paused {
            cx.theme().warning
        } else {
            cx.theme().success
        };

        v_flex()
            .id("actions-scenarios-story")
            .track_focus(&self.focus_handle)
            .key_context(ACTION_CONTEXT)
            .on_action(cx.listener(Self::increase_count))
            .on_action(cx.listener(Self::toggle_paused))
            .on_action(cx.listener(Self::reset_count))
            .w_full()
            .gap_6()
            .child(
                v_flex()
                    .gap_1()
                    .child(div().text_lg().font_semibold().child("One command model"))
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child(
                                "Use the buttons below, the contextual shortcuts, the Actions tab, or the Scenarios tab.",
                            ),
                    ),
            )
            .child(
                GroupBox::new()
                    .outline()
                    .title("Live action state")
                    .child(
                        v_flex()
                            .id("actions-scenarios-state")
                            .gap_4()
                            .child(
                                h_flex()
                                    .gap_6()
                                    .flex_wrap()
                                    .child(
                                        v_flex()
                                            .gap_1()
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(cx.theme().muted_foreground)
                                                    .child("Count"),
                                            )
                                            .child(
                                                div()
                                                    .text_lg()
                                                    .font_semibold()
                                                    .child(self.state.count.to_string()),
                                            ),
                                    )
                                    .child(
                                        v_flex()
                                            .gap_1()
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(cx.theme().muted_foreground)
                                                    .child("Updates"),
                                            )
                                            .child(
                                                div()
                                                    .text_sm()
                                                    .font_semibold()
                                                    .text_color(status_color)
                                                    .child(if self.state.paused {
                                                        "Paused"
                                                    } else {
                                                        "Active"
                                                    }),
                                            ),
                                    )
                                    .child(
                                        v_flex()
                                            .gap_1()
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(cx.theme().muted_foreground)
                                                    .child("Dispatches"),
                                            )
                                            .child(
                                                div()
                                                    .text_sm()
                                                    .font_semibold()
                                                    .child(self.state.dispatch_count.to_string()),
                                            ),
                                    ),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .child(format!("Last action: {}", self.state.last_action)),
                            )
                            .storybook_value(&self.state),
                    ),
            )
            .child(
                h_flex()
                    .gap_2()
                    .flex_wrap()
                    .child(
                        Button::new("increase-count")
                            .label("Increase count")
                            .small()
                            .disabled(self.state.paused)
                            .on_click(move |_, window, cx| {
                                increase_focus.dispatch_action(&IncreaseCount, window, cx);
                            }),
                    )
                    .child(
                        Button::new("toggle-paused")
                            .label(if self.state.paused { "Resume" } else { "Pause" })
                            .small()
                            .outline()
                            .on_click(move |_, window, cx| {
                                toggle_focus.dispatch_action(&TogglePaused, window, cx);
                            }),
                    )
                    .child(
                        Button::new("reset-count")
                            .label("Reset count")
                            .small()
                            .ghost()
                            .on_click(move |_, window, cx| {
                                reset_focus.dispatch_action(&ResetCount, window, cx);
                            }),
                    ),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(
                        "Shortcuts: Ctrl+Shift+Up increase, Ctrl+Shift+P pause or resume, Ctrl+Shift+R reset.",
                    ),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui_storybook::Story as _;

    #[test]
    fn action_state_preserves_pause_and_reset_policy() {
        let mut state = ActionScenarioState::default();
        state.increase_count();
        state.toggle_paused();
        state.increase_count();

        assert_eq!(state.count, 1);
        assert!(state.paused);
        assert_eq!(state.dispatch_count, 3);
        assert_eq!(state.last_action, "Increase ignored while paused");

        state.reset_count();
        assert_eq!(state.count, 0);
        assert!(!state.paused);
        assert_eq!(state.dispatch_count, 4);
        assert_eq!(state.last_action, "Reset count");
    }

    #[test]
    fn scenarios_dispatch_only_the_story_actions() {
        let scenarios = ActionsAndScenariosStory::scenarios();
        assert_eq!(
            scenarios
                .iter()
                .map(|scenario| scenario.key.as_str())
                .collect::<Vec<_>>(),
            ["count-and-pause", "paused-command"]
        );

        let action_names = scenarios
            .iter()
            .flat_map(|scenario| &scenario.steps)
            .map(|step| match &step.step {
                StoryInteractionStep::DispatchAction { name, args } => {
                    assert_eq!(args, &None);
                    name.as_str()
                },
                step => panic!("expected an action step, got {step:?}"),
            })
            .collect::<Vec<_>>();
        assert_eq!(
            action_names,
            [
                IncreaseCount.name(),
                IncreaseCount.name(),
                IncreaseCount.name(),
                TogglePaused.name(),
                TogglePaused.name(),
                IncreaseCount.name(),
            ]
        );
    }

    #[gpui_kit::test]
    fn showcased_actions_are_default_buildable(cx: &mut App) {
        for action in [
            &IncreaseCount as &dyn gpui_kit::Action,
            &TogglePaused,
            &ResetCount,
        ] {
            let built = cx
                .build_action(action.name(), None)
                .expect("showcase action should build without arguments");
            assert_eq!(built.name(), action.name());
        }
    }
}
