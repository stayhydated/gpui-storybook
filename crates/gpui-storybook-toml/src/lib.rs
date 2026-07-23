//! Loader and schema boundary for `storybook.toml`.
//!
//! This crate deliberately does not know about GPUI, inventory, story
//! containers, or runtime config selection. It only loads a config file from a
//! directory, deserializes the schema, exposes deterministic preference
//! overrides, and evaluates group/story filters for a caller-supplied
//! candidate.
//!
//! `gpui-storybook` is the crate that decides which config is the active
//! runtime config for a process and whether a candidate group comes from a
//! story crate's `group` or from a story's declared section.

use std::path::{Path, PathBuf};

use serde::Deserialize;

/// File name loaded by [`load_from_dir`].
pub const STORYBOOK_TOML_FILE_NAME: &str = "storybook.toml";

/// Effective light/dark scheme selected by a TOML preference override.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum StorybookColorScheme {
    /// Force light appearance.
    Light,
    /// Force dark appearance.
    Dark,
}

/// Deterministic runtime preference overrides from `storybook.toml`.
///
/// Every field is optional. These values affect effective presentation for the
/// current launch without replacing saved user intent.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct StorybookPreferenceOverrides {
    /// Effective light/dark scheme override.
    #[serde(default)]
    pub color_scheme: Option<StorybookColorScheme>,
    /// Effective registered theme name override.
    #[serde(default)]
    pub theme: Option<String>,
    /// Effective BCP 47 language override.
    #[serde(default)]
    pub language: Option<String>,
}

/// Parsed `storybook.toml` schema.
///
/// The `group` key has no serde default, so it is required when the file
/// exists. `allow` defaults to `None`, which means only the config's own
/// normalized group is allowed. `disable_story` defaults to an empty denylist,
/// and `overrides` defaults to no preference overrides.
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
    /// Deterministic effective preference overrides for the runtime config.
    #[serde(default)]
    pub overrides: StorybookPreferenceOverrides,
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
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};
    use std::{error::Error as _, path::Path};

    static NEXT_TEMP_DIR: AtomicU64 = AtomicU64::new(0);

    fn with_temp_dir(test_fn: impl FnOnce(&Path)) {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should be monotonic")
            .as_nanos();
        let sequence = NEXT_TEMP_DIR.fetch_add(1, Ordering::Relaxed);
        let dir =
            std::env::temp_dir().join(format!("gpui_storybook_toml_tests_{timestamp}_{sequence}"));

        std::fs::create_dir(&dir).expect("should create temp dir");
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
    fn preference_overrides_parse_as_optional_nested_values() {
        with_temp_dir(|dir| {
            std::fs::write(
                dir.join(STORYBOOK_TOML_FILE_NAME),
                concat!(
                    "group = \"Examples\"\n",
                    "[overrides]\n",
                    "color_scheme = \"dark\"\n",
                    "theme = \"Midnight\"\n",
                    "language = \"fr-CA\"\n",
                ),
            )
            .expect("should write config file");

            let config = load_from_dir(dir)
                .expect("valid config should parse")
                .expect("config should exist");

            assert_eq!(
                config.overrides,
                StorybookPreferenceOverrides {
                    color_scheme: Some(StorybookColorScheme::Dark),
                    theme: Some("Midnight".to_owned()),
                    language: Some("fr-CA".to_owned()),
                }
            );
        });
    }

    #[test]
    fn invalid_preference_override_is_a_parse_error() {
        with_temp_dir(|dir| {
            std::fs::write(
                dir.join(STORYBOOK_TOML_FILE_NAME),
                "group = \"Examples\"\n[overrides]\ncolor_scheme = \"sepia\"\n",
            )
            .expect("should write config file");

            let error = load_from_dir(dir).expect_err("unknown scheme should fail parsing");
            assert!(error.to_string().contains("unknown variant `sepia`"));
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

    #[test]
    fn filters_normalize_blank_and_whitespace_groups() {
        let config = StorybookToml {
            group: "  Examples  ".to_string(),
            allow: None,
            disable_story: Vec::new(),
            overrides: StorybookPreferenceOverrides::default(),
        };

        assert_eq!(config.group(), Some("Examples"));
        assert!(config.allows_group(Some("  Examples ")));
        assert!(!config.allows_group(None));
        assert!(!config.allows_group(Some("   ")));

        let blank = StorybookToml {
            group: "   ".to_string(),
            ..StorybookToml::default()
        };
        assert_eq!(blank.group(), None);
        assert!(!blank.allows_group(None));

        let allow = StorybookToml {
            group: "Examples".to_string(),
            allow: Some(vec![" Other ".to_string()]),
            disable_story: Vec::new(),
            overrides: StorybookPreferenceOverrides::default(),
        };
        assert!(allow.allows_group(Some(" Other ")));
    }

    #[test]
    fn parse_errors_preserve_path_and_source() {
        with_temp_dir(|dir| {
            let path = dir.join(STORYBOOK_TOML_FILE_NAME);
            std::fs::write(&path, "unknown = true\n").expect("should write invalid config");

            let error = load_from_dir(dir).expect_err("invalid config should fail parsing");
            assert!(error.to_string().contains(&path.display().to_string()));
            assert!(error.source().is_some());
            assert!(
                matches!(error, StorybookTomlError::Parse { path: error_path, .. } if error_path == path)
            );
        });
    }

    #[test]
    fn read_errors_preserve_path_and_source() {
        with_temp_dir(|dir| {
            let path = dir.join(STORYBOOK_TOML_FILE_NAME);
            std::fs::create_dir(&path).expect("should create unreadable config path");

            let error = load_from_dir(dir).expect_err("a directory should not parse as a file");
            assert!(error.to_string().contains(&path.display().to_string()));
            assert!(error.source().is_some());
            assert!(
                matches!(error, StorybookTomlError::Read { path: error_path, .. } if error_path == path)
            );
        });
    }
}
