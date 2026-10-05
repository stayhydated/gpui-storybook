use crate::regions::*;
use gpui::{App, Window};
use gpui_storybook_automation::*;
use tokio::sync::oneshot;

pub fn rendered_interaction_targets(
    story: StorySnapshot,
    window: &Window,
    cx: &App,
) -> Result<StoryInteractionTargetsSnapshot, StorybookAutomationError> {
    let route = story.capture_route_id.clone();
    let targets = interaction_targets(&route, window, cx).map_err(|error| match error {
        InteractionTargetLookupError::RouteNotRendered => {
            StorybookAutomationError::InteractionTargetsUnavailable {
                route: route.clone(),
            }
        },
        InteractionTargetLookupError::DuplicateKey(key) => {
            StorybookAutomationError::DuplicateInteractionTarget {
                route: route.clone(),
                key,
            }
        },
    })?;
    Ok(StoryInteractionTargetsSnapshot { story, targets })
}

pub fn rendered_semantic_values(
    story: StorySnapshot,
    window: &Window,
    cx: &App,
) -> Result<StorySemanticValuesSnapshot, StorybookAutomationError> {
    let route = story.capture_route_id.clone();
    let values = semantic_values(&route, window, cx).map_err(|error| match error {
        SemanticValueLookupError::RouteNotRendered => {
            StorybookAutomationError::SemanticValuesUnavailable {
                route: route.clone(),
            }
        },
        SemanticValueLookupError::DuplicateKey(key) => {
            StorybookAutomationError::DuplicateSemanticValue {
                route: route.clone(),
                key,
            }
        },
    })?;
    Ok(StorySemanticValuesSnapshot { story, values })
}

pub fn schedule_semantic_value_read(
    story: StorySnapshot,
    response: oneshot::Sender<Result<StorySemanticValuesSnapshot, StorybookAutomationError>>,
    window: &mut Window,
) {
    window.refresh();
    window.on_next_frame(move |window, cx| {
        let _ = response.send(rendered_semantic_values(story, window, cx));
    });
}
