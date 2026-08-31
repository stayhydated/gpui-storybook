use super::*;

impl Gallery {
    pub(super) fn set_active_story(&mut self, name: &str, app_cx: &mut App) {
        let lowercase_name = name.to_lowercase().replace("story", "");
        let story_index = self.stories.iter().position(|story_entity| {
            let story_data = story_entity.read(app_cx);
            let title = story_data.display_title(app_cx);
            title.to_lowercase().replace("story", "") == lowercase_name
        });

        if let Some(index) = story_index {
            self.active_index = Some(index);
            self.sync_workbench_active(app_cx);
        }
    }

    pub(super) fn sync_workbench_active(&self, cx: &mut App) {
        let story = self
            .active_index
            .and_then(|index| self.stories.get(index))
            .cloned();
        self.workbench_state.update(cx, |state, cx| {
            state.set_active_story(story, cx);
        });
    }

    pub(crate) fn active_story_snapshot(&self, cx: &impl Borrow<App>) -> Option<StorySnapshot> {
        let story = self.workbench_state.read(cx.borrow()).active_story()?;
        StorySnapshot::from_container(story.read(cx.borrow()), cx)
    }

    pub(super) fn sync_automation_stories(&self, cx: &impl Borrow<App>) {
        if let Some(automation) = &self.automation {
            automation.set_stories(story_snapshots_from_containers(&self.stories, cx));
        }
    }

    pub(super) fn confirm_active_story(&self, cx: &impl Borrow<App>) {
        let Some(automation) = &self.automation else {
            return;
        };
        let Some(snapshot) = self.active_story_snapshot(cx) else {
            return;
        };

        let _ = automation.confirm_current_story(&snapshot.key);
    }

    fn story_contains_key(
        story: &Entity<StoryContainer>,
        key: &str,
        cx: &impl Borrow<App>,
    ) -> bool {
        let (matches, members) = {
            let story = story.read(cx.borrow());
            (
                story
                    .story_key_label()
                    .is_some_and(|story_key| story_key == key),
                story.variants.clone(),
            )
        };

        matches
            || members
                .iter()
                .any(|member| Self::story_contains_key(member, key, cx))
    }

    pub(crate) fn set_active_story_by_key(
        &mut self,
        key: &str,
        cx: &mut App,
    ) -> Result<StoryCurrentSnapshot, StorybookAutomationError> {
        let story_key = capture_route_story_key(key);
        let Some(index) = self
            .stories
            .iter()
            .position(|story| Self::story_contains_key(story, story_key, cx))
        else {
            return Err(StorybookAutomationError::StoryNotFound {
                key: key.to_string(),
            });
        };

        self.active_index = Some(index);
        let group = self.stories[index].clone();
        self.workbench_state.update(cx, |state, cx| {
            state.set_active_story_by_key(group, story_key, cx);
        });
        self.automation
            .as_ref()
            .expect("automation command requires automation")
            .confirm_current_story(key)
    }
}
