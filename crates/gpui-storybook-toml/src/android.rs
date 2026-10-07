//! Application-owned Android launch metadata.

use serde::Deserialize;
use std::{collections::BTreeMap, path::PathBuf};

/// Application launch configuration. Paths resolve against its directory;
/// the invoking command explicitly selects the device.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct AndroidApplication {
    pub package: String,
    pub activity: String,
    #[serde(default = "default_port")]
    pub device_port: u16,
    #[serde(default)]
    pub apk: Option<PathBuf>,
    /// Application-owned semantic smoke plan, relative to this configuration.
    #[serde(default)]
    pub smoke_plan: Option<PathBuf>,
    #[serde(default)]
    pub boolean_extras: BTreeMap<String, bool>,
    #[serde(default)]
    pub string_extras: BTreeMap<String, String>,
    /// Application build hook, executed in the configuration directory.
    #[serde(default)]
    pub build: Vec<String>,
}
fn default_port() -> u16 {
    28437
}
impl AndroidApplication {
    pub fn validate(&self) -> Result<(), &'static str> {
        let identifier = |value: &str| {
            !value.is_empty()
                && value.len() <= 256
                && value.split('.').all(|part| {
                    !part.is_empty()
                        && part
                            .bytes()
                            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
                })
        };
        if !identifier(&self.package)
            || !identifier(self.activity.trim_start_matches('.'))
            || self.device_port == 0
        {
            return Err("Android package, activity, and nonzero device port must be valid");
        }
        if self.boolean_extras.len() + self.string_extras.len() > 32
            || self
                .boolean_extras
                .keys()
                .chain(self.string_extras.keys())
                .any(|key| key.is_empty() || key.len() > 128 || key.contains(['\0', '\r', '\n']))
            || self
                .string_extras
                .values()
                .any(|value| value.len() > 4096 || value.contains('\0'))
            || self.build.len() > 64
            || self.build.iter().any(|argument| {
                argument.is_empty() || argument.len() > 4096 || argument.contains('\0')
            })
        {
            return Err("Android launch extras or build arguments exceed their bounds");
        }
        Ok(())
    }
    pub fn component(&self) -> String {
        format!("{}/{}", self.package, self.activity)
    }
    /// Deterministic arguments for one launch; the host never replays it.
    pub fn launch_arguments(&self) -> Vec<String> {
        let mut arguments = ["am", "start", "-W", "-S", "-n"]
            .map(str::to_owned)
            .to_vec();
        arguments.push(self.component());
        for (key, value) in &self.boolean_extras {
            arguments.extend(["--ez".to_owned(), key.clone(), value.to_string()]);
        }
        for (key, value) in &self.string_extras {
            arguments.extend(["--es".to_owned(), key.clone(), value.clone()]);
        }
        arguments
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn launch_preserves_consumer_identity_and_typed_extras() {
        let app: AndroidApplication = toml::from_str(
            r#"
package = "com.example.production"
activity = ".MainActivity"
device_port = 28438
[boolean_extras]
example_automation = true
[string_extras]
example_fixture = "account preview"
"#,
        )
        .unwrap();
        assert_eq!(app.validate(), Ok(()));
        assert_eq!(
            app.launch_arguments(),
            [
                "am",
                "start",
                "-W",
                "-S",
                "-n",
                "com.example.production/.MainActivity",
                "--ez",
                "example_automation",
                "true",
                "--es",
                "example_fixture",
                "account preview"
            ]
        );
        let mut invalid = app;
        invalid.package = "-bad".to_owned();
        assert!(invalid.validate().is_err());
    }
}
