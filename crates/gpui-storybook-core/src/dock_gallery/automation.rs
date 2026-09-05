use super::*;

impl StoryWorkspace {
    pub(super) fn attach_automation_host(
        &self,
        mut receiver: StorybookAutomationCommandReceiver,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        cx.spawn_in(window, async move |this, cx| {
            while let Some(command) = receiver.recv().await {
                let _ = this.update_in(cx, |workspace, window, cx| {
                    workspace.handle_automation_command(command, window, cx);
                });
            }
        })
        .detach();
    }

    pub(crate) fn handle_automation_command(
        &mut self,
        command: StorybookAutomationCommand,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match command {
            StorybookAutomationCommand::OpenStory { key, response, .. } => {
                let result = self.open_story_by_key(&key, window, cx);
                let _ = response.send(result);
            },
            StorybookAutomationCommand::CaptureCurrentStory {
                request_id,
                request,
                response,
                operation,
            } => {
                let quit_after_capture = request.quit_after_capture;
                match self.prepare_capture_current_story(&request, window, cx) {
                    Ok(story) => {
                        schedule_story_capture(
                            request_id,
                            request,
                            story,
                            response,
                            operation,
                            quit_after_capture,
                            window,
                        );
                    },
                    Err(error) => {
                        eprintln!("gpui-storybook capture session failed: {error}");
                        let _ = response.send(Err(error));
                        if quit_after_capture {
                            std::process::exit(1);
                        }
                    },
                }
            },
            StorybookAutomationCommand::ReadControls { response } => {
                let result = self.workbench_state.read(cx).controls_snapshot(cx);
                let _ = response.send(result);
            },
            StorybookAutomationCommand::SetControl {
                key,
                value,
                response,
                ..
            } => {
                let result = self
                    .workbench_state
                    .update(cx, |state, cx| state.set_control(&key, value, cx));
                cx.notify();
                let _ = response.send(result);
            },
            StorybookAutomationCommand::ResetControl { key, response, .. } => {
                let result = self
                    .workbench_state
                    .update(cx, |state, cx| state.reset_control(key.as_deref(), cx));
                cx.notify();
                let _ = response.send(result);
            },
            StorybookAutomationCommand::ListActions { response } => {
                let _ = response.send(Ok(crate::automation::interaction::list_registered_actions(
                    cx,
                )));
            },
            StorybookAutomationCommand::ListInteractionTargets { response } => {
                let result = self
                    .automation
                    .as_ref()
                    .and_then(|automation| automation.current_story().story)
                    .ok_or(StorybookAutomationError::NoActiveStory);
                match result {
                    Ok(story) => {
                        crate::automation::interaction::schedule_interaction_target_listing(
                            story, response, window,
                        );
                    },
                    Err(error) => {
                        let _ = response.send(Err(error));
                    },
                }
            },
            StorybookAutomationCommand::ReadSemanticValues { response } => {
                let result = self
                    .automation
                    .as_ref()
                    .and_then(|automation| automation.current_story().story)
                    .ok_or(StorybookAutomationError::NoActiveStory);
                match result {
                    Ok(story) => {
                        crate::automation::schedule_semantic_value_read(story, response, window)
                    },
                    Err(error) => {
                        let _ = response.send(Err(error));
                    },
                }
            },
            StorybookAutomationCommand::RunSteps {
                request_id,
                request,
                fresh_story,
                response,
                progress,
                operation,
            } => {
                if response.is_closed() {
                    return;
                }
                let prepared = (|| {
                    crate::automation::interaction::validate_interaction_request(&request)?;
                    let steps = crate::automation::interaction::prepare_interaction_steps(
                        &request.steps,
                        cx,
                    )?;
                    if let Some(route) = &request.story_key {
                        self.open_story_by_key(route, window, cx)?;
                    }
                    if fresh_story {
                        let story_entity = self
                            .workbench_state
                            .read(cx)
                            .active_story()
                            .ok_or(StorybookAutomationError::NoActiveStory)?;
                        story_entity.update(cx, |story, cx| {
                            story.recreate_for_scenario(window, cx);
                        });
                    }
                    if let Some(presentation) = request.presentation {
                        self.workbench_state.update(cx, |state, cx| {
                            state.set_viewport(presentation.viewport, cx);
                            state.set_background(presentation.background, cx);
                        });
                    }
                    self.workbench_state
                        .update(cx, |state, cx| state.apply_controls(&request.controls, cx))?;
                    let story = self
                        .automation
                        .as_ref()
                        .and_then(|automation| automation.current_story().story)
                        .ok_or(StorybookAutomationError::NoActiveStory)?;
                    let target_size =
                        crate::automation::interaction::interaction_target_size(&request)?;
                    let story_entity = self
                        .workbench_state
                        .read(cx)
                        .active_story()
                        .ok_or(StorybookAutomationError::NoActiveStory)?;
                    set_capture_target_size(&story_entity, window, target_size, cx);
                    if request.story_key.is_some() {
                        gpui_kit::Focusable::focus_handle(&story_entity, cx).focus(window, cx);
                    }
                    cx.notify();
                    window.refresh();
                    Ok((story, steps, request.postconditions, request.capture))
                })();

                match prepared {
                    Ok((story, steps, postconditions, capture)) => {
                        if response.is_closed() {
                            return;
                        }
                        crate::automation::interaction::schedule_story_interaction(
                            crate::automation::interaction::PreparedStoryInteraction {
                                request_id,
                                story,
                                steps,
                                postconditions,
                                capture,
                                response,
                                progress,
                                operation,
                            },
                            window,
                        );
                    },
                    Err(error) => {
                        let _ = response.send(Err(error));
                    },
                }
            },
        }
    }

    pub(crate) fn open_story_by_key(
        &mut self,
        key: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<StoryCurrentSnapshot, StorybookAutomationError> {
        let story_key = capture_route_story_key(key);
        StorySidebar::open_story_by_key(
            self.dock_area.downgrade(),
            story_key,
            self.automation.clone(),
            window,
            cx,
        )
        .ok_or_else(|| StorybookAutomationError::StoryNotFound {
            key: key.to_string(),
        })?;

        self.automation
            .as_ref()
            .expect("automation command requires automation")
            .confirm_current_story(key)
    }

    fn prepare_capture_current_story(
        &mut self,
        request: &StoryScreenshotRequest,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<StorySnapshot, StorybookAutomationError> {
        self.workbench_state
            .update(cx, |state, cx| state.apply_controls(&request.controls, cx))?;
        let story = self
            .automation
            .as_ref()
            .and_then(|automation| automation.current_story().story)
            .ok_or_else(|| StorybookAutomationError::CaptureUnavailable {
                message: "no current story is selected for capture".to_string(),
            })?;

        let target_size = validate_capture_target_size(request)?;
        let story_entity = self
            .workbench_state
            .read(cx)
            .active_story()
            .ok_or(StorybookAutomationError::NoActiveStory)?;
        set_capture_target_size(&story_entity, window, target_size, cx);
        cx.notify();
        window.refresh();

        Ok(story)
    }

    pub(crate) fn active_story_snapshot(&self, cx: &App) -> Option<StorySnapshot> {
        let story = self.workbench_state.read(cx).active_story()?;
        StorySnapshot::from_container(story.read(cx), cx)
    }
}
