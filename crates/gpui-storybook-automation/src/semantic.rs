use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Logical bounds of a semantic interaction target relative to its story route.
#[derive(Clone, Copy, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StoryInteractionTargetBounds {
    /// Horizontal offset from the route origin in logical pixels.
    pub x: f32,
    /// Vertical offset from the route origin in logical pixels.
    pub y: f32,
    /// Target width in logical pixels.
    pub width: f32,
    /// Target height in logical pixels.
    pub height: f32,
}

/// One stable semantic interaction target rendered by a story route.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StoryInteractionTargetSnapshot {
    /// Stable target key supplied by the story author.
    pub key: String,
    /// Human-readable target label supplied by the story author.
    pub label: String,
    /// Target bounds in logical pixels relative to the active route.
    pub bounds: StoryInteractionTargetBounds,
}

/// One stable machine-readable value rendered by a story route.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StorySemanticValueSnapshot {
    /// Stable value key supplied by the story author.
    pub key: String,
    /// Human-readable value label supplied by the story author.
    pub label: String,
    /// Current JSON value captured from application state during rendering.
    pub value: Value,
}
