use super::*;

impl StoryContainer {
    /// Store typed registry metadata on this runtime container.
    ///
    /// This also populates the string metadata fields exposed by this
    /// container.
    pub fn set_registration_metadata(&mut self, metadata: RegisteredStoryMetadata) {
        self.story_key = Some(metadata.key().as_str().into());
        self.story_name = Some(metadata.name().as_str().into());
        self.crate_name = Some(metadata.crate_name().into());
        self.source_file = Some(metadata.source_file().into());
        self.source_line = Some(metadata.source_line());
        self.registration_metadata = Some(metadata);
    }

    /// Returns the typed metadata copied from the inventory registry.
    pub fn registration_metadata(&self) -> Option<RegisteredStoryMetadata> {
        self.registration_metadata
    }

    /// Returns this story's typed stable key when it came from the registry.
    pub fn story_key(&self) -> Option<StoryKey> {
        self.registration_metadata.map(RegisteredStoryMetadata::key)
    }

    /// Returns this story's typed registered name when it came from the
    /// registry.
    pub fn story_name(&self) -> Option<StoryName> {
        self.registration_metadata
            .map(RegisteredStoryMetadata::name)
    }

    /// Returns this story's stable key as a string label.
    pub fn story_key_label(&self) -> Option<&str> {
        self.registration_metadata
            .map(|metadata| metadata.key().as_str())
            .or_else(|| self.story_key.as_ref().map(|story_key| story_key.as_ref()))
    }

    /// Returns this story's registered name as a string label.
    pub fn story_name_label(&self) -> Option<&str> {
        self.registration_metadata
            .map(|metadata| metadata.name().as_str())
            .or_else(|| {
                self.story_name
                    .as_ref()
                    .map(|story_name| story_name.as_ref())
            })
    }

    /// Returns the crate package name that registered this story.
    pub fn crate_name_label(&self) -> Option<&str> {
        self.registration_metadata
            .map(RegisteredStoryMetadata::crate_name)
            .or_else(|| {
                self.crate_name
                    .as_ref()
                    .map(|crate_name| crate_name.as_ref())
            })
    }

    /// Returns the source file recorded for this story.
    pub fn source_file_label(&self) -> Option<&str> {
        self.registration_metadata
            .map(RegisteredStoryMetadata::source_file)
            .or_else(|| {
                self.source_file
                    .as_ref()
                    .map(|source_file| source_file.as_ref())
            })
    }

    /// Returns the source line recorded for this story.
    pub fn source_line(&self) -> Option<u32> {
        self.registration_metadata
            .map(RegisteredStoryMetadata::source_line)
            .or(self.source_line)
    }

    pub fn display_title(&self, cx: &impl Borrow<App>) -> String {
        if let Some(title_fn) = &self.title_fn {
            title_fn(cx.borrow())
        } else {
            self.name.to_string()
        }
    }

    pub fn display_description(&self, cx: &impl Borrow<App>) -> String {
        if let Some(description_fn) = &self.description_fn {
            description_fn(cx.borrow())
        } else {
            self.description.to_string()
        }
    }
}

pub(super) fn recreate_story<S: Story>(
    window: &mut Window,
    cx: &mut App,
) -> (
    AnyView,
    Option<Rc<dyn ControlTarget>>,
    gpui::FocusHandle,
    Option<gpui::FocusHandle>,
) {
    let story = S::new_view(window, cx);
    let control_target = EntityControlTarget::optional(story.clone(), cx);
    let focus_handle = story.focus_handle(cx);
    let action_scope_focus_handle = story.read(cx).action_scope_focus_handle(cx);
    (
        story.into(),
        control_target,
        focus_handle,
        action_scope_focus_handle,
    )
}

#[derive(Debug, Deserialize, Serialize)]
pub struct StoryState {
    pub story_klass: SharedString,
}

impl StoryState {
    pub(super) fn to_value(&self) -> serde_json::Value {
        serde_json::json!({
            "story_klass": self.story_klass,
        })
    }
}
