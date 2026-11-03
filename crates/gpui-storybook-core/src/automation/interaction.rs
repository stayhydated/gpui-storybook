//! Gallery policy over the reusable owning-thread executor.
use gpui_kit::App;
pub use gpui_storybook_automation::*;
pub use gpui_storybook_automation_gpui::interaction::{
    PreparedInteractionStep, PreparedStoryInteraction, interaction_target_size,
    schedule_interaction_target_listing, schedule_story_interaction,
};

pub(crate) fn list_registered_actions(cx: &App) -> Vec<StoryActionSnapshot> {
    gpui_storybook_automation_gpui::interaction::list_registered_actions(cx)
        .into_iter()
        .filter(|action| !action.name.starts_with("storybook_workbench::"))
        .collect()
}

pub(crate) fn prepare_interaction_steps(
    steps: &[StoryInteractionStep],
    cx: &App,
) -> Result<Vec<PreparedInteractionStep>, StorybookAutomationError> {
    let action_scope = list_registered_actions(cx)
        .into_iter()
        .map(|action| action.name)
        .collect();
    gpui_storybook_automation_gpui::interaction::prepare_interaction_steps(steps, &action_scope, cx)
}
