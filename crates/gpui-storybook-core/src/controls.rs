//! Typed story controls shared by the workbench and automation surfaces.

use gpui_kit::{App, Entity, Hsla, SharedString};
use std::{rc::Rc, str::FromStr};

pub use gpui_storybook_automation::{
    ControlBounds, ControlColor, ControlError, ControlKind, ControlSnapshot, ControlSpec,
    ControlValue, validate_control_value,
};

/// Projects GPUI color components into the portable control representation.
pub fn control_color(value: Hsla) -> ControlColor {
    ControlColor {
        h: value.h,
        s: value.s,
        l: value.l,
        a: value.a,
    }
}

/// Builds a GPUI color from the portable control representation.
pub fn hsla_color(value: ControlColor) -> Hsla {
    Hsla {
        h: value.h,
        s: value.s,
        l: value.l,
        a: value.a,
    }
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
        ControlValue::Color(control_color(*self))
    }

    fn from_control_value(key: &str, value: ControlValue) -> Result<Self, ControlError> {
        match value {
            ControlValue::Color(value) => Ok(hsla_color(value)),
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
        EntityControlTarget, StoryControls,
    };
    use gpui_kit::{App, AppContext as _};
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

    #[gpui_kit::test]
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
