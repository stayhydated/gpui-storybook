use super::*;

impl StoryWorkbench {
    pub(super) fn run_scenario(
        &mut self,
        story_key: String,
        scenario: StoryScenario,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let automation = self.state.read(cx).automation();
        let Some(automation) = automation else {
            self.scenario_run = Some(ScenarioRunState::Finished {
                story_key,
                scenario,
                result: Box::new(Err(StorybookAutomationError::NoLiveHost)),
            });
            cx.notify();
            return;
        };

        let scenario_key = scenario.key.clone();
        self.scenario_run = Some(ScenarioRunState::Running {
            story_key: story_key.clone(),
            scenario,
        });
        cx.notify();

        cx.spawn_in(window, async move |this, cx| {
            let result = automation
                .run_scenario(Some(story_key.clone()), scenario_key)
                .await;
            _ = this.update_in(cx, |this, window, cx| {
                let scenario = match &this.scenario_run {
                    Some(ScenarioRunState::Running { scenario, .. }) => scenario.clone(),
                    Some(ScenarioRunState::Finished { scenario, .. }) => scenario.clone(),
                    None => return,
                };
                this.scenario_run = Some(ScenarioRunState::Finished {
                    story_key,
                    scenario,
                    result: Box::new(result),
                });
                this.rebuild_control_editors(window, cx);
                cx.notify();
            });
        })
        .detach();
    }

    pub(super) fn reset_active_story(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(story) = self.active_story(cx) else {
            return;
        };
        story.update(cx, |story, cx| {
            story.recreate_for_scenario(window, cx);
        });
        self.scenario_run = None;
        self.rebuild_control_editors(window, cx);
        cx.notify();
    }

    pub(super) fn render_story_reset_toolbar(
        &self,
        button_id: &'static str,
        header_selector: &'static str,
        reset_selector: &'static str,
        disabled: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        h_flex()
            .debug_selector(move || header_selector.to_owned())
            .flex_shrink_0()
            .justify_end()
            .px_4()
            .py_2()
            .border_b_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().background)
            .child(
                Button::new(button_id)
                    .debug_selector(move || reset_selector.to_owned())
                    .label("Reset")
                    .xsmall()
                    .ghost()
                    .disabled(disabled)
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.reset_active_story(window, cx);
                    })),
            )
            .into_any_element()
    }

    pub(super) fn scenario_progress(error: &StorybookAutomationError) -> usize {
        match error {
            StorybookAutomationError::HostDisconnected {
                steps_dispatched, ..
            }
            | StorybookAutomationError::InteractionFailed {
                steps_dispatched, ..
            } => *steps_dispatched,
            _ => 0,
        }
    }

    pub(super) fn render_scenarios(
        &self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(story) = self.active_story(cx) else {
            return v_flex()
                .p_4()
                .text_color(cx.theme().muted_foreground)
                .child("Select a story")
                .into_any_element();
        };
        let story = story.read(cx);
        let story_key = story.story_key_label().unwrap_or_default().to_owned();
        let scenarios = story.scenarios().to_vec();

        if scenarios.is_empty() {
            return v_flex()
                .p_4()
                .text_color(cx.theme().muted_foreground)
                .child("No scenarios")
                .into_any_element();
        }

        let any_scenario_running =
            matches!(&self.scenario_run, Some(ScenarioRunState::Running { .. }));
        let toolbar = self.render_story_reset_toolbar(
            "reset-scenario-story",
            "workbench-scenarios-sticky-header",
            "workbench-scenarios-reset",
            any_scenario_running,
            cx,
        );

        let scenario_rows = scenarios.into_iter().map(|scenario| {
            let run = self.scenario_run.as_ref().filter(|run| match run {
                ScenarioRunState::Running {
                    story_key: run_story_key,
                    scenario: run_scenario,
                }
                | ScenarioRunState::Finished {
                    story_key: run_story_key,
                    scenario: run_scenario,
                    ..
                } => run_story_key == &story_key && run_scenario.key == scenario.key,
            });
            let running = matches!(run, Some(ScenarioRunState::Running { .. }));
            let scenario_for_run = scenario.clone();
            let story_key_for_run = story_key.clone();

            let step_rows = scenario.steps.iter().enumerate().map(|(index, step)| {
                let status = match run {
                    Some(ScenarioRunState::Running { .. }) => {
                        if index == 0 {
                            "Running"
                        } else {
                            "Queued"
                        }
                    },
                    Some(ScenarioRunState::Finished { result, .. }) => match result.as_ref() {
                        Ok(result) if index < result.interaction.steps_dispatched => "Passed",
                        Ok(_) => "Not run",
                        Err(error) => {
                            let completed = Self::scenario_progress(error);
                            if index < completed {
                                "Passed"
                            } else if index == completed {
                                "Failed"
                            } else {
                                "Not run"
                            }
                        },
                    },
                    None => "Ready",
                };

                h_flex()
                    .justify_between()
                    .gap_2()
                    .child(
                        div()
                            .text_xs()
                            .child(format!("{}. {}", index + 1, step.name)),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(status),
                    )
            });

            v_flex()
                .id(format!("workbench-scenario-{}", scenario.key))
                .gap_2()
                .py_3()
                .border_b_1()
                .border_color(cx.theme().border)
                .child(
                    h_flex()
                        .justify_between()
                        .gap_2()
                        .child(
                            v_flex()
                                .gap_0p5()
                                .child(div().text_sm().child(scenario.title.clone()))
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(cx.theme().muted_foreground)
                                        .child(scenario.key.clone()),
                                ),
                        )
                        .child(
                            Button::new(format!("run-scenario-{}", scenario.key))
                                .debug_selector({
                                    let scenario_key = scenario.key.clone();
                                    move || format!("run-scenario-{scenario_key}")
                                })
                                .label(if running { "Running…" } else { "Run fresh" })
                                .xsmall()
                                .disabled(any_scenario_running)
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.run_scenario(
                                        story_key_for_run.clone(),
                                        scenario_for_run.clone(),
                                        window,
                                        cx,
                                    );
                                })),
                        ),
                )
                .when(!scenario.description.is_empty(), |this| {
                    this.child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(scenario.description.clone()),
                    )
                })
                .child(v_flex().gap_1().children(step_rows))
                .when_some(run, |this, run| match run {
                    ScenarioRunState::Running { .. } => this,
                    ScenarioRunState::Finished { result, .. } => match result.as_ref() {
                        Ok(result) => this.child(div().text_xs().child(format!(
                            "Passed · {} postconditions · {}",
                            result.interaction.postconditions.len(),
                            result
                                .interaction
                                .capture
                                .as_ref()
                                .map(|capture| capture.path.display().to_string())
                                .unwrap_or_else(|| "no capture".to_owned())
                        ))),
                        Err(error) => this.child(
                            div()
                                .text_xs()
                                .text_color(cx.theme().danger)
                                .child(error.to_string()),
                        ),
                    },
                })
        });

        v_flex()
            .id("workbench-scenarios")
            .size_full()
            .min_h_0()
            .overflow_hidden()
            .child(toolbar)
            .child(
                div()
                    .debug_selector(|| "workbench-scenarios-items".to_owned())
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
                            .children(scenario_rows),
                    ),
            )
            .into_any_element()
    }
}
