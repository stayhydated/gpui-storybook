//! Typed, serializable control metadata and values.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// A serializable color used by story controls and automation.
#[derive(Clone, Copy, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ControlColor {
    pub h: f32,
    pub s: f32,
    pub l: f32,
    pub a: f32,
}

/// A value that can be edited by the Storybook workbench.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
#[serde(deny_unknown_fields)]
pub enum ControlValue {
    Boolean(bool),
    Integer(i64),
    Float(f64),
    Text(String),
    Color(ControlColor),
    Choice(String),
    Json(serde_json::Value),
}

impl ControlValue {
    pub fn kind_name(&self) -> &'static str {
        match self {
            Self::Boolean(_) => "boolean",
            Self::Integer(_) => "integer",
            Self::Float(_) => "float",
            Self::Text(_) => "text",
            Self::Color(_) => "color",
            Self::Choice(_) => "choice",
            Self::Json(_) => "json",
        }
    }

    pub fn numeric_value(&self) -> Option<f64> {
        match self {
            Self::Integer(value) => Some(*value as f64),
            Self::Float(value) => Some(*value),
            _ => None,
        }
    }
}

/// The editor presented for a control.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ControlKind {
    Checkbox,
    Number,
    Range,
    Text,
    ColorPicker,
    Select,
    Custom(String),
}

/// Numeric limits applied before a value reaches a story instance.
#[derive(Clone, Copy, Debug, Default, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ControlBounds {
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub step: Option<f64>,
}

/// Metadata and default value for one story control.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ControlSpec {
    pub key: String,
    pub label: String,
    pub description: String,
    pub category: String,
    pub kind: ControlKind,
    pub default: ControlValue,
    pub bounds: ControlBounds,
    pub options: Vec<String>,
}

/// A current control value paired with its metadata.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ControlSnapshot {
    pub spec: ControlSpec,
    pub value: ControlValue,
}

/// Structured failures produced while reading or editing story controls.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum ControlError {
    #[error("unknown story control `{key}`")]
    UnknownControl { key: String },
    #[error("story control `{key}` expected {expected}, received {actual}")]
    InvalidValue {
        key: String,
        expected: &'static str,
        actual: &'static str,
    },
    #[error(
        "story control `{key}` rejected choice `{value}`; expected one of {}",
        .options.join(", ")
    )]
    InvalidChoice {
        key: String,
        value: String,
        options: Vec<String>,
    },
    #[error("story control `{key}` value {value} is outside bounds {min:?}..={max:?}")]
    RangeViolation {
        key: String,
        value: f64,
        min: Option<f64>,
        max: Option<f64>,
    },
}

pub fn validate_control_value(
    spec: &ControlSpec,
    value: &ControlValue,
) -> Result<(), ControlError> {
    let kind_matches = match (&spec.kind, value) {
        (ControlKind::Checkbox, ControlValue::Boolean(_))
        | (
            ControlKind::Number | ControlKind::Range,
            ControlValue::Integer(_) | ControlValue::Float(_),
        )
        | (ControlKind::Text, ControlValue::Text(_))
        | (ControlKind::ColorPicker, ControlValue::Color(_))
        | (ControlKind::Select, ControlValue::Choice(_)) => true,
        (ControlKind::Custom(_), value) => value.kind_name() == spec.default.kind_name(),
        _ => false,
    };
    if !kind_matches {
        return Err(ControlError::InvalidValue {
            key: spec.key.clone(),
            expected: spec.default.kind_name(),
            actual: value.kind_name(),
        });
    }
    if value
        .numeric_value()
        .is_some_and(|number| !number.is_finite())
    {
        return Err(ControlError::InvalidValue {
            key: spec.key.clone(),
            expected: "finite number",
            actual: "nonfinite number",
        });
    }
    if !spec.options.is_empty() {
        let choice = match value {
            ControlValue::Choice(choice) => choice,
            _ => {
                return Err(ControlError::InvalidValue {
                    key: spec.key.clone(),
                    expected: "choice",
                    actual: value.kind_name(),
                });
            },
        };
        if !spec.options.contains(choice) {
            return Err(ControlError::InvalidChoice {
                key: spec.key.clone(),
                value: choice.clone(),
                options: spec.options.clone(),
            });
        }
    }

    if spec.bounds.min.is_some() || spec.bounds.max.is_some() {
        let Some(numeric_value) = value.numeric_value() else {
            return Err(ControlError::InvalidValue {
                key: spec.key.clone(),
                expected: "number",
                actual: value.kind_name(),
            });
        };
        if spec
            .bounds
            .min
            .is_some_and(|minimum| numeric_value < minimum)
            || spec
                .bounds
                .max
                .is_some_and(|maximum| numeric_value > maximum)
        {
            return Err(ControlError::RangeViolation {
                key: spec.key.clone(),
                value: numeric_value,
                min: spec.bounds.min,
                max: spec.bounds.max,
            });
        }
    }

    Ok(())
}
