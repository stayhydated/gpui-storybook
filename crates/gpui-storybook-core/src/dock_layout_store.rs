use std::path::Path;

use anyhow::Result;
use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;

pub(super) struct DockLayoutStore;

impl DockLayoutStore {
    fn sanitize_state_json(value: &mut Value) {
        match value {
            Value::Object(map) => {
                for child in map.values_mut() {
                    Self::sanitize_state_json(child);
                }

                let is_tab_panel = map
                    .get("panel_name")
                    .and_then(Value::as_str)
                    .is_some_and(|name| name == "TabPanel");
                if !is_tab_panel {
                    return;
                }

                if let Some(info) = map.get_mut("info")
                    && info
                        .as_object()
                        .and_then(|info| info.get("panel"))
                        .is_some_and(Value::is_null)
                {
                    *info = serde_json::json!({
                        "tabs": {
                            "active_index": 0
                        }
                    });
                }
            },
            Value::Array(items) => {
                for item in items {
                    Self::sanitize_state_json(item);
                }
            },
            _ => {},
        }
    }

    pub(super) fn to_json<T: Serialize>(state: &T) -> Result<String> {
        let mut state_json = serde_json::to_value(state)?;
        Self::sanitize_state_json(&mut state_json);
        Ok(serde_json::to_string_pretty(&state_json)?)
    }

    pub(super) fn sanitize_state<T>(state: T) -> Result<T>
    where
        T: Serialize + DeserializeOwned,
    {
        let mut state_json = serde_json::to_value(state)?;
        Self::sanitize_state_json(&mut state_json);
        Ok(serde_json::from_value(state_json)?)
    }

    pub(super) fn from_json<T: DeserializeOwned>(json: &str) -> Result<T> {
        let mut state_json = serde_json::from_str::<Value>(json)?;
        Self::sanitize_state_json(&mut state_json);
        Ok(serde_json::from_value(state_json)?)
    }

    pub(super) fn save_to_path<T: Serialize>(path: impl AsRef<Path>, state: &T) -> Result<()> {
        let json = Self::to_json(state)?;
        let path = path.as_ref();
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            std::fs::create_dir_all(parent)?;
        }

        let mut temporary_path = path.as_os_str().to_owned();
        temporary_path.push(".tmp");
        let temporary_path = Path::new(&temporary_path);
        std::fs::write(temporary_path, json)?;
        std::fs::rename(temporary_path, path)?;
        Ok(())
    }

    pub(super) fn load_from_path<T: DeserializeOwned>(path: impl AsRef<Path>) -> Result<T> {
        let json = std::fs::read_to_string(path)?;
        Self::from_json(&json)
    }
}

#[cfg(test)]
mod tests {
    use std::{
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
    };

    use serde_json::{Value, json};

    use super::DockLayoutStore;

    static NEXT_PATH_ID: AtomicU64 = AtomicU64::new(0);

    fn temporary_path(name: &str) -> PathBuf {
        let id = NEXT_PATH_ID.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "gpui-storybook-{name}-{}-{id}.json",
            std::process::id()
        ))
    }

    #[test]
    fn sanitizes_nested_null_tab_panels_without_changing_other_panels() {
        let mut state = json!({
            "root": [
                {
                    "panel_name": "TabPanel",
                    "info": { "panel": null }
                },
                {
                    "panel_name": "TabPanel",
                    "info": { "panel": { "name": "kept" } }
                },
                {
                    "panel_name": "StackPanel",
                    "info": { "panel": null }
                }
            ]
        });

        DockLayoutStore::sanitize_state_json(&mut state);

        assert_eq!(
            state["root"][0]["info"],
            json!({ "tabs": { "active_index": 0 } })
        );
        assert_eq!(state["root"][1]["info"]["panel"]["name"], "kept");
        assert!(state["root"][2]["info"]["panel"].is_null());
    }

    #[test]
    fn typed_json_paths_share_the_same_sanitization_contract() {
        let state = json!({
            "panel_name": "TabPanel",
            "info": { "panel": null }
        });

        let sanitized: Value = DockLayoutStore::sanitize_state(state.clone()).unwrap();
        let encoded = DockLayoutStore::to_json(&state).unwrap();
        let decoded: Value = DockLayoutStore::from_json(&encoded).unwrap();

        let expected = json!({
            "panel_name": "TabPanel",
            "info": { "tabs": { "active_index": 0 } }
        });
        assert_eq!(sanitized, expected);
        assert_eq!(decoded, expected);
    }

    #[test]
    fn saves_and_loads_atomically_from_nested_directories() {
        let path = temporary_path("atomic").join("nested").join("layout.json");
        let state = json!({ "version": 5, "items": [1, 2, 3] });

        DockLayoutStore::save_to_path(&path, &state).unwrap();
        let loaded: Value = DockLayoutStore::load_from_path(&path).unwrap();

        assert_eq!(loaded, state);
        let mut temporary_path = path.as_os_str().to_owned();
        temporary_path.push(".tmp");
        assert!(!PathBuf::from(temporary_path).exists());
        std::fs::remove_dir_all(path.ancestors().nth(2).unwrap()).unwrap();
    }

    #[test]
    fn reports_invalid_json_and_missing_files() {
        assert!(DockLayoutStore::from_json::<Value>("{").is_err());
        assert!(DockLayoutStore::load_from_path::<Value>(temporary_path("missing")).is_err());
    }
}
