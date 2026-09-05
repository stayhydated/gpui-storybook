use super::*;
use gpui_kit::{ScrollDelta, ScrollWheelEvent, TestAppContext, VisualTestContext, point};
use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Action, Clone, Default, Eq, PartialEq)]
#[action(namespace = storybook_action_scope_test)]
struct ShellAction;

#[derive(Action, Clone, Default, Eq, PartialEq)]
#[action(namespace = storybook_action_scope_test)]
struct StoryAction;

#[derive(Action, Clone, Default, Eq, PartialEq)]
#[action(namespace = storybook_action_scope_test)]
struct NestedInputAction;

gpui_kit::actions!(
    workbench_action_reset_test,
    [
        #[derive(Eq)]
        ActionReset00,
        #[derive(Eq)]
        ActionReset01,
        #[derive(Eq)]
        ActionReset02,
        #[derive(Eq)]
        ActionReset03,
        #[derive(Eq)]
        ActionReset04,
        #[derive(Eq)]
        ActionReset05,
        #[derive(Eq)]
        ActionReset06,
        #[derive(Eq)]
        ActionReset07,
    ]
);

struct ActionScopeFixture {
    shell_focus: FocusHandle,
    story_focus: FocusHandle,
    nested_input_focus: FocusHandle,
}

impl ActionScopeFixture {
    fn new(cx: &mut Context<Self>) -> Self {
        Self {
            shell_focus: cx.focus_handle(),
            story_focus: cx.focus_handle(),
            nested_input_focus: cx.focus_handle(),
        }
    }

    fn ignore_shell_action(&mut self, _: &ShellAction, _: &mut Window, _: &mut Context<Self>) {}

    fn ignore_story_action(&mut self, _: &StoryAction, _: &mut Window, _: &mut Context<Self>) {}

    fn ignore_nested_input_action(
        &mut self,
        _: &NestedInputAction,
        _: &mut Window,
        _: &mut Context<Self>,
    ) {
    }
}

impl Render for ActionScopeFixture {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .on_action(cx.listener(Self::ignore_shell_action))
            .child(div().track_focus(&self.shell_focus))
            .child(
                div()
                    .track_focus(&self.story_focus)
                    .on_action(cx.listener(Self::ignore_story_action))
                    .child(
                        div()
                            .track_focus(&self.nested_input_focus)
                            .on_action(cx.listener(Self::ignore_nested_input_action)),
                    ),
            )
    }
}

struct ScopedStory {
    interaction_focus: FocusHandle,
    action_scope_focus: FocusHandle,
}

impl crate::controls::StoryControls for ScopedStory {}

impl Focusable for ScopedStory {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.interaction_focus.clone()
    }
}

impl crate::story::Story for ScopedStory {
    fn title(_: &App) -> String {
        "Scoped story".to_owned()
    }

    fn new_view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| Self {
            interaction_focus: cx.focus_handle(),
            action_scope_focus: cx.focus_handle(),
        })
    }

    fn action_scope_focus_handle(&self, _: &App) -> Option<FocusHandle> {
        Some(self.action_scope_focus.clone())
    }
}

impl ScopedStory {
    fn ignore_story_action(&mut self, _: &StoryAction, _: &mut Window, _: &mut Context<Self>) {}

    fn ignore_nested_input_action(
        &mut self,
        _: &NestedInputAction,
        _: &mut Window,
        _: &mut Context<Self>,
    ) {
    }
}

impl Render for ScopedStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .track_focus(&self.action_scope_focus)
            .on_action(cx.listener(Self::ignore_story_action))
            .child(
                div()
                    .track_focus(&self.interaction_focus)
                    .on_action(cx.listener(Self::ignore_nested_input_action)),
            )
    }
}

static ACTION_RESET_STORY_CREATIONS: AtomicUsize = AtomicUsize::new(0);

struct ActionResetStory {
    focus_handle: FocusHandle,
}

impl crate::controls::StoryControls for ActionResetStory {}

impl Focusable for ActionResetStory {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl crate::story::Story for ActionResetStory {
    fn title(_: &App) -> String {
        "Action reset story".to_owned()
    }

    fn new_view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        ACTION_RESET_STORY_CREATIONS.fetch_add(1, Ordering::SeqCst);
        cx.new(|cx| Self {
            focus_handle: cx.focus_handle(),
        })
    }

    fn action_scope_focus_handle(&self, _: &App) -> Option<FocusHandle> {
        Some(self.focus_handle.clone())
    }
}

impl ActionResetStory {
    fn ignore_action<A: Action>(&mut self, _: &A, _: &mut Window, _: &mut Context<Self>) {}
}

impl Render for ActionResetStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::ignore_action::<ActionReset00>))
            .on_action(cx.listener(Self::ignore_action::<ActionReset01>))
            .on_action(cx.listener(Self::ignore_action::<ActionReset02>))
            .on_action(cx.listener(Self::ignore_action::<ActionReset03>))
            .on_action(cx.listener(Self::ignore_action::<ActionReset04>))
            .on_action(cx.listener(Self::ignore_action::<ActionReset05>))
            .on_action(cx.listener(Self::ignore_action::<ActionReset06>))
            .on_action(cx.listener(Self::ignore_action::<ActionReset07>))
    }
}

struct ActionResetWorkbenchFixture {
    story: Entity<StoryContainer>,
    workbench: Entity<StoryWorkbench>,
}

impl ActionResetWorkbenchFixture {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let story = StoryContainer::panel::<ActionResetStory>(window, cx);
        let state = cx.new(|cx| WorkbenchState::new(Some(story.clone()), cx));
        let workbench = cx.new(|cx| StoryWorkbench::new(state, WorkbenchTab::Controls, window, cx));
        Self { story, workbench }
    }
}

impl Render for ActionResetWorkbenchFixture {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        h_flex()
            .size_full()
            .child(div().flex_1().min_w_0().h_full().child(self.story.clone()))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .child(self.workbench.clone()),
            )
    }
}

static SCENARIO_RESET_STORY_CREATIONS: AtomicUsize = AtomicUsize::new(0);

struct ScenarioResetStory {
    focus_handle: FocusHandle,
}

impl crate::controls::StoryControls for ScenarioResetStory {}

impl Focusable for ScenarioResetStory {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl crate::story::Story for ScenarioResetStory {
    fn title(_: &App) -> String {
        "Scenario reset story".to_owned()
    }

    fn new_view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        SCENARIO_RESET_STORY_CREATIONS.fetch_add(1, Ordering::SeqCst);
        cx.new(|cx| Self {
            focus_handle: cx.focus_handle(),
        })
    }

    fn scenarios() -> Vec<StoryScenario> {
        let mut scenarios = vec![StoryScenario::new("restore-defaults", "Restore defaults")];
        scenarios.extend(
            (0..20).map(|ix| StoryScenario::new(format!("example-{ix}"), format!("Example {ix}"))),
        );
        scenarios
    }
}

impl Render for ScenarioResetStory {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

struct RecreatedControlStory {
    focus_handle: FocusHandle,
    values: BTreeMap<String, ControlValue>,
}

impl crate::controls::StoryControls for RecreatedControlStory {
    fn control_specs(&self) -> Vec<ControlSpec> {
        vec![
            ControlSpec {
                key: "label".to_owned(),
                label: "Label".to_owned(),
                description: String::new(),
                category: String::new(),
                kind: ControlKind::Text,
                default: self.values["label"].clone(),
                bounds: crate::controls::ControlBounds::default(),
                options: Vec::new(),
            },
            ControlSpec {
                key: "count".to_owned(),
                label: "Count".to_owned(),
                description: String::new(),
                category: String::new(),
                kind: ControlKind::Number,
                default: self.values["count"].clone(),
                bounds: crate::controls::ControlBounds::default(),
                options: Vec::new(),
            },
            ControlSpec {
                key: "ratio".to_owned(),
                label: "Ratio".to_owned(),
                description: String::new(),
                category: String::new(),
                kind: ControlKind::Range,
                default: self.values["ratio"].clone(),
                bounds: crate::controls::ControlBounds {
                    min: Some(0.0),
                    max: Some(1.0),
                    step: Some(0.1),
                },
                options: Vec::new(),
            },
            ControlSpec {
                key: "tint".to_owned(),
                label: "Tint".to_owned(),
                description: String::new(),
                category: String::new(),
                kind: ControlKind::ColorPicker,
                default: self.values["tint"].clone(),
                bounds: crate::controls::ControlBounds::default(),
                options: Vec::new(),
            },
        ]
    }

    fn control_value(&self, key: &str) -> Result<ControlValue, crate::controls::ControlError> {
        self.values
            .get(key)
            .cloned()
            .ok_or_else(|| crate::controls::ControlError::UnknownControl {
                key: key.to_owned(),
            })
    }

    fn set_control_value(
        &mut self,
        key: &str,
        value: ControlValue,
    ) -> Result<(), crate::controls::ControlError> {
        let Some(current) = self.values.get_mut(key) else {
            return Err(crate::controls::ControlError::UnknownControl {
                key: key.to_owned(),
            });
        };
        *current = value;
        Ok(())
    }
}

impl Focusable for RecreatedControlStory {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl crate::story::Story for RecreatedControlStory {
    fn title(_: &App) -> String {
        "Recreated control story".to_owned()
    }

    fn new_view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| Self {
            focus_handle: cx.focus_handle(),
            values: BTreeMap::from([
                ("label".to_owned(), ControlValue::Text("default".to_owned())),
                ("count".to_owned(), ControlValue::Integer(1)),
                ("ratio".to_owned(), ControlValue::Float(0.25)),
                (
                    "tint".to_owned(),
                    ControlValue::Color(crate::controls::ControlColor {
                        h: 0.0,
                        s: 0.5,
                        l: 0.5,
                        a: 1.0,
                    }),
                ),
            ]),
        })
    }
}

impl Render for RecreatedControlStory {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

struct RecreatedControlWorkbenchFixture {
    story: Entity<StoryContainer>,
    workbench: Entity<StoryWorkbench>,
}

impl RecreatedControlWorkbenchFixture {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let story = StoryContainer::panel::<RecreatedControlStory>(window, cx);
        let state = cx.new(|cx| WorkbenchState::new(Some(story.clone()), cx));
        let workbench = cx.new(|cx| StoryWorkbench::new(state, WorkbenchTab::Controls, window, cx));
        Self { story, workbench }
    }
}

impl Render for RecreatedControlWorkbenchFixture {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        h_flex()
            .size_full()
            .child(div().flex_1().min_w_0().h_full().child(self.story.clone()))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .child(self.workbench.clone()),
            )
    }
}

struct StoryContainerActionFixture {
    shell_focus: FocusHandle,
    story: Entity<StoryContainer>,
}

impl StoryContainerActionFixture {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            shell_focus: cx.focus_handle(),
            story: StoryContainer::panel::<ScopedStory>(window, cx),
        }
    }

    fn ignore_shell_action(&mut self, _: &ShellAction, _: &mut Window, _: &mut Context<Self>) {}
}

impl Render for StoryContainerActionFixture {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .on_action(cx.listener(Self::ignore_shell_action))
            .child(div().track_focus(&self.shell_focus))
            .child(self.story.clone())
    }
}

mod actions;
mod controls;
mod scenarios;
mod state;
mod theme;
