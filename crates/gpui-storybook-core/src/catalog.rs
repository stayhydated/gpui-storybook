//! Static Storybook catalog and registration autodocumentation.
//!
//! The static catalog is built from [`crate::registry::StoryEntry`] values and
//! never constructs a story. It therefore contains registration identity,
//! source provenance, Rust documentation, and control shape metadata. Runtime
//! values such as localized titles, localized descriptions, and control
//! defaults remain properties of the live story instance and are intentionally
//! not represented here.

use crate::registry::StoryEntry;
use schemars::JsonSchema;
use serde::Serialize;
use std::{fs::File, io::Write, path::Path};
use thiserror::Error;

/// Errors returned while rendering or writing a static story catalog.
#[derive(Debug, Error)]
pub enum StoryCatalogExportError {
    /// JSON serialization failed.
    #[error("failed to serialize story catalog: {0}")]
    Serialize(#[from] serde_json::Error),
    /// Writing rendered JSON to the requested destination failed.
    #[error("failed to write story catalog: {0}")]
    Io(#[from] std::io::Error),
}

/// The editor kind captured for a control without constructing its story.
///
/// This mirrors the serializable names used by [`crate::controls::ControlKind`]
/// while remaining usable in compile-time inventory initializers.
#[derive(Clone, Copy, Debug, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StaticControlKind {
    /// A boolean checkbox.
    Checkbox,
    /// A numeric input without explicit range bounds.
    Number,
    /// A numeric input with at least one explicit range bound.
    Range,
    /// A free-form text input.
    Text,
    /// A color picker for an `Hsla` value.
    ColorPicker,
    /// A finite set of string options.
    Select,
    /// A custom control kind supplied by an integration.
    Custom(&'static str),
}

/// Static metadata for one marked story control.
///
/// Control defaults are omitted because they are read from a concrete story
/// instance at runtime. The remaining fields are derived from the field type
/// and `#[storybook(control(...))]` attributes and are safe to export from a
/// static catalog.
#[derive(Clone, Copy, Debug, JsonSchema, PartialEq, Serialize)]
pub struct StaticControlSpec {
    /// Stable field key used by controls and automation.
    pub key: &'static str,
    /// Human-facing label supplied by the registration declaration.
    pub label: &'static str,
    /// Human-facing help text supplied by the registration declaration.
    pub description: &'static str,
    /// Human-facing category supplied by the registration declaration.
    pub category: &'static str,
    /// Editor kind inferred from the field type and options.
    pub kind: StaticControlKind,
    /// Numeric bounds that can be evaluated without a story instance.
    pub bounds: crate::controls::ControlBounds,
    /// String choices supplied by the registration declaration.
    pub options: &'static [&'static str],
}

impl StaticControlSpec {
    /// Creates static metadata for a marked story control.
    pub const fn new(
        key: &'static str,
        label: &'static str,
        description: &'static str,
        category: &'static str,
        kind: StaticControlKind,
        bounds: crate::controls::ControlBounds,
        options: &'static [&'static str],
    ) -> Self {
        Self {
            key,
            label,
            description,
            category,
            kind,
            bounds,
            options,
        }
    }
}

/// Source provenance included in a static story catalog entry.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize)]
pub struct StoryCatalogSource {
    /// Cargo package name that registered the story.
    pub crate_name: String,
    /// Manifest directory of the registering package.
    pub crate_dir: String,
    /// Source file recorded at the registration site.
    pub file: String,
    /// Source line recorded at the registration site.
    pub line: u32,
}

/// One story entry exported by the static catalog.
///
/// `name` and `key` are registration identities and are never localized. A
/// runtime Storybook may display a different localized title while retaining
/// these values for automation, filtering, and capture routes.
#[derive(Clone, Debug, JsonSchema, PartialEq, Serialize)]
pub struct StoryCatalogEntry {
    /// Globally stable automation and capture key.
    pub key: String,
    /// Stable registered story type name.
    pub name: String,
    /// Declared story section, if any.
    pub section: Option<String>,
    /// Enum-derived section ordering, if any.
    pub section_order: Option<usize>,
    /// Registration source provenance.
    pub source: StoryCatalogSource,
    /// Rust documentation captured from the story declaration.
    pub docs: String,
    /// Static control shape metadata. Defaults are runtime-only.
    pub controls: Vec<StaticControlSpec>,
}

impl StoryCatalogEntry {
    fn from_entry(entry: &StoryEntry) -> Self {
        let metadata = entry.metadata();
        let autodoc = entry.autodoc();

        Self {
            key: metadata.key().as_str().to_owned(),
            name: metadata.name().as_str().to_owned(),
            section: metadata
                .section()
                .map(|section| section.as_str().to_owned()),
            section_order: entry.section_order,
            source: StoryCatalogSource {
                crate_name: metadata.crate_name().to_owned(),
                crate_dir: metadata.crate_dir().to_owned(),
                file: metadata.source_file().to_owned(),
                line: metadata.source_line(),
            },
            docs: autodoc.docs().to_owned(),
            controls: autodoc.controls().to_vec(),
        }
    }
}

impl From<&StoryEntry> for StoryCatalogEntry {
    fn from(entry: &StoryEntry) -> Self {
        Self::from_entry(entry)
    }
}

/// Deterministic static catalog of all inventory-registered stories.
///
/// Entries are sorted by stable key, then by source provenance to make JSON
/// output reproducible even when registration order differs between linkers.
/// The catalog intentionally preserves duplicate keys so tooling can report
/// the same diagnostic as runtime generation instead of silently dropping an
/// entry.
#[derive(Clone, Debug, Default, JsonSchema, PartialEq, Serialize)]
pub struct StoryCatalog {
    /// Registered story entries in deterministic order.
    pub stories: Vec<StoryCatalogEntry>,
}

impl StoryCatalog {
    /// Builds a catalog from the supplied registration entries.
    pub fn from_entries<'a>(entries: impl IntoIterator<Item = &'a StoryEntry>) -> Self {
        let mut stories = entries
            .into_iter()
            .map(StoryCatalogEntry::from_entry)
            .collect::<Vec<_>>();
        stories.sort_by(|left, right| {
            left.key
                .cmp(&right.key)
                .then_with(|| left.name.cmp(&right.name))
                .then_with(|| left.section.cmp(&right.section))
                .then_with(|| left.section_order.cmp(&right.section_order))
                .then_with(|| left.source.crate_name.cmp(&right.source.crate_name))
                .then_with(|| left.source.file.cmp(&right.source.file))
                .then_with(|| left.source.line.cmp(&right.source.line))
                .then_with(|| left.docs.cmp(&right.docs))
                .then_with(|| {
                    serde_json::to_string(&left.controls)
                        .expect("static control metadata should serialize")
                        .cmp(
                            &serde_json::to_string(&right.controls)
                                .expect("static control metadata should serialize"),
                        )
                })
        });
        Self { stories }
    }

    /// Builds a catalog from all stories submitted to the process registry.
    pub fn from_registry() -> Self {
        Self::from_entries(inventory::iter::<StoryEntry>())
    }

    /// Returns the catalog entries in deterministic order.
    pub fn entries(&self) -> &[StoryCatalogEntry] {
        &self.stories
    }

    /// Serializes this catalog as compact deterministic JSON.
    pub fn to_json(&self) -> Result<String, StoryCatalogExportError> {
        Ok(serde_json::to_string(self)?)
    }

    /// Serializes this catalog as pretty deterministic JSON.
    pub fn to_json_pretty(&self) -> Result<String, StoryCatalogExportError> {
        Ok(serde_json::to_string_pretty(self)?)
    }

    /// Writes compact deterministic JSON to an arbitrary writer.
    pub fn write_json<W: Write>(&self, writer: &mut W) -> Result<(), StoryCatalogExportError> {
        writer.write_all(self.to_json()?.as_bytes())?;
        Ok(())
    }

    /// Writes pretty deterministic JSON to an arbitrary writer.
    pub fn write_json_pretty<W: Write>(
        &self,
        writer: &mut W,
    ) -> Result<(), StoryCatalogExportError> {
        writer.write_all(self.to_json_pretty()?.as_bytes())?;
        Ok(())
    }

    /// Writes compact deterministic JSON to a file without opening a story
    /// window.
    pub fn write_json_file(&self, path: impl AsRef<Path>) -> Result<(), StoryCatalogExportError> {
        let mut file = File::create(path)?;
        self.write_json(&mut file)
    }

    /// Writes pretty deterministic JSON to a file without opening a story
    /// window.
    pub fn write_json_file_pretty(
        &self,
        path: impl AsRef<Path>,
    ) -> Result<(), StoryCatalogExportError> {
        let mut file = File::create(path)?;
        self.write_json_pretty(&mut file)
    }
}

/// Collects all statically registered story metadata without constructing a
/// live GPUI story.
pub fn static_story_catalog() -> StoryCatalog {
    StoryCatalog::from_registry()
}

/// Exports all statically registered story metadata as compact JSON.
pub fn static_story_catalog_json() -> Result<String, StoryCatalogExportError> {
    static_story_catalog().to_json()
}

/// Exports all statically registered story metadata as pretty JSON.
pub fn static_story_catalog_json_pretty() -> Result<String, StoryCatalogExportError> {
    static_story_catalog().to_json_pretty()
}

/// Renders the process-wide static story catalog as compact JSON.
pub fn export_static_catalog_json<W: Write>(writer: &mut W) -> Result<(), StoryCatalogExportError> {
    static_story_catalog().write_json(writer)
}

/// Renders the process-wide static story catalog as pretty JSON.
pub fn export_static_catalog_json_pretty<W: Write>(
    writer: &mut W,
) -> Result<(), StoryCatalogExportError> {
    static_story_catalog().write_json_pretty(writer)
}

/// Writes the process-wide static story catalog as compact JSON to a file.
pub fn write_static_catalog_json(path: impl AsRef<Path>) -> Result<(), StoryCatalogExportError> {
    static_story_catalog().write_json_file(path)
}

/// Writes the process-wide static story catalog as pretty JSON to a file.
pub fn write_static_catalog_json_pretty(
    path: impl AsRef<Path>,
) -> Result<(), StoryCatalogExportError> {
    static_story_catalog().write_json_file_pretty(path)
}

#[cfg(test)]
mod tests {
    use super::{StaticControlKind, StaticControlSpec, StoryCatalog, static_story_catalog_json};
    use crate::{
        controls::ControlBounds,
        registry::{StoryAutodoc, StoryEntry, StoryRegistrationSource},
        story::StoryContainer,
    };

    fn unused_create_fn(
        _: &mut ::gpui_kit::Window,
        _: &mut ::gpui_kit::App,
    ) -> ::gpui_kit::Entity<StoryContainer> {
        unreachable!("story creation is not used in this test");
    }

    struct FailingWriter;

    impl std::io::Write for FailingWriter {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("catalog test writer failed"))
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    static CONTROLS: &[StaticControlSpec] = &[StaticControlSpec::new(
        "disabled",
        "Disabled",
        "Disable interaction",
        "State",
        StaticControlKind::Checkbox,
        ControlBounds {
            min: None,
            max: None,
            step: None,
        },
        &[],
    )];

    static ENTRY: StoryEntry = StoryEntry::new(
        "catalog-ZStory",
        "ZStory",
        Some("Components"),
        Some(2),
        unused_create_fn,
        StoryRegistrationSource::new("catalog", "/tmp/catalog", "src/z.rs", 12),
    )
    .with_autodoc(StoryAutodoc::new("A story docs.", CONTROLS));

    static FIRST_ENTRY: StoryEntry = StoryEntry::new(
        "catalog-AStory",
        "AStory",
        None,
        None,
        unused_create_fn,
        StoryRegistrationSource::new("catalog", "/tmp/catalog", "src/a.rs", 4),
    );

    #[test]
    fn catalog_sorts_by_stable_key_and_keeps_static_metadata() {
        let catalog = StoryCatalog::from_entries([&ENTRY, &FIRST_ENTRY]);

        assert_eq!(catalog.entries()[0].key, "catalog-AStory");
        assert_eq!(catalog.entries()[1].key, "catalog-ZStory");
        assert_eq!(catalog.entries()[1].name, "ZStory");
        assert_eq!(catalog.entries()[1].section.as_deref(), Some("Components"));
        assert_eq!(catalog.entries()[1].section_order, Some(2));
        assert_eq!(catalog.entries()[1].docs, "A story docs.");
        assert_eq!(catalog.entries()[1].controls[0].key, "disabled");
        assert_eq!(
            catalog.entries()[1].controls[0].kind,
            StaticControlKind::Checkbox
        );
    }

    #[test]
    fn catalog_json_is_stable_and_has_explicit_source_and_docs_fields() {
        let catalog = StoryCatalog::from_entries([&ENTRY]);
        let json = catalog
            .to_json()
            .expect("catalog metadata should serialize");
        let repeated = catalog
            .to_json()
            .expect("catalog metadata should serialize repeatedly");

        assert_eq!(json, repeated);
        assert!(json.contains("\"key\":\"catalog-ZStory\""));
        assert!(json.contains("\"source\":{\"crate_name\":\"catalog\""));
        assert!(json.contains("\"docs\":\"A story docs.\""));
        assert!(json.contains("\"controls\":[{\"key\":\"disabled\""));
    }

    #[test]
    fn static_json_export_is_available_without_live_story_construction() {
        let json = static_story_catalog_json().expect("inventory catalog should serialize");
        assert!(json.starts_with("{\"stories\":["));
    }

    #[test]
    fn catalog_writer_preserves_io_errors_in_typed_export_result() {
        let catalog = StoryCatalog::from_entries([&ENTRY]);
        let error = catalog
            .write_json(&mut FailingWriter)
            .expect_err("failing writer should be reported");

        assert!(matches!(
            error,
            super::StoryCatalogExportError::Io(error)
                if error.kind() == std::io::ErrorKind::Other
        ));
    }
}
