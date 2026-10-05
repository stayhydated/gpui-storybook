//! Gallery backend preserves the live controller's owner-thread execution.

use super::*;

impl AutomationBackend for StorybookAutomation {
    fn capabilities(&self) -> AutomationCapabilities {
        use AutomationCapability::*;
        let mut capabilities = vec![
            Navigation,
            Controls,
            ControlMutation,
            SemanticValues,
            Focus,
            Keystrokes,
            TextInsertion,
            Actions,
            Pointer,
            Scroll,
            FrameWaits,
            SemanticTargets,
            FreshScenarios,
            StorySizing,
            Presentation,
            DesktopLaunch,
        ];
        if cfg!(feature = "capture") {
            capabilities.push(StoryCapture);
            capabilities.push(CaptureControls);
            capabilities.push(InteractionCapture);
        }
        AutomationCapabilities::new(capabilities)
    }
    fn wait_until_ready(&self) -> BackendFuture<'_, ()> {
        Box::pin(async move {
            StorybookAutomation::wait_until_ready(self).await;
            Ok(())
        })
    }
    fn stories(&self) -> BackendFuture<'_, Vec<StorySnapshot>> {
        Box::pin(async move { Ok(StorybookAutomation::stories(self)) })
    }
    fn get_story(&self, key: String) -> BackendFuture<'_, StorySnapshot> {
        Box::pin(async move { StorybookAutomation::get_story(self, &key) })
    }
    fn current_story(&self) -> BackendFuture<'_, StoryCurrentSnapshot> {
        Box::pin(async move { Ok(StorybookAutomation::current_story(self)) })
    }
    fn list_scenarios(
        &self,
        story_key: Option<String>,
    ) -> BackendFuture<'_, StoryScenariosSnapshot> {
        Box::pin(async move {
            match story_key {
                Some(key) => StorybookAutomation::list_scenarios_for(self, &key),
                None => StorybookAutomation::list_scenarios(self),
            }
        })
    }
    fn open_story(&self, key: String) -> BackendFuture<'_, StoryCurrentSnapshot> {
        Box::pin(async move { StorybookAutomation::open_story(self, key).await })
    }
    fn read_controls(&self) -> BackendFuture<'_, StoryControlsSnapshot> {
        Box::pin(async move { StorybookAutomation::read_controls(self).await })
    }
    fn set_control(
        &self,
        key: String,
        value: ControlValue,
    ) -> BackendFuture<'_, StoryControlsSnapshot> {
        Box::pin(async move { StorybookAutomation::set_control(self, key, value).await })
    }
    fn reset_control(&self, key: Option<String>) -> BackendFuture<'_, StoryControlsSnapshot> {
        Box::pin(async move { StorybookAutomation::reset_control(self, key).await })
    }
    fn list_actions(&self) -> BackendFuture<'_, Vec<StoryActionSnapshot>> {
        Box::pin(async move { StorybookAutomation::list_actions(self).await })
    }
    fn list_interaction_targets(&self) -> BackendFuture<'_, StoryInteractionTargetsSnapshot> {
        Box::pin(async move { StorybookAutomation::list_interaction_targets(self).await })
    }
    fn read_semantic_values(&self) -> BackendFuture<'_, StorySemanticValuesSnapshot> {
        Box::pin(async move { StorybookAutomation::read_semantic_values(self).await })
    }
    fn run_steps(
        &self,
        request: StoryInteractionRequest,
    ) -> BackendFuture<'_, StoryInteractionSnapshot> {
        Box::pin(async move {
            self.capabilities().validate_interaction(&request)?;
            StorybookAutomation::run_steps(self, request).await
        })
    }
    fn run_scenario(
        &self,
        story_key: Option<String>,
        scenario_key: String,
    ) -> BackendFuture<'_, StoryScenarioRunSnapshot> {
        Box::pin(async move {
            self.capabilities()
                .require(AutomationCapability::FreshScenarios)?;
            let story = match story_key.as_deref() {
                Some(key) => StorybookAutomation::get_story(self, key)?,
                None => StorybookAutomation::current_story(self)
                    .story
                    .ok_or(StorybookAutomationError::NoActiveStory)?,
            };
            let scenario = find_scenario(&story, &scenario_key)?;
            self.capabilities()
                .validate_interaction(&scenario.interaction_request(story.capture_route_id))?;
            StorybookAutomation::run_scenario(self, story_key, scenario_key).await
        })
    }
    fn capture_current_story(
        &self,
        request: StoryScreenshotRequest,
    ) -> BackendFuture<'_, StoryCaptureSnapshot> {
        Box::pin(async move {
            self.capabilities().validate_capture(&request)?;
            StorybookAutomation::capture_current_story(self, request).await
        })
    }
}
