//! Typed story controls shared by the workbench and automation surfaces.

use gpui::{App, Entity, Hsla, SharedString};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::{rc::Rc, str::FromStr};
use thiserror::Error;

/// A serializable color used by story controls and automation.
#[derive(Clone, Copy, Debug, JsonSchema, PartialEq, Serialize, Deserialize)]
#[schemars(deny_unknown_fields)]
pub struct ControlColor {
    pub h: f32,
    pub s: f32,
    pub l: f32,
    pub a: f32,
}

impl From<Hsla> for ControlColor {
    fn from(value: Hsla) -> Self {
        Self {
            h: value.h,
            s: value.s,
            l: value.l,
            a: value.a,
        }
    }
}

impl From<ControlColor> for Hsla {
    fn from(value: ControlColor) -> Self {
        Self {
            h: value.h,
            s: value.s,
            l: value.l,
            a: value.a,
        }
    }
}

/// A value that can be edited by the Storybook workbench.
#[derive(Clone, Debug, JsonSchema, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
#[schemars(deny_unknown_fields)]
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
    fn kind_name(&self) -> &'static str {
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

    fn numeric_value(&self) -> Option<f64> {
        match self {
            Self::Integer(value) => Some(*value as f64),
            Self::Float(value) => Some(*value),
            _ => None,
        }
    }
}

/// The editor presented for a control.
#[derive(Clone, Debug, Eq, JsonSchema, PartialEq, Serialize, Deserialize)]
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
#[derive(Clone, Copy, Debug, Default, JsonSchema, PartialEq, Serialize, Deserialize)]
#[schemars(deny_unknown_fields)]
pub struct ControlBounds {
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub step: Option<f64>,
}

/// Metadata and default value for one story control.
#[derive(Clone, Debug, JsonSchema, PartialEq, Serialize, Deserialize)]
#[schemars(deny_unknown_fields)]
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
#[derive(Clone, Debug, JsonSchema, PartialEq, Serialize, Deserialize)]
#[schemars(deny_unknown_fields)]
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

/// Typed access generated for a story's controllable fields.
///
/// Implement this trait manually for macro-free integrations or derive it with
/// `gpui_storybook::StoryControls`.
pub trait StoryControls: 'static {
    fn control_specs(&self) -> Vec<ControlSpec> {
        Vec::new()
    }

    fn control_value(&self, key: &str) -> Result<ControlValue, ControlError> {
        Err(ControlError::UnknownControl {
            key: key.to_owned(),
        })
    }

    fn set_control_value(&mut self, key: &str, _value: ControlValue) -> Result<(), ControlError> {
        Err(ControlError::UnknownControl {
            key: key.to_owned(),
        })
    }
}

/// Object-safe runtime boundary used by the workbench and MCP automation.
pub trait ControlTarget: 'static {
    fn specs(&self) -> &[ControlSpec];
    fn value(&self, key: &str, cx: &App) -> Result<ControlValue, ControlError>;
    fn snapshots(&self, cx: &App) -> Result<Vec<ControlSnapshot>, ControlError>;
    fn set(&self, key: &str, value: ControlValue, cx: &mut App) -> Result<(), ControlError>;
    fn reset(&self, key: &str, cx: &mut App) -> Result<(), ControlError>;
    fn reset_all(&self, cx: &mut App) -> Result<(), ControlError>;
}

/// Main-thread adapter from a typed GPUI entity to [`ControlTarget`].
pub struct EntityControlTarget<S: StoryControls> {
    entity: Entity<S>,
    specs: Vec<ControlSpec>,
}

impl<S: StoryControls> EntityControlTarget<S> {
    /// Captures the entity's current control values as reset defaults.
    pub fn new(entity: Entity<S>, cx: &App) -> Self {
        let specs = entity.read(cx).control_specs();
        Self { entity, specs }
    }

    /// Creates a heterogeneous target when the story exposes at least one control.
    pub fn optional(entity: Entity<S>, cx: &App) -> Option<Rc<dyn ControlTarget>> {
        let target = Self::new(entity, cx);
        (!target.specs.is_empty()).then(|| Rc::new(target) as Rc<dyn ControlTarget>)
    }

    /// Returns the typed story entity owned by this adapter.
    pub fn entity(&self) -> &Entity<S> {
        &self.entity
    }

    fn spec(&self, key: &str) -> Result<&ControlSpec, ControlError> {
        self.specs
            .iter()
            .find(|spec| spec.key == key)
            .ok_or_else(|| ControlError::UnknownControl {
                key: key.to_owned(),
            })
    }
}

impl<S: StoryControls> ControlTarget for EntityControlTarget<S> {
    fn specs(&self) -> &[ControlSpec] {
        &self.specs
    }

    fn value(&self, key: &str, cx: &App) -> Result<ControlValue, ControlError> {
        self.spec(key)?;
        self.entity.read(cx).control_value(key)
    }

    fn snapshots(&self, cx: &App) -> Result<Vec<ControlSnapshot>, ControlError> {
        self.specs
            .iter()
            .map(|spec| {
                Ok(ControlSnapshot {
                    spec: spec.clone(),
                    value: self.entity.read(cx).control_value(&spec.key)?,
                })
            })
            .collect()
    }

    fn set(&self, key: &str, value: ControlValue, cx: &mut App) -> Result<(), ControlError> {
        let spec = self.spec(key)?;
        validate_control_value(spec, &value)?;
        self.entity.update(cx, |story, cx| {
            story.set_control_value(key, value)?;
            cx.notify();
            Ok(())
        })
    }

    fn reset(&self, key: &str, cx: &mut App) -> Result<(), ControlError> {
        let default = self.spec(key)?.default.clone();
        self.set(key, default, cx)
    }

    fn reset_all(&self, cx: &mut App) -> Result<(), ControlError> {
        self.entity.update(cx, |story, cx| {
            for spec in &self.specs {
                story.set_control_value(&spec.key, spec.default.clone())?;
            }
            cx.notify();
            Ok(())
        })
    }
}

fn validate_control_value(spec: &ControlSpec, value: &ControlValue) -> Result<(), ControlError> {
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

/// Conversion contract used by generated controls for supported field types.
#[doc(hidden)]
pub trait ControlValueField: Clone + 'static {
    fn control_kind() -> ControlKind;
    fn to_control_value(&self) -> ControlValue;
    fn from_control_value(key: &str, value: ControlValue) -> Result<Self, ControlError>;
}

fn invalid_field_value(key: &str, expected: &'static str, value: &ControlValue) -> ControlError {
    ControlError::InvalidValue {
        key: key.to_owned(),
        expected,
        actual: value.kind_name(),
    }
}

impl ControlValueField for bool {
    fn control_kind() -> ControlKind {
        ControlKind::Checkbox
    }

    fn to_control_value(&self) -> ControlValue {
        ControlValue::Boolean(*self)
    }

    fn from_control_value(key: &str, value: ControlValue) -> Result<Self, ControlError> {
        match value {
            ControlValue::Boolean(value) => Ok(value),
            value => Err(invalid_field_value(key, "boolean", &value)),
        }
    }
}

macro_rules! impl_integer_control_field {
    ($($type:ty),+ $(,)?) => {
        $(
            impl ControlValueField for $type {
                fn control_kind() -> ControlKind {
                    ControlKind::Number
                }

                fn to_control_value(&self) -> ControlValue {
                    ControlValue::Integer(i64::try_from(*self).unwrap_or(i64::MAX))
                }

                fn from_control_value(
                    key: &str,
                    value: ControlValue,
                ) -> Result<Self, ControlError> {
                    match value {
                        ControlValue::Integer(value) => <$type>::try_from(value).map_err(|_| {
                            ControlError::RangeViolation {
                                key: key.to_owned(),
                                value: value as f64,
                                min: Some(<$type>::MIN as f64),
                                max: Some(<$type>::MAX as f64),
                            }
                        }),
                        value => Err(invalid_field_value(key, "integer", &value)),
                    }
                }
            }
        )+
    };
}

impl_integer_control_field!(i8, i16, i32, i64, isize, u8, u16, u32, usize);

macro_rules! impl_float_control_field {
    ($($type:ty),+ $(,)?) => {
        $(
            impl ControlValueField for $type {
                fn control_kind() -> ControlKind {
                    ControlKind::Number
                }

                fn to_control_value(&self) -> ControlValue {
                    ControlValue::Float(*self as f64)
                }

                fn from_control_value(
                    key: &str,
                    value: ControlValue,
                ) -> Result<Self, ControlError> {
                    match value {
                        ControlValue::Float(value) if value.is_finite() => Ok(value as $type),
                        ControlValue::Integer(value) => Ok(value as $type),
                        value => Err(invalid_field_value(key, "finite number", &value)),
                    }
                }
            }
        )+
    };
}

impl_float_control_field!(f32, f64);

impl ControlValueField for String {
    fn control_kind() -> ControlKind {
        ControlKind::Text
    }

    fn to_control_value(&self) -> ControlValue {
        ControlValue::Text(self.clone())
    }

    fn from_control_value(key: &str, value: ControlValue) -> Result<Self, ControlError> {
        match value {
            ControlValue::Text(value) => Ok(value),
            value => Err(invalid_field_value(key, "text", &value)),
        }
    }
}

impl ControlValueField for SharedString {
    fn control_kind() -> ControlKind {
        ControlKind::Text
    }

    fn to_control_value(&self) -> ControlValue {
        ControlValue::Text(self.to_string())
    }

    fn from_control_value(key: &str, value: ControlValue) -> Result<Self, ControlError> {
        match value {
            ControlValue::Text(value) => Ok(value.into()),
            value => Err(invalid_field_value(key, "text", &value)),
        }
    }
}

impl ControlValueField for Hsla {
    fn control_kind() -> ControlKind {
        ControlKind::ColorPicker
    }

    fn to_control_value(&self) -> ControlValue {
        ControlValue::Color((*self).into())
    }

    fn from_control_value(key: &str, value: ControlValue) -> Result<Self, ControlError> {
        match value {
            ControlValue::Color(value) => Ok(value.into()),
            value => Err(invalid_field_value(key, "color", &value)),
        }
    }
}

/// Converts an enum-like field into a select control value.
#[doc(hidden)]
pub fn choice_control_value(value: &impl ToString) -> ControlValue {
    ControlValue::Choice(value.to_string())
}

/// Parses a select control value into an enum-like field.
#[doc(hidden)]
pub fn parse_choice_control_value<T>(
    key: &str,
    value: ControlValue,
    options: &[String],
) -> Result<T, ControlError>
where
    T: FromStr,
{
    let ControlValue::Choice(choice) = value else {
        return Err(invalid_field_value(key, "choice", &value));
    };

    if !options.iter().any(|option| option == &choice) {
        return Err(ControlError::InvalidChoice {
            key: key.to_owned(),
            value: choice,
            options: options.to_vec(),
        });
    }

    choice.parse().map_err(|_| ControlError::InvalidChoice {
        key: key.to_owned(),
        value: choice,
        options: options.to_vec(),
    })
}

#[cfg(test)]
mod tests {
    use super::{
        ControlBounds, ControlError, ControlKind, ControlSpec, ControlTarget, ControlValue,
        EntityControlTarget, StoryControls, validate_control_value,
    };
    use gpui::{App, AppContext as _};
    use std::rc::Rc;

    struct ControlledStory {
        enabled: bool,
        padding: f64,
    }

    impl StoryControls for ControlledStory {
        fn control_specs(&self) -> Vec<ControlSpec> {
            vec![
                ControlSpec {
                    key: "enabled".to_owned(),
                    label: "Enabled".to_owned(),
                    description: String::new(),
                    category: "Properties".to_owned(),
                    kind: ControlKind::Checkbox,
                    default: ControlValue::Boolean(self.enabled),
                    bounds: ControlBounds::default(),
                    options: Vec::new(),
                },
                range_spec_with_default(self.padding),
            ]
        }

        fn control_value(&self, key: &str) -> Result<ControlValue, ControlError> {
            match key {
                "enabled" => Ok(ControlValue::Boolean(self.enabled)),
                "padding" => Ok(ControlValue::Float(self.padding)),
                _ => Err(ControlError::UnknownControl {
                    key: key.to_owned(),
                }),
            }
        }

        fn set_control_value(
            &mut self,
            key: &str,
            value: ControlValue,
        ) -> Result<(), ControlError> {
            match (key, value) {
                ("enabled", ControlValue::Boolean(value)) => self.enabled = value,
                ("padding", ControlValue::Float(value)) => self.padding = value,
                (key, value) => {
                    return Err(ControlError::InvalidValue {
                        key: key.to_owned(),
                        expected: "matching value",
                        actual: value.kind_name(),
                    });
                },
            }
            Ok(())
        }
    }

    fn range_spec_with_default(default: f64) -> ControlSpec {
        let mut spec = range_spec();
        spec.default = ControlValue::Float(default);
        spec
    }

    fn range_spec() -> ControlSpec {
        ControlSpec {
            key: "padding".to_owned(),
            label: "Padding".to_owned(),
            description: String::new(),
            category: "Properties".to_owned(),
            kind: ControlKind::Range,
            default: ControlValue::Float(8.0),
            bounds: ControlBounds {
                min: Some(0.0),
                max: Some(32.0),
                step: Some(1.0),
            },
            options: Vec::new(),
        }
    }

    #[test]
    fn values_round_trip_through_json() {
        let value = ControlValue::Choice("primary".to_owned());
        let json = serde_json::to_string(&value).expect("control value serializes");
        let decoded: ControlValue =
            serde_json::from_str(&json).expect("control value deserializes");

        assert_eq!(decoded, value);
    }

    #[test]
    fn range_validation_accepts_bounds() {
        assert_eq!(
            validate_control_value(&range_spec(), &ControlValue::Float(16.0)),
            Ok(())
        );
    }

    #[test]
    fn range_validation_reports_typed_violation() {
        assert_eq!(
            validate_control_value(&range_spec(), &ControlValue::Float(64.0)),
            Err(ControlError::RangeViolation {
                key: "padding".to_owned(),
                value: 64.0,
                min: Some(0.0),
                max: Some(32.0),
            })
        );
    }

    #[test]
    fn select_validation_rejects_unknown_option() {
        let spec = ControlSpec {
            key: "kind".to_owned(),
            label: "Kind".to_owned(),
            description: String::new(),
            category: "Properties".to_owned(),
            kind: ControlKind::Select,
            default: ControlValue::Choice("primary".to_owned()),
            bounds: ControlBounds::default(),
            options: vec!["primary".to_owned(), "danger".to_owned()],
        };

        assert_eq!(
            validate_control_value(&spec, &ControlValue::Choice("quiet".to_owned())),
            Err(ControlError::InvalidChoice {
                key: "kind".to_owned(),
                value: "quiet".to_owned(),
                options: vec!["primary".to_owned(), "danger".to_owned()],
            })
        );
    }

    #[gpui::test]
    fn entity_targets_mutate_and_reset_only_their_exact_story(cx: &mut App) {
        let first = cx.new(|_| ControlledStory {
            enabled: false,
            padding: 8.0,
        });
        let second = cx.new(|_| ControlledStory {
            enabled: true,
            padding: 12.0,
        });
        let first_target: Rc<dyn ControlTarget> =
            Rc::new(EntityControlTarget::new(first.clone(), cx));
        let second_target: Rc<dyn ControlTarget> =
            Rc::new(EntityControlTarget::new(second.clone(), cx));

        first_target
            .set("enabled", ControlValue::Boolean(true), cx)
            .expect("valid checkbox edit applies");
        first_target
            .set("padding", ControlValue::Float(24.0), cx)
            .expect("valid range edit applies");
        assert!(first.read(cx).enabled);
        assert_eq!(first.read(cx).padding, 24.0);
        assert!(second.read(cx).enabled);
        assert_eq!(second.read(cx).padding, 12.0);

        assert!(matches!(
            first_target.set("padding", ControlValue::Float(64.0), cx),
            Err(ControlError::RangeViolation { .. })
        ));
        assert!(matches!(
            first_target.set("missing", ControlValue::Boolean(false), cx),
            Err(ControlError::UnknownControl { .. })
        ));

        first_target
            .reset("padding", cx)
            .expect("one reset applies");
        assert_eq!(first.read(cx).padding, 8.0);
        first_target.reset_all(cx).expect("all controls reset");
        assert!(!first.read(cx).enabled);
        assert_eq!(first.read(cx).padding, 8.0);
        assert_eq!(
            second_target.value("padding", cx),
            Ok(ControlValue::Float(12.0))
        );
    }
}
