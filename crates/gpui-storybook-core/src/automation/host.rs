use super::*;

pub(crate) use gpui_storybook_automation_gpui::snapshot::schedule_semantic_value_read;

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
