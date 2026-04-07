use serde::Deserialize;
use std::path::{Path, PathBuf};

pub const STORYBOOK_TOML_FILE_NAME: &str = "storybook.toml";

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct StorybookToml {
    pub group: String,
    #[serde(default)]
    pub allow: Vec<String>,
}

impl StorybookToml {
    pub fn allows(&self, story_name: &str) -> bool {
        self.allow
            .iter()
            .any(|allowed_story| allowed_story == "*" || allowed_story == story_name)
    }

    pub fn group(&self) -> Option<&str> {
        Some(self.group.as_str())
            .map(str::trim)
            .filter(|group| !group.is_empty())
    }
}

#[derive(Debug)]
pub enum StorybookTomlError {
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
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

pub fn load_from_dir(dir: impl AsRef<Path>) -> Result<Option<StorybookToml>, StorybookTomlError> {
    let path = dir.as_ref().join(STORYBOOK_TOML_FILE_NAME);

    let raw = match std::fs::read_to_string(&path) {
        Ok(raw) => raw,
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(source) => {
            return Err(StorybookTomlError::Read {
                path: path.to_path_buf(),
                source,
            });
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

            assert!(config.allows("ButtonStory"));
            assert!(config.allows("TableStory"));
            assert_eq!(config.group(), Some("Examples"));
        });
    }

    #[test]
    fn allow_list_filters_to_explicit_story_names() {
        with_temp_dir(|dir| {
            std::fs::write(
                dir.join(STORYBOOK_TOML_FILE_NAME),
                "group = \"Examples\"\nallow = [\"ButtonStory\", \"HelloStory\"]\n",
            )
            .expect("should write config file");

            let config = load_from_dir(dir)
                .expect("valid config should parse")
                .expect("config should exist");

            assert!(config.allows("ButtonStory"));
            assert!(config.allows("HelloStory"));
            assert!(!config.allows("TableStory"));
        });
    }

    #[test]
    fn allow_defaults_to_empty_when_not_provided() {
        with_temp_dir(|dir| {
            std::fs::write(dir.join(STORYBOOK_TOML_FILE_NAME), "group = \"Examples\"\n")
                .expect("should write config file");

            let config = load_from_dir(dir)
                .expect("valid config should parse")
                .expect("config should exist");

            assert_eq!(config.allow, Vec::<String>::new());
            assert!(!config.allows("ButtonStory"));
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

            assert!(!config.allows("ButtonStory"));
            assert!(!config.allows("AnyStory"));
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
