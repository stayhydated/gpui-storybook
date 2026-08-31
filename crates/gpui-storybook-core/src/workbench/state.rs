use super::*;

/// Tabs available in the Storybook workbench.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkbenchTab {
    #[default]
    Controls,
    Theme,
    Inspect,
    Actions,
    Scenarios,
    #[cfg(feature = "performance")]
    Performance,
}

pub(crate) enum WorkbenchEvent {
    OpenVariant(Entity<StoryContainer>),
}

impl WorkbenchTab {
    pub(super) fn index(self) -> usize {
        match self {
            Self::Controls => 0,
            Self::Theme => 1,
            Self::Inspect => 2,
            Self::Actions => 3,
            Self::Scenarios => 4,
            #[cfg(feature = "performance")]
            Self::Performance => 5,
        }
    }

    pub(super) fn from_index(index: usize) -> Self {
        match index {
            1 => Self::Theme,
            2 => Self::Inspect,
            3 => Self::Actions,
            4 => Self::Scenarios,
            #[cfg(feature = "performance")]
            5 => Self::Performance,
            _ => Self::Controls,
        }
    }
}

/// Per-window active story and variant selection.
pub struct WorkbenchState {
    active_group: Option<Entity<StoryContainer>>,
    active_story: Option<Entity<StoryContainer>>,
    automation: Option<SharedStorybookAutomation>,
    presentation: StoryPresentation,
    responsive_size: Option<Size<Pixels>>,
}

impl WorkbenchState {
    /// Creates window-scoped workbench state and resolves a variant group to its
    /// first concrete member.
    pub fn new(initial_story: Option<Entity<StoryContainer>>, cx: &App) -> Self {
        Self::new_with_automation(initial_story, None, cx)
    }

    pub(crate) fn new_with_automation(
        initial_story: Option<Entity<StoryContainer>>,
        automation: Option<SharedStorybookAutomation>,
        cx: &App,
    ) -> Self {
        let (active_group, active_story) = Self::resolve_story(initial_story, cx);
        Self {
            active_group,
            active_story,
            automation,
            presentation: StoryPresentation::default(),
            responsive_size: None,
        }
    }

    fn resolve_story(
        story: Option<Entity<StoryContainer>>,
        cx: &App,
    ) -> (
        Option<Entity<StoryContainer>>,
        Option<Entity<StoryContainer>>,
    ) {
        story.map_or((None, None), |story| {
            let (first_variant, variant_group) = {
                let story_data = story.read(cx);
                (
                    story_data.variants.first().cloned(),
                    story_data
                        .variant_group
                        .as_ref()
                        .and_then(gpui::WeakEntity::upgrade),
                )
            };

            if let Some(first_variant) = first_variant {
                (Some(story), Some(first_variant))
            } else if let Some(variant_group) = variant_group {
                (Some(variant_group), Some(story))
            } else {
                (Some(story.clone()), Some(story))
            }
        })
    }

    /// Select a gallery or dock story, choosing its first variant when grouped.
    pub fn set_active_story(
        &mut self,
        story: Option<Entity<StoryContainer>>,
        cx: &mut Context<Self>,
    ) {
        let (group, active_story) = Self::resolve_story(story, cx);
        self.active_group = group;
        self.active_story = active_story;
        self.apply_presentation(cx);
        cx.notify();
    }

    /// Select a story group and the exact registered member matching `key`.
    pub fn set_active_story_by_key(
        &mut self,
        story: Entity<StoryContainer>,
        key: &str,
        cx: &mut Context<Self>,
    ) {
        fn find(
            story: &Entity<StoryContainer>,
            key: &str,
            cx: &App,
        ) -> Option<Entity<StoryContainer>> {
            let (matches, members) = {
                let story = story.read(cx);
                (
                    story
                        .story_key_label()
                        .is_some_and(|candidate| candidate == key),
                    story.variants.clone(),
                )
            };
            if matches {
                return Some(story.clone());
            }
            members.iter().find_map(|member| find(member, key, cx))
        }

        self.active_group = if story.read(cx).variants.is_empty() {
            story
                .read(cx)
                .variant_group
                .as_ref()
                .and_then(gpui::WeakEntity::upgrade)
                .or(Some(story.clone()))
        } else {
            Some(story.clone())
        };
        self.active_story = find(&story, key, cx)
            .or_else(|| story.read(cx).variants.first().cloned().or(Some(story)));
        self.apply_presentation(cx);
        cx.notify();
    }

    /// Select one member of the active grouped story.
    pub fn set_active_variant(&mut self, story: Entity<StoryContainer>, cx: &mut Context<Self>) {
        let belongs_to_group = self.active_group.as_ref().is_some_and(|group| {
            group
                .read(cx)
                .variants
                .iter()
                .any(|member| member == &story)
        });
        if belongs_to_group || self.active_group.as_ref() == Some(&story) {
            let variant = story.clone();
            self.active_story = Some(story);
            self.apply_presentation(cx);
            cx.emit(WorkbenchEvent::OpenVariant(variant));
            cx.notify();
        }
    }

    pub fn active_story(&self) -> Option<Entity<StoryContainer>> {
        self.active_story.clone()
    }

    pub(crate) fn automation(&self) -> Option<SharedStorybookAutomation> {
        self.automation.clone()
    }

    pub fn active_group(&self) -> Option<Entity<StoryContainer>> {
        self.active_group.clone()
    }

    pub fn variants(&self, cx: &App) -> Vec<Entity<StoryContainer>> {
        self.active_group
            .as_ref()
            .map(|group| group.read(cx).variants.clone())
            .unwrap_or_default()
    }

    pub fn presentation(&self) -> StoryPresentation {
        self.presentation
    }

    #[cfg(test)]
    pub(crate) fn responsive_size(&self) -> Option<Size<Pixels>> {
        self.responsive_size
    }

    pub fn set_viewport(&mut self, viewport: StoryViewportPreset, cx: &mut Context<Self>) {
        if viewport == StoryViewportPreset::Responsive
            && self.presentation.viewport != StoryViewportPreset::Responsive
        {
            self.responsive_size = self
                .presentation
                .viewport
                .dimensions()
                .map(|(width, height)| size(px(width as f32), px(height as f32)));
        }
        self.presentation.viewport = viewport;
        self.apply_presentation(cx);
        cx.notify();
    }

    pub fn set_background(&mut self, background: StoryCanvasBackground, cx: &mut Context<Self>) {
        self.presentation.background = background;
        self.apply_presentation(cx);
        cx.notify();
    }

    pub(crate) fn set_responsive_size(
        &mut self,
        responsive_size: Size<Pixels>,
        cx: &mut Context<Self>,
    ) {
        if self.presentation.viewport != StoryViewportPreset::Responsive {
            return;
        }
        self.responsive_size = Some(responsive_size);
        cx.notify();
    }

    fn apply_presentation(&self, cx: &mut Context<Self>) {
        if let Some(story) = &self.active_story {
            let workbench_state = cx.entity().downgrade();
            story.update(cx, |story, cx| {
                story.set_presentation(self.presentation);
                story.set_responsive_size(self.responsive_size);
                story.set_workbench_state(workbench_state);
                cx.notify();
            });
        }
    }

    pub(crate) fn controls_snapshot(
        &self,
        cx: &App,
    ) -> Result<StoryControlsSnapshot, StorybookAutomationError> {
        let story = self
            .active_story()
            .ok_or(StorybookAutomationError::NoActiveStory)?;
        let snapshot = StorySnapshot::from_container(story.read(cx), cx)
            .ok_or(StorybookAutomationError::NoActiveStory)?;
        let target = story.read(cx).control_target().ok_or_else(|| {
            StorybookAutomationError::ControlsUnavailable {
                key: snapshot.key.clone(),
            }
        })?;
        let controls = target.snapshots(cx).map_err(|error| {
            StorybookAutomationError::ControlOperationFailed {
                message: error.to_string(),
            }
        })?;
        Ok(StoryControlsSnapshot {
            story: snapshot,
            controls,
        })
    }

    pub(crate) fn set_control(
        &mut self,
        key: &str,
        value: ControlValue,
        cx: &mut Context<Self>,
    ) -> Result<StoryControlsSnapshot, StorybookAutomationError> {
        let story = self
            .active_story()
            .ok_or(StorybookAutomationError::NoActiveStory)?;
        let target = story.read(cx).control_target().ok_or_else(|| {
            StorybookAutomationError::ControlsUnavailable {
                key: story
                    .read(cx)
                    .story_key_label()
                    .unwrap_or_default()
                    .to_owned(),
            }
        })?;
        target.set(key, value, cx).map_err(|error| {
            StorybookAutomationError::ControlOperationFailed {
                message: error.to_string(),
            }
        })?;
        cx.notify();
        self.controls_snapshot(cx)
    }

    pub(crate) fn reset_control(
        &mut self,
        key: Option<&str>,
        cx: &mut Context<Self>,
    ) -> Result<StoryControlsSnapshot, StorybookAutomationError> {
        let story = self
            .active_story()
            .ok_or(StorybookAutomationError::NoActiveStory)?;
        let target = story.read(cx).control_target().ok_or_else(|| {
            StorybookAutomationError::ControlsUnavailable {
                key: story
                    .read(cx)
                    .story_key_label()
                    .unwrap_or_default()
                    .to_owned(),
            }
        })?;
        let result = if let Some(key) = key {
            target.reset(key, cx)
        } else {
            target.reset_all(cx)
        };
        result.map_err(|error| StorybookAutomationError::ControlOperationFailed {
            message: error.to_string(),
        })?;
        cx.notify();
        self.controls_snapshot(cx)
    }

    pub(crate) fn apply_controls(
        &mut self,
        controls: &BTreeMap<String, ControlValue>,
        cx: &mut Context<Self>,
    ) -> Result<(), StorybookAutomationError> {
        for (key, value) in controls {
            self.set_control(key, value.clone(), cx)?;
        }
        Ok(())
    }
}

impl EventEmitter<WorkbenchEvent> for WorkbenchState {}
