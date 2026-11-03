//! Compact registration of application-owned routes.

use gpui_storybook_automation::{StoryDefaultSize, StoryScenarioSnapshot, StorySnapshot};

/// Metadata for an embedded application route. The key is also its capture route.
/// Applications own route selection and scenario fixture policy.
#[derive(Clone, Debug, bon::Builder)]
pub struct EmbeddedRoute {
    key: String,
    title: String,
    #[builder(default)]
    crate_name: String,
    #[builder(default)]
    story_name: String,
    #[builder(default)]
    description: String,
    group: Option<String>,
    section: Option<String>,
    #[builder(default)]
    source_file: String,
    #[builder(default)]
    source_line: u32,
    #[builder(default)]
    scenarios: Vec<StoryScenarioSnapshot>,
}

impl EmbeddedRoute {
    /// Register a route with its caller's source location and ordinary defaults.
    #[track_caller]
    pub fn new(key: impl Into<String>, title: impl Into<String>) -> Self {
        let source = std::panic::Location::caller();
        Self::builder()
            .key(key.into())
            .title(title.into())
            .source_file(source.file().to_owned())
            .source_line(source.line())
            .build()
    }

    /// Declare fresh-fixture scenarios owned by this route.
    pub fn with_scenarios(mut self, scenarios: Vec<StoryScenarioSnapshot>) -> Self {
        self.scenarios = scenarios;
        self
    }
}

impl From<EmbeddedRoute> for StorySnapshot {
    fn from(route: EmbeddedRoute) -> Self {
        Self {
            capture_route_id: route.key.clone(),
            key: route.key,
            title: route.title,
            crate_name: route.crate_name,
            story_name: route.story_name,
            description: route.description,
            group: route.group,
            section: route.section,
            source_file: route.source_file,
            source_line: route.source_line,
            default_size: StoryDefaultSize::default(),
            scenarios: route.scenarios,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compact_route_retains_identity_source_and_scenarios() {
        let scenario = StoryScenarioSnapshot::new("edit", "Edit");
        let expected_line = line!() + 1;
        let route = EmbeddedRoute::new("account", "Account").with_scenarios(vec![scenario.clone()]);
        let snapshot: StorySnapshot = route.into();
        assert_eq!(snapshot.key, "account");
        assert_eq!(snapshot.capture_route_id, snapshot.key);
        assert_eq!(snapshot.source_file, file!());
        assert_eq!(snapshot.source_line, expected_line);
        assert_eq!(snapshot.scenarios, vec![scenario]);
    }
}
