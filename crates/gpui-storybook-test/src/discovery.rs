use super::*;

/// Static metadata and executable constructor for one inventory story.
#[derive(Clone)]
pub struct StoryDescriptor {
    pub(super) entry: Option<&'static StoryEntry>,
    pub(super) metadata: PortableStoryMetadata,
}

impl fmt::Debug for StoryDescriptor {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("StoryDescriptor")
            .field("metadata", &self.metadata)
            .field("has_constructor", &self.entry.is_some())
            .finish()
    }
}

/// Inventory metadata copied into a serializable discovery report.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PortableStoryMetadata {
    /// Globally stable story key.
    pub key: String,
    /// Registered Rust story name.
    pub name: String,
    /// Declared Storybook section.
    pub section: Option<String>,
    /// Source package.
    pub crate_name: String,
    /// Source manifest directory.
    pub crate_dir: String,
    /// Source file.
    pub source_file: String,
    /// Source line.
    pub source_line: u32,
    /// Rustdoc captured at registration time.
    pub docs: String,
}

impl StoryDescriptor {
    fn from_entry(entry: &'static StoryEntry) -> Self {
        let metadata = entry.metadata();
        Self {
            entry: Some(entry),
            metadata: PortableStoryMetadata {
                key: metadata.key().as_str().to_owned(),
                name: metadata.name().as_str().to_owned(),
                section: metadata
                    .section()
                    .map(|section| section.as_str().to_owned()),
                crate_name: metadata.crate_name().to_owned(),
                crate_dir: metadata.crate_dir().to_owned(),
                source_file: metadata.source_file().to_owned(),
                source_line: metadata.source_line(),
                docs: entry.autodoc().docs().to_owned(),
            },
        }
    }

    /// Returns the serializable discovery metadata.
    pub fn metadata(&self) -> &PortableStoryMetadata {
        &self.metadata
    }

    /// Returns the stable story key.
    pub fn key(&self) -> &str {
        &self.metadata.key
    }

    /// Returns the registered Rust story name.
    pub fn name(&self) -> &str {
        &self.metadata.name
    }

    /// Returns the source package.
    pub fn crate_name(&self) -> &str {
        &self.metadata.crate_name
    }

    /// Returns the executable inventory entry, when this descriptor came from
    /// [`discover_stories`].
    pub fn entry(&self) -> Option<&'static StoryEntry> {
        self.entry
    }

    #[cfg(test)]
    pub(crate) fn for_test(key: &str, name: &str) -> Self {
        Self {
            entry: None,
            metadata: PortableStoryMetadata {
                key: key.to_owned(),
                name: name.to_owned(),
                section: None,
                crate_name: "test".to_owned(),
                crate_dir: "/tmp/test".to_owned(),
                source_file: "test.rs".to_owned(),
                source_line: 1,
                docs: String::new(),
            },
        }
    }
}

/// Discovers inventory stories sorted by stable key and source location.
pub fn discover_stories() -> Vec<StoryDescriptor> {
    let mut stories = inventory::iter::<StoryEntry>()
        .map(StoryDescriptor::from_entry)
        .collect::<Vec<_>>();
    stories.sort_by(|left, right| {
        left.metadata
            .key
            .cmp(&right.metadata.key)
            .then_with(|| left.metadata.source_file.cmp(&right.metadata.source_file))
            .then_with(|| left.metadata.source_line.cmp(&right.metadata.source_line))
    });
    stories
}

/// Discovers stories and rejects duplicate global keys before execution.
pub fn discover_stories_checked() -> Result<Vec<StoryDescriptor>, StorybookTestError> {
    let stories = discover_stories();
    for duplicate in stories.windows(2) {
        if duplicate[0].key() == duplicate[1].key() {
            return Err(StorybookTestError::DuplicateStoryKey {
                key: duplicate[0].key().to_owned(),
                first: duplicate_location(&duplicate[0]),
                second: duplicate_location(&duplicate[1]),
            });
        }
    }
    Ok(stories)
}

fn duplicate_location(story: &StoryDescriptor) -> String {
    format!(
        "{}:{} ({})",
        story.metadata.source_file, story.metadata.source_line, story.metadata.crate_name
    )
}
