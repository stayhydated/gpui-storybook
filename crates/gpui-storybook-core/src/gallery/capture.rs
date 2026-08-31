use super::*;

impl Gallery {
    pub(super) fn prepare_capture_current_story(
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
            .or_else(|| self.active_story_snapshot(cx))
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
}
