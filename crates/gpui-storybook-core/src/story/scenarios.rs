//! Story-owned, repeatable interaction scenarios.
//!
//! A scenario is a named [`StoryInteractionRequest`](crate::automation::StoryInteractionRequest)
//! template. Story authors keep the initial control values, presentation, ordered
//! named steps, semantic postconditions, and optional final capture together so
//! the same workflow can be listed by the Storybook UI, driven by MCP, or used
//! by another automation host.

use crate::{
    automation::{
        StoryInteractionCaptureRequest, StoryInteractionPostcondition, StoryInteractionRequest,
        StoryInteractionStep,
    },
    presentation::{StoryPresentation, StoryViewportPreset},
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::controls::ControlValue;

/// One named operation in a story-owned scenario.
///
/// Names are intended for UI rows and diagnostics; the contained interaction
/// is executed by the same [`StoryInteractionRequest`] executor used by MCP.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[schemars(deny_unknown_fields)]
pub struct StoryScenarioStep {
    /// Stable, human-readable step name.
    pub name: String,
    /// Interaction operation dispatched for this step.
    pub step: StoryInteractionStep,
}

impl StoryScenarioStep {
    /// Creates one named scenario step.
    pub fn new(name: impl Into<String>, step: StoryInteractionStep) -> Self {
        Self {
            name: name.into(),
            step,
        }
    }
}

impl From<(String, StoryInteractionStep)> for StoryScenarioStep {
    fn from((name, step): (String, StoryInteractionStep)) -> Self {
        Self::new(name, step)
    }
}

/// A reusable story-owned interaction workflow.
///
/// Scenario keys are stable within a story and are used by automation clients;
/// titles and descriptions are presentation text. Controls and presentation
/// are applied before the first interaction step. Postconditions compare the
/// selected route's semantic values using exact JSON equality after the steps
/// complete. Each invocation creates a new [`StoryInteractionRequest`] and is
/// therefore a fresh run; callers must not resume or retry a partially
/// dispatched destructive batch.
#[derive(Clone, Debug, Default, Deserialize, JsonSchema, PartialEq, Serialize)]
#[schemars(deny_unknown_fields)]
pub struct StoryScenario {
    /// Stable key used to select this scenario.
    pub key: String,
    /// Human-readable scenario title.
    pub title: String,
    /// Optional human-readable scenario description.
    #[serde(default)]
    pub description: String,
    /// Control values applied before interaction begins.
    #[serde(default)]
    pub controls: BTreeMap<String, ControlValue>,
    /// Presentation applied before interaction begins.
    #[serde(default)]
    pub presentation: StoryPresentation,
    /// Ordered, named interaction steps.
    #[serde(default)]
    pub steps: Vec<StoryScenarioStep>,
    /// Exact semantic-value checks evaluated after all steps complete.
    #[serde(default)]
    pub postconditions: Vec<StoryInteractionPostcondition>,
    /// Optional PNG capture after successful postconditions.
    #[serde(default)]
    pub capture: Option<StoryInteractionCaptureRequest>,
}

impl StoryScenario {
    /// Creates an empty scenario. Add steps with [`Self::step`] or construct
    /// the public fields directly.
    pub fn new(key: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            title: title.into(),
            ..Self::default()
        }
    }

    /// Sets the scenario description.
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = description.into();
        self
    }

    /// Replaces the initial controls.
    pub fn controls(mut self, controls: impl IntoIterator<Item = (String, ControlValue)>) -> Self {
        self.controls = controls.into_iter().collect();
        self
    }

    /// Sets one initial control value.
    pub fn control(mut self, key: impl Into<String>, value: ControlValue) -> Self {
        self.controls.insert(key.into(), value);
        self
    }

    /// Sets the initial presentation.
    pub fn presentation(mut self, presentation: StoryPresentation) -> Self {
        self.presentation = presentation;
        self
    }

    /// Sets the initial viewport while retaining the configured background.
    pub fn viewport(mut self, viewport: StoryViewportPreset) -> Self {
        self.presentation.viewport = viewport;
        self
    }

    /// Appends one named interaction step.
    pub fn step(mut self, step: impl Into<StoryScenarioStep>) -> Self {
        self.steps.push(step.into());
        self
    }

    /// Replaces the ordered interaction steps.
    pub fn steps(mut self, steps: impl IntoIterator<Item = StoryScenarioStep>) -> Self {
        self.steps = steps.into_iter().collect();
        self
    }

    /// Appends one exact semantic-value postcondition.
    pub fn postcondition(mut self, postcondition: StoryInteractionPostcondition) -> Self {
        self.postconditions.push(postcondition);
        self
    }

    /// Replaces the semantic-value postconditions.
    pub fn postconditions(
        mut self,
        postconditions: impl IntoIterator<Item = StoryInteractionPostcondition>,
    ) -> Self {
        self.postconditions = postconditions.into_iter().collect();
        self
    }

    /// Sets an optional final capture request.
    pub fn capture(mut self, capture: Option<StoryInteractionCaptureRequest>) -> Self {
        self.capture = capture;
        self
    }

    /// Converts this scenario into the shared interaction request contract.
    ///
    /// A fixed scenario viewport is copied into the request's viewport field so
    /// the existing capture-size and frame preparation logic applies it. A
    /// responsive viewport remains a `None` target size while still carrying
    /// the full presentation for the host to apply.
    pub fn interaction_request(&self, story_key: impl Into<String>) -> StoryInteractionRequest {
        let viewport = self.presentation.viewport;
        StoryInteractionRequest {
            story_key: Some(story_key.into()),
            controls: self.controls.clone(),
            width: None,
            height: None,
            viewport: Some(viewport),
            presentation: Some(self.presentation),
            steps: self.steps.iter().map(|step| step.step.clone()).collect(),
            postconditions: self.postconditions.clone(),
            capture: self.capture.clone(),
        }
    }
}

/// Snapshot alias used by automation and MCP list/run results.
///
/// Scenario descriptors are immutable, serializable values, so the authoring
/// and runtime-list representations intentionally share one type.
pub type StoryScenarioSnapshot = StoryScenario;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::automation::{StoryInteractionPostcondition, StoryMouseButton};
    use serde_json::json;

    #[test]
    fn scenario_builder_preserves_named_ordered_steps_and_initial_state() {
        let scenario = StoryScenario::new("submit", "Submit form")
            .description("Enter a valid value and submit it.")
            .control("enabled", ControlValue::Boolean(true))
            .viewport(StoryViewportPreset::Mobile)
            .step(StoryScenarioStep::new(
                "focus submit",
                StoryInteractionStep::FocusNext,
            ))
            .step((
                "click submit".to_string(),
                StoryInteractionStep::ClickTarget {
                    target_key: "submit".to_string(),
                    button: StoryMouseButton::Left,
                    click_count: 1,
                    modifiers: Default::default(),
                },
            ))
            .postcondition(StoryInteractionPostcondition::new(
                "status",
                json!({ "state": "submitted" }),
            ));

        assert_eq!(scenario.key, "submit");
        assert_eq!(scenario.title, "Submit form");
        assert_eq!(scenario.description, "Enter a valid value and submit it.");
        assert_eq!(scenario.controls["enabled"], ControlValue::Boolean(true));
        assert_eq!(scenario.presentation.viewport, StoryViewportPreset::Mobile);
        assert_eq!(
            scenario
                .steps
                .iter()
                .map(|step| step.name.as_str())
                .collect::<Vec<_>>(),
            ["focus submit", "click submit"]
        );

        let request = scenario.interaction_request("crate-FormStory");
        assert_eq!(request.story_key.as_deref(), Some("crate-FormStory"));
        assert_eq!(request.controls, scenario.controls);
        assert_eq!(request.viewport, Some(StoryViewportPreset::Mobile));
        assert_eq!(request.presentation, Some(scenario.presentation));
        assert_eq!(request.steps.len(), 2);
        assert_eq!(request.postconditions, scenario.postconditions);
    }
}
