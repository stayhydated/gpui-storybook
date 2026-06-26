use crate::story::StoryContainer;
use std::borrow::Borrow;

/// Stable runtime key for a registered story.
///
/// Keys are globally scoped to the registering crate and story type using the
/// format `{crate-name}-{story-name}`. Unlike story titles, keys are not
/// localized and are suitable for automation and capture routes.
#[derive(
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
    derive_more::AsRef,
    derive_more::Display,
    derive_more::From,
)]
#[as_ref(forward)]
#[display("{_0}")]
pub struct StoryKey(&'static str);

impl StoryKey {
    /// Creates a story key from a static registration label.
    pub const fn new(value: &'static str) -> Self {
        Self(value)
    }

    /// Returns the stable story key.
    pub const fn as_str(self) -> &'static str {
        self.0
    }
}

impl Borrow<str> for StoryKey {
    fn borrow(&self) -> &str {
        self.0
    }
}

/// Story-local identity for a registered story.
///
/// This remains the struct name used for sorting and `disable_story`
/// compatibility. Use [`StoryKey`] when an automation or capture workflow needs
/// a stable global identity.
#[derive(
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
    derive_more::AsRef,
    derive_more::Display,
    derive_more::From,
)]
#[as_ref(forward)]
#[display("{_0}")]
pub struct StoryName(&'static str);

impl StoryName {
    /// Creates a story name from a static registration label.
    pub const fn new(value: &'static str) -> Self {
        Self(value)
    }

    /// Returns the registered story name.
    pub const fn as_str(self) -> &'static str {
        self.0
    }
}

impl Borrow<str> for StoryName {
    fn borrow(&self) -> &str {
        self.0
    }
}

/// Stable identity for a declared story section.
#[derive(
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
    derive_more::AsRef,
    derive_more::Display,
    derive_more::From,
)]
#[as_ref(forward)]
#[display("{_0}")]
pub struct StorySectionName(&'static str);

impl StorySectionName {
    /// Creates a section name from a static registration label.
    pub const fn new(value: &'static str) -> Self {
        Self(value)
    }

    /// Returns the declared section name.
    pub const fn as_str(self) -> &'static str {
        self.0
    }
}

impl Borrow<str> for StorySectionName {
    fn borrow(&self) -> &str {
        self.0
    }
}

/// Entry type for story registration
pub struct StoryEntry {
    pub key: StoryKey,
    pub name: StoryName,
    pub section: Option<StorySectionName>,
    pub section_order: Option<usize>,
    pub create_fn: fn(&mut ::gpui::Window, &mut ::gpui::App) -> ::gpui::Entity<StoryContainer>,
    pub crate_name: &'static str,
    pub crate_dir: &'static str,
    pub file: &'static str,
    pub line: u32,
}

impl StoryEntry {
    /// Creates a registry entry while keeping macro call sites string-oriented.
    pub const fn new(
        key: &'static str,
        name: &'static str,
        section: Option<&'static str>,
        section_order: Option<usize>,
        create_fn: fn(&mut ::gpui::Window, &mut ::gpui::App) -> ::gpui::Entity<StoryContainer>,
        crate_name: &'static str,
        crate_dir: &'static str,
        file: &'static str,
        line: u32,
    ) -> Self {
        let section = match section {
            Some(section) => Some(StorySectionName::new(section)),
            None => None,
        };

        Self {
            key: StoryKey::new(key),
            name: StoryName::new(name),
            section,
            section_order,
            create_fn,
            crate_name,
            crate_dir,
            file,
            line,
        }
    }

    /// Returns this story's stable machine key.
    pub const fn key(&self) -> StoryKey {
        self.key
    }
}

inventory::collect!(StoryEntry);

/// Entry type for init function registration
pub struct InitEntry {
    pub init_fn: fn(&mut ::gpui::App),
    pub fn_name: &'static str,
    pub file: &'static str,
    pub line: u32,
}

inventory::collect!(InitEntry);

#[cfg(test)]
mod tests {
    use super::{StoryContainer, StoryEntry, StoryKey, StoryName, StorySectionName};

    fn unused_create_fn(
        _: &mut ::gpui::Window,
        _: &mut ::gpui::App,
    ) -> ::gpui::Entity<StoryContainer> {
        unreachable!("story creation is not used in this test");
    }

    #[test]
    fn story_key_exposes_registered_label() {
        let key = StoryKey::new("gpui-storybook-example-story-ButtonStory");
        let key_ref: &str = key.as_ref();

        assert_eq!(key.as_str(), "gpui-storybook-example-story-ButtonStory");
        assert_eq!(key.to_string(), "gpui-storybook-example-story-ButtonStory");
        assert_eq!(key_ref, "gpui-storybook-example-story-ButtonStory");
    }

    #[test]
    fn story_name_exposes_registered_label() {
        let name = StoryName::new("ButtonStory");
        let name_ref: &str = name.as_ref();

        assert_eq!(name.as_str(), "ButtonStory");
        assert_eq!(name.to_string(), "ButtonStory");
        assert_eq!(name_ref, "ButtonStory");
    }

    #[test]
    fn section_name_exposes_registered_label() {
        let section = StorySectionName::new("Components");
        let section_ref: &str = section.as_ref();

        assert_eq!(section.as_str(), "Components");
        assert_eq!(section.to_string(), "Components");
        assert_eq!(section_ref, "Components");
    }

    #[test]
    fn story_entry_new_wraps_names_at_registry_boundary() {
        let entry = StoryEntry::new(
            "storybook-ButtonStory",
            "ButtonStory",
            Some("Components"),
            Some(1),
            unused_create_fn,
            "storybook",
            "/tmp/storybook",
            "src/lib.rs",
            42,
        );

        assert_eq!(entry.key().as_str(), "storybook-ButtonStory");
        assert_eq!(entry.name.as_str(), "ButtonStory");
        assert_eq!(
            entry.section.map(StorySectionName::as_str),
            Some("Components")
        );
        assert_eq!(entry.section_order, Some(1));
    }
}
