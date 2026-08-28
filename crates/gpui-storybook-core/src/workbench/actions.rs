use super::*;

pub(super) fn format_key_binding(binding: &KeyBinding) -> String {
    binding
        .keystrokes()
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(" ")
}

pub(super) fn is_story_scoped_action(
    action: &dyn Action,
    action_scope_focus: &FocusHandle,
    storybook_focus: &FocusHandle,
    window: &Window,
) -> bool {
    // Both handles share the Storybook shell path. The story scope handle is an
    // explicit root scope rather than the story's primary (possibly nested)
    // interaction focus, so child-control actions never enter this set.
    window.is_action_available_in(action, action_scope_focus)
        && !window.is_action_available_in(action, storybook_focus)
}

pub(super) fn story_scoped_actions(
    action_scope_focus: &FocusHandle,
    storybook_focus: &FocusHandle,
    window: &Window,
    cx: &App,
) -> Vec<Box<dyn Action>> {
    let mut actions = cx
        .all_action_names()
        .iter()
        .filter_map(|name| cx.build_action(name, None).ok())
        .filter(|action| {
            is_story_scoped_action(action.as_ref(), action_scope_focus, storybook_focus, window)
        })
        .collect::<Vec<_>>();
    actions.sort_by_key(|action| action.name());
    actions
}

impl StoryWorkbench {
    pub(super) fn render_actions(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let Some(story) = self.state.read(cx).active_story() else {
            return v_flex()
                .id("workbench-actions")
                .p_4()
                .text_color(cx.theme().muted_foreground)
                .child("Select a story")
                .into_any_element();
        };
        let action_scope_focus = {
            let story = story.read(cx);
            story.action_scope_focus_handle()
        };
        let Some(action_scope_focus) = action_scope_focus else {
            return v_flex()
                .id("workbench-actions")
                .p_4()
                .text_color(cx.theme().muted_foreground)
                .child("No action scope")
                .into_any_element();
        };
        let actions = story_scoped_actions(&action_scope_focus, &self.focus_handle, window, cx);
        let actions_empty = actions.is_empty();
        let any_scenario_running =
            matches!(&self.scenario_run, Some(ScenarioRunState::Running { .. }));
        let toolbar = self.render_story_reset_toolbar(
            "reset-action-story",
            "workbench-actions-sticky-header",
            "workbench-actions-reset",
            any_scenario_running,
            cx,
        );
        let documentation = cx.action_documentation();
        let mut schema_generator = schemars::generate::SchemaSettings::draft2020_12()
            .with(|settings| settings.inline_subschemas = true)
            .into_generator();

        let action_rows = actions.into_iter().map(|action| {
            let name = action.name();
            let documentation = documentation.get(name).copied();
            let argument_schema = cx
                .action_schema_by_name(name, &mut schema_generator)
                .flatten()
                .and_then(|schema| serde_json::to_string(&schema).ok());
            let bindings = window
                .bindings_for_action_in(action.as_ref(), &action_scope_focus)
                .into_iter()
                .map(|binding| format_key_binding(&binding))
                .collect::<Vec<_>>();
            let dispatch_focus = action_scope_focus.clone();

            v_flex()
                .id(format!("workbench-action-{name}"))
                .gap_1()
                .py_3()
                .border_b_1()
                .border_color(cx.theme().border)
                .child(
                    h_flex()
                        .justify_between()
                        .gap_2()
                        .child(div().text_sm().child(name))
                        .child(
                            Button::new(format!("dispatch-action-{name}"))
                                .debug_selector(move || format!("dispatch-action-{name}"))
                                .label("Dispatch")
                                .xsmall()
                                .on_click(move |_, window, cx| {
                                    dispatch_focus.dispatch_action(action.as_ref(), window, cx);
                                }),
                        ),
                )
                .when_some(documentation, |this, documentation| {
                    this.child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(documentation),
                    )
                })
                .when_some(argument_schema, |this, schema| {
                    this.child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(format!("Arguments: {schema}")),
                    )
                })
                .when(!bindings.is_empty(), |this| {
                    this.child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(format!("Bindings: {}", bindings.join(", "))),
                    )
                })
        });

        v_flex()
            .id("workbench-actions")
            .size_full()
            .min_h_0()
            .overflow_hidden()
            .child(toolbar)
            .child(
                div()
                    .debug_selector(|| "workbench-actions-items".to_owned())
                    .flex_1()
                    .min_h_0()
                    .overflow_hidden()
                    .child(
                        v_flex()
                            .size_full()
                            .overflow_y_scrollbar()
                            .px_4()
                            .pb_4()
                            .gap_2()
                            .when(actions_empty, |this| {
                                this.child(
                                    div()
                                        .py_3()
                                        .text_color(cx.theme().muted_foreground)
                                        .child("No actions"),
                                )
                            })
                            .children(action_rows),
                    ),
            )
            .into_any_element()
    }
}
