use super::*;

pub(super) fn group_duplicate_story_titles(
    stories: Vec<::gpui::Entity<StoryContainer>>,
    window: &mut ::gpui::Window,
    cx: &mut ::gpui::App,
) -> Vec<::gpui::Entity<StoryContainer>> {
    let mut grouped: Vec<(StoryGroupKey, Vec<::gpui::Entity<StoryContainer>>)> = Vec::new();

    for story in stories {
        let key = {
            let story_data = story.read(cx);
            StoryGroupKey {
                group: story_data.group.as_ref().map(ToString::to_string),
                section: story_data.section.as_ref().map(ToString::to_string),
                title: story_data.display_title(cx),
            }
        };

        if let Some((_, bucket)) = grouped
            .iter_mut()
            .find(|(existing_key, _)| *existing_key == key)
        {
            bucket.push(story);
        } else {
            grouped.push((key, vec![story]));
        }
    }

    grouped
        .into_iter()
        .map(|(key, bucket)| {
            if bucket.len() == 1 {
                return bucket.into_iter().next().expect("bucket has one story");
            }

            let panel = StoryContainer::variant_group(key.title, bucket, window, cx);
            panel.update(cx, |container, _| {
                container.group = key.group.map(Into::into);
                container.section = key.section.map(Into::into);
            });
            panel
        })
        .collect()
}
