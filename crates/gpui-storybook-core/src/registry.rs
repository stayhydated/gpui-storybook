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
/// This is the struct name used for sorting and `disable_story` matching. Use
/// [`StoryKey`] when an automation or capture workflow needs a stable global
/// identity.
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

/// Typed registration metadata copied from the inventory registry into runtime
/// story containers.
///
/// This keeps story identity, declared section, and source location together so
/// automation, capture, and integrations do not need to coordinate separate
/// string fields by hand.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RegisteredStoryMetadata {
    key: StoryKey,
    name: StoryName,
    section: Option<StorySectionName>,
    crate_name: &'static str,
    source_file: &'static str,
    source_line: u32,
}

impl RegisteredStoryMetadata {
    /// Creates registration metadata from typed registry values.
    pub const fn new(
        key: StoryKey,
        name: StoryName,
        section: Option<StorySectionName>,
        crate_name: &'static str,
        source_file: &'static str,
        source_line: u32,
    ) -> Self {
        Self {
            key,
            name,
            section,
            crate_name,
            source_file,
            source_line,
        }
    }

    /// Returns this story's stable machine key.
    pub const fn key(self) -> StoryKey {
        self.key
    }

    /// Returns the registered story type name.
    pub const fn name(self) -> StoryName {
        self.name
    }

    /// Returns the declared registration section, if any.
    pub const fn section(self) -> Option<StorySectionName> {
        self.section
    }

    /// Returns the crate package name that registered the story.
    pub const fn crate_name(self) -> &'static str {
        self.crate_name
    }

    /// Returns the source file recorded by the registration macro.
    pub const fn source_file(self) -> &'static str {
        self.source_file
    }

    /// Returns the source line recorded by the registration macro.
    pub const fn source_line(self) -> u32 {
        self.source_line
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

/// Compile-time source provenance for a registered story.
///
/// Registration macros use this value to keep package and source-location
/// fields together when constructing a [`StoryEntry`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StoryRegistrationSource {
    crate_name: &'static str,
    crate_dir: &'static str,
    file: &'static str,
    line: u32,
}

impl StoryRegistrationSource {
    /// Creates source provenance from compile-time Cargo and Rust macros.
    pub const fn new(
        crate_name: &'static str,
        crate_dir: &'static str,
        file: &'static str,
        line: u32,
    ) -> Self {
        Self {
            crate_name,
            crate_dir,
            file,
            line,
        }
    }
}

impl StoryEntry {
    /// Creates a registry entry while keeping macro call sites string-oriented.
    pub const fn new(
        key: &'static str,
        name: &'static str,
        section: Option<&'static str>,
        section_order: Option<usize>,
        create_fn: fn(&mut ::gpui::Window, &mut ::gpui::App) -> ::gpui::Entity<StoryContainer>,
        source: StoryRegistrationSource,
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
            crate_name: source.crate_name,
            crate_dir: source.crate_dir,
            file: source.file,
            line: source.line,
        }
    }

    /// Returns this story's stable machine key.
    pub const fn key(&self) -> StoryKey {
        self.key
    }

    /// Returns the typed metadata that should be copied into runtime
    /// [`StoryContainer`] values.
    pub const fn metadata(&self) -> RegisteredStoryMetadata {
        RegisteredStoryMetadata::new(
            self.key,
            self.name,
            self.section,
            self.crate_name,
            self.file,
            self.line,
        )
    }
}

impl From<&StoryEntry> for RegisteredStoryMetadata {
    fn from(entry: &StoryEntry) -> Self {
        entry.metadata()
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
    use super::{
        RegisteredStoryMetadata, StoryContainer, StoryEntry, StoryKey, StoryName,
        StoryRegistrationSource, StorySectionName,
    };

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
        let borrowed: &str = std::borrow::Borrow::borrow(&key);

        assert_eq!(key.as_str(), "gpui-storybook-example-story-ButtonStory");
        assert_eq!(key.to_string(), "gpui-storybook-example-story-ButtonStory");
        assert_eq!(key_ref, "gpui-storybook-example-story-ButtonStory");
        assert_eq!(borrowed, "gpui-storybook-example-story-ButtonStory");
    }

    #[test]
    fn story_name_exposes_registered_label() {
        let name = StoryName::new("ButtonStory");
        let name_ref: &str = name.as_ref();
        let borrowed: &str = std::borrow::Borrow::borrow(&name);

        assert_eq!(name.as_str(), "ButtonStory");
        assert_eq!(name.to_string(), "ButtonStory");
        assert_eq!(name_ref, "ButtonStory");
        assert_eq!(borrowed, "ButtonStory");
    }

    #[test]
    fn section_name_exposes_registered_label() {
        let section = StorySectionName::new("Components");
        let section_ref: &str = section.as_ref();
        let borrowed: &str = std::borrow::Borrow::borrow(&section);

        assert_eq!(section.as_str(), "Components");
        assert_eq!(section.to_string(), "Components");
        assert_eq!(section_ref, "Components");
        assert_eq!(borrowed, "Components");
    }

    #[test]
    fn story_entry_new_wraps_names_at_registry_boundary() {
        let entry = StoryEntry::new(
            "storybook-ButtonStory",
            "ButtonStory",
            Some("Components"),
            Some(1),
            unused_create_fn,
            StoryRegistrationSource::new("storybook", "/tmp/storybook", "src/lib.rs", 42),
        );

        assert_eq!(entry.key().as_str(), "storybook-ButtonStory");
        assert_eq!(entry.name.as_str(), "ButtonStory");
        assert_eq!(
            entry.section.map(StorySectionName::as_str),
            Some("Components")
        );
        assert_eq!(entry.section_order, Some(1));
    }

    #[test]
    fn story_entry_metadata_keeps_registration_fields_together() {
        let entry = StoryEntry::new(
            "storybook-ButtonStory",
            "ButtonStory",
            Some("Components"),
            Some(1),
            unused_create_fn,
            StoryRegistrationSource::new("storybook", "/tmp/storybook", "src/lib.rs", 42),
        );

        let metadata = RegisteredStoryMetadata::from(&entry);

        assert_eq!(metadata.key(), StoryKey::new("storybook-ButtonStory"));
        assert_eq!(metadata.name(), StoryName::new("ButtonStory"));
        assert_eq!(
            metadata.section(),
            Some(StorySectionName::new("Components"))
        );
        assert_eq!(metadata.crate_name(), "storybook");
        assert_eq!(metadata.source_file(), "src/lib.rs");
        assert_eq!(metadata.source_line(), 42);
    }

    #[test]
    fn story_entry_supports_unsectioned_registration() {
        let entry = StoryEntry::new(
            "storybook-ButtonStory",
            "ButtonStory",
            None,
            None,
            unused_create_fn,
            StoryRegistrationSource::new("storybook", "/tmp/storybook", "src/lib.rs", 42),
        );

        assert_eq!(entry.section, None);
        assert_eq!(entry.metadata().section(), None);
        assert_eq!(entry.crate_dir, "/tmp/storybook");
    }
}
