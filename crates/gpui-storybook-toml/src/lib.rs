//! Loader and schema boundary for `storybook.toml`.
//!
//! This crate deliberately does not know about GPUI, inventory, story
//! containers, or runtime config selection. It only loads a config file from a
//! directory, deserializes the schema, and evaluates group/story filters for a
//! caller-supplied candidate.
//!
//! `gpui-storybook` is the crate that decides which config is the active
//! runtime config for a process and whether a candidate group comes from a
//! story crate's `group` or from a story's declared section.

use serde::Deserialize;
use std::path::{Path, PathBuf};

/// File name loaded by [`load_from_dir`].
pub const STORYBOOK_TOML_FILE_NAME: &str = "storybook.toml";

/// Parsed `storybook.toml` schema.
///
/// The `group` key has no serde default, so it is required when the file
/// exists. `allow` defaults to `None`, which means only the config's own
/// normalized group is allowed. `disable_story` defaults to an empty denylist.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct StorybookToml {
    /// Top-level group for stories from the crate that owns this config.
    pub group: String,
    /// Optional runtime group allowlist. `["*"]` allows every group.
    #[serde(default)]
    pub allow: Option<Vec<String>>,
    /// Registered story type names to hide.
    #[serde(default)]
    pub disable_story: Vec<String>,
}

impl StorybookToml {
    /// Returns whether the supplied group is allowed by this config.
    ///
    /// Candidate groups and allow entries are trimmed before comparison. When
    /// `allow` is omitted, only this config's own normalized [`group`](Self::group)
    /// is allowed. When `allow` is present, `"*"` allows every group and an
    /// empty list allows none.
    pub fn allows_group(&self, group: Option<&str>) -> bool {
        let group = group.map(str::trim).filter(|group| !group.is_empty());
        let self_group = self.group();

        let Some(allow_list) = self.allow.as_ref() else {
            return matches!((group, self_group), (Some(group), Some(self_group)) if group == self_group);
        };

        allow_list.iter().any(|allowed| {
            let allowed = allowed.trim();
            allowed == "*" || group.is_some_and(|group_name| allowed == group_name)
        })
    }

    /// Returns whether `story_name` exactly matches a disabled story name.
    pub fn is_story_disabled(&self, story_name: &str) -> bool {
        self.disable_story
            .iter()
            .any(|disabled| disabled == story_name)
    }

    /// Returns the normalized group, or `None` if it is blank.
    pub fn group(&self) -> Option<&str> {
        Some(str::trim(self.group.as_str())).filter(|group| !group.is_empty())
    }
}

/// Errors produced while loading or parsing `storybook.toml`.
#[derive(Debug)]
pub enum StorybookTomlError {
    /// The file existed but could not be read.
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    /// The file contents were not valid for the schema.
    Parse {
        path: PathBuf,
        source: toml::de::Error,
    },
}

impl std::fmt::Display for StorybookTomlError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StorybookTomlError::Read { path, source } => {
                write!(f, "failed to read {}: {source}", path.display())
            },
            StorybookTomlError::Parse { path, source } => {
                write!(f, "failed to parse {}: {source}", path.display())
            },
        }
    }
}

impl std::error::Error for StorybookTomlError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            StorybookTomlError::Read { source, .. } => Some(source),
            StorybookTomlError::Parse { source, .. } => Some(source),
        }
    }
}

/// Loads `<dir>/storybook.toml`.
///
/// Missing files return `Ok(None)`. Read and parse failures preserve the full
/// path in [`StorybookTomlError`] so callers can log actionable diagnostics.
pub fn load_from_dir(dir: impl AsRef<Path>) -> Result<Option<StorybookToml>, StorybookTomlError> {
    let path = dir.as_ref().join(STORYBOOK_TOML_FILE_NAME);

    let raw = match std::fs::read_to_string(&path) {
        Ok(raw) => raw,
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(source) => {
            return Err(StorybookTomlError::Read { path, source });
        },
    };

    toml::from_str::<StorybookToml>(&raw)
        .map(Some)
        .map_err(|source| StorybookTomlError::Parse {
            path: path.to_path_buf(),
            source,
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn with_temp_dir(test_fn: impl FnOnce(&Path)) {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should be monotonic")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("gpui_storybook_toml_tests_{timestamp}"));

        std::fs::create_dir_all(&dir).expect("should create temp dir");
        test_fn(&dir);
        std::fs::remove_dir_all(&dir).expect("should remove temp dir");
    }

    #[test]
    fn missing_storybook_toml_returns_none() {
        with_temp_dir(|dir| {
            let config = load_from_dir(dir).expect("missing file should not error");
            assert!(config.is_none());
        });
    }

    #[test]
    fn wildcard_allow_includes_all_stories() {
        with_temp_dir(|dir| {
            std::fs::write(
                dir.join(STORYBOOK_TOML_FILE_NAME),
                "group = \"Examples\"\nallow = [\"*\"]\n",
            )
            .expect("should write config file");

            let config = load_from_dir(dir)
                .expect("valid config should parse")
                .expect("config should exist");

            assert!(config.allows_group(config.group()));
            assert_eq!(config.group(), Some("Examples"));
        });
    }

    #[test]
    fn allow_list_filters_to_explicit_groups() {
        with_temp_dir(|dir| {
            std::fs::write(
                dir.join(STORYBOOK_TOML_FILE_NAME),
                "group = \"Examples\"\nallow = [\"Examples\", \"Other\"]\n",
            )
            .expect("should write config file");

            let config = load_from_dir(dir)
                .expect("valid config should parse")
                .expect("config should exist");

            assert!(config.allows_group(config.group()));
            assert!(!config.allows_group(Some("Unlisted")));
        });
    }

    #[test]
    fn allow_defaults_to_self_group_when_not_provided() {
        with_temp_dir(|dir| {
            std::fs::write(dir.join(STORYBOOK_TOML_FILE_NAME), "group = \"Examples\"\n")
                .expect("should write config file");

            let config = load_from_dir(dir)
                .expect("valid config should parse")
                .expect("config should exist");

            assert_eq!(config.allow, None);
            assert!(config.allows_group(config.group()));
            assert!(!config.allows_group(Some("OtherGroup")));
        });
    }

    #[test]
    fn empty_allow_list_disallows_everything() {
        with_temp_dir(|dir| {
            std::fs::write(
                dir.join(STORYBOOK_TOML_FILE_NAME),
                "group = \"Examples\"\nallow = []\n",
            )
            .expect("should write config file");

            let config = load_from_dir(dir)
                .expect("valid config should parse")
                .expect("config should exist");

            assert!(!config.allows_group(config.group()));
        });
    }

    #[test]
    fn group_name_in_allow_list_includes_group() {
        with_temp_dir(|dir| {
            std::fs::write(
                dir.join(STORYBOOK_TOML_FILE_NAME),
                "group = \"Examples\"\nallow = [\"Examples\"]\n",
            )
            .expect("should write config file");

            let config = load_from_dir(dir)
                .expect("valid config should parse")
                .expect("config should exist");

            assert!(config.allows_group(config.group()));
            assert!(!config.allows_group(Some("OtherGroup")));
        });
    }

    #[test]
    fn disable_story_filters_specific_story_names() {
        with_temp_dir(|dir| {
            std::fs::write(
                dir.join(STORYBOOK_TOML_FILE_NAME),
                "group = \"Examples\"\ndisable_story = [\"ButtonStory\"]\n",
            )
            .expect("should write config file");

            let config = load_from_dir(dir)
                .expect("valid config should parse")
                .expect("config should exist");

            assert!(config.is_story_disabled("ButtonStory"));
            assert!(!config.is_story_disabled("TableStory"));
        });
    }

    #[test]
    fn group_is_required_when_storybook_toml_exists() {
        with_temp_dir(|dir| {
            std::fs::write(dir.join(STORYBOOK_TOML_FILE_NAME), "allow = [\"*\"]\n")
                .expect("should write config file");

            let error = load_from_dir(dir).expect_err("missing group should fail parsing");
            assert!(error.to_string().contains("missing field `group`"));
        });
    }
}
