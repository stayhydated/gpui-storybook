use super::*;
use crate::{capture::storybook_capture_env, tools::*};
use component_shape_mcp::{
    McpSchema, McpServer, tool_call_structured_content, tool_structured_result,
};
use serde_json::{Value, json};
use std::{env, ffi::OsString, path::PathBuf, sync::Mutex};

static ENV_LOCK: Mutex<()> = Mutex::new(());

struct EnvGuard(Vec<(String, Option<OsString>)>);

impl EnvGuard {
    fn set(vars: &[(&str, &str)]) -> Self {
        let previous = vars
            .iter()
            .map(|(name, _)| ((*name).to_string(), env::var_os(name)))
            .collect();

        unsafe {
            for (name, value) in vars {
                env::set_var(name, value);
            }
        }

        Self(previous)
    }

    fn remove(names: &[&str]) -> Self {
        let previous = names
            .iter()
            .map(|name| ((*name).to_string(), env::var_os(name)))
            .collect();

        unsafe {
            for name in names {
                env::remove_var(name);
            }
        }

        Self(previous)
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        unsafe {
            for (name, value) in self.0.drain(..) {
                if let Some(value) = value {
                    env::set_var(name, value);
                } else {
                    env::remove_var(name);
                }
            }
        }
    }
}

fn sample_story() -> StorySnapshot {
    StorySnapshot {
        key: "example-ButtonStory".to_string(),
        crate_name: "example".to_string(),
        story_name: "ButtonStory".to_string(),
        title: "Button".to_string(),
        description: "Button states".to_string(),
        group: Some("Inputs".to_string()),
        section: None,
        source_file: "src/stories/button.rs".to_string(),
        source_line: 12,
        capture_route_id: "example-ButtonStory".to_string(),
        default_size: StoryDefaultSize::default(),
        scenarios: Vec::new(),
    }
}

mod capture_tests;
mod tool_tests;
