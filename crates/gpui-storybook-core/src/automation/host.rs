use super::*;

pub(crate) fn rendered_interaction_targets(
    story: StorySnapshot,
) -> Result<StoryInteractionTargetsSnapshot, StorybookAutomationError> {
    let route = story.capture_route_id.clone();
    let targets = interaction_targets(&route).map_err(|error| match error {
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

pub(crate) fn rendered_semantic_values(
    story: StorySnapshot,
) -> Result<StorySemanticValuesSnapshot, StorybookAutomationError> {
    let route = story.capture_route_id.clone();
    let values = semantic_values(&route).map_err(|error| match error {
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

pub(crate) fn schedule_semantic_value_read(
    story: StorySnapshot,
    response: oneshot::Sender<Result<StorySemanticValuesSnapshot, StorybookAutomationError>>,
    window: &mut Window,
) {
    window.refresh();
    window.on_next_frame(move |_window, _cx| {
        let _ = response.send(rendered_semantic_values(story));
    });
}

pub(super) async fn receive_host_response<T>(
    receiver: oneshot::Receiver<Result<T, StorybookAutomationError>>,
) -> Result<T, StorybookAutomationError> {
    receiver
        .await
        .map_err(|error| StorybookAutomationError::HostDisconnected {
            message: error.to_string(),
            steps_dispatched: 0,
        })?
}

pub(super) fn resolve_story_route(
    stories: &[StorySnapshot],
    route_id: &str,
) -> Option<StorySnapshot> {
    let story_key = capture_route_story_key(route_id);
    let story = stories
        .iter()
        .find(|story| story.key == story_key || story.capture_route_id == story_key)?;

    Some(story_snapshot_for_route(story.clone(), route_id))
}

pub(super) fn find_scenario(
    story: &StorySnapshot,
    scenario_key: &str,
) -> Result<StoryScenarioSnapshot, StorybookAutomationError> {
    let mut matches = story
        .scenarios
        .iter()
        .filter(|scenario| scenario.key == scenario_key);
    let Some(scenario) = matches.next() else {
        return Err(StorybookAutomationError::ScenarioNotFound {
            story_key: story.key.clone(),
            scenario_key: scenario_key.to_owned(),
        });
    };
    if matches.next().is_some() {
        return Err(StorybookAutomationError::DuplicateScenarioKey {
            story_key: story.key.clone(),
            scenario_key: scenario_key.to_owned(),
        });
    }
    Ok(scenario.clone())
}

pub(super) fn story_snapshot_for_route(mut story: StorySnapshot, route_id: &str) -> StorySnapshot {
    if route_id != story.capture_route_id {
        story.capture_route_id = route_id.to_string();
        if let Some((_, slug)) = route_id.split_once('/') {
            story.title = format!("{} / {}", story.title, humanize_capture_slug(slug));
        }
    }

    story
}

pub(super) fn humanize_capture_slug(slug: &str) -> String {
    let mut result = String::new();
    let mut capitalize_next = true;

    for ch in slug.chars() {
        if ch == '-' || ch == '_' {
            result.push(' ');
            capitalize_next = true;
        } else if capitalize_next {
            result.push(ch.to_ascii_uppercase());
            capitalize_next = false;
        } else {
            result.push(ch);
        }
    }

    result
}
