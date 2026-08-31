use super::*;

pub(crate) fn validate_interaction_request(
    request: &StoryInteractionRequest,
) -> Result<(), StorybookAutomationError> {
    if request.steps.is_empty() {
        return Err(StorybookAutomationError::InvalidInteractionRequest {
            message: "interaction steps must not be empty".to_owned(),
        });
    }
    if request.steps.len() > MAX_INTERACTION_STEPS {
        return Err(StorybookAutomationError::InvalidInteractionRequest {
            message: format!("interaction steps exceed the limit of {MAX_INTERACTION_STEPS}"),
        });
    }

    if request.postconditions.len() > MAX_INTERACTION_POSTCONDITIONS {
        return Err(StorybookAutomationError::InvalidInteractionRequest {
            message: format!(
                "interaction postconditions exceed the limit of {MAX_INTERACTION_POSTCONDITIONS}"
            ),
        });
    }

    let capture_size_request = StoryScreenshotRequest {
        width: request.width,
        height: request.height,
        viewport: request.viewport,
        ..StoryScreenshotRequest::default()
    };
    validate_capture_target_size(&capture_size_request).map_err(|error| {
        StorybookAutomationError::InvalidInteractionRequest {
            message: error.to_string(),
        }
    })?;

    let mut text_bytes = 0usize;
    let mut waited_frames = 0u16;
    for (step_index, step) in request.steps.iter().enumerate() {
        match step {
            StoryInteractionStep::Keystrokes { keys } => {
                if keys.is_empty() {
                    return invalid_step(step_index, "keystrokes must not be empty");
                }
                if keys.len() > MAX_INTERACTION_STEPS {
                    return invalid_step(
                        step_index,
                        format!("keystrokes exceed the per-step limit of {MAX_INTERACTION_STEPS}"),
                    );
                }
                text_bytes = keys
                    .iter()
                    .fold(text_bytes, |total, key| total.saturating_add(key.len()));
            },
            StoryInteractionStep::Text { value } => {
                text_bytes = text_bytes.saturating_add(value.len());
            },
            StoryInteractionStep::PointerClick { click_count: 0, .. } => {
                return invalid_step(step_index, "click_count must be greater than zero");
            },
            StoryInteractionStep::ClickTarget {
                target_key,
                click_count: 0,
                ..
            } => {
                if target_key.trim().is_empty() {
                    return invalid_step(step_index, "target key must not be empty");
                }
                return invalid_step(step_index, "click_count must be greater than zero");
            },
            StoryInteractionStep::ClickTarget { target_key, .. } => {
                if target_key.trim().is_empty() {
                    return invalid_step(step_index, "target key must not be empty");
                }
                text_bytes = text_bytes.saturating_add(target_key.len());
            },
            StoryInteractionStep::PointerMove { point }
            | StoryInteractionStep::PointerClick { point, .. } => {
                validate_point(*point, step_index)?;
            },
            StoryInteractionStep::Scroll {
                point,
                delta_x,
                delta_y,
            } => {
                validate_point(*point, step_index)?;
                if !delta_x.is_finite() || !delta_y.is_finite() {
                    return invalid_step(step_index, "scroll deltas must be finite");
                }
            },
            StoryInteractionStep::WaitFrames { count: 0 } => {
                return invalid_step(step_index, "wait frame count must be greater than zero");
            },
            StoryInteractionStep::WaitFrames { count } => {
                waited_frames = waited_frames.saturating_add(*count);
            },
            _ => {},
        }
    }

    for (postcondition_index, postcondition) in request.postconditions.iter().enumerate() {
        if postcondition.value_key.trim().is_empty() {
            return invalid_postcondition(postcondition_index, "value key must not be empty");
        }
        if postcondition
            .json_pointer
            .as_deref()
            .is_some_and(|pointer| !pointer.is_empty() && !pointer.starts_with('/'))
        {
            return invalid_postcondition(
                postcondition_index,
                "JSON Pointers must be empty or start with `/`",
            );
        }
        if postcondition
            .max_frames
            .is_some_and(|frames| frames == 0 || frames > MAX_INTERACTION_WAITED_FRAMES)
        {
            return invalid_postcondition(
                postcondition_index,
                format!("max_frames must be between 1 and {MAX_INTERACTION_WAITED_FRAMES}"),
            );
        }
    }

    if text_bytes > MAX_INTERACTION_TEXT_BYTES {
        return Err(StorybookAutomationError::InvalidInteractionRequest {
            message: format!(
                "interaction text and keystroke syntax exceed the limit of {MAX_INTERACTION_TEXT_BYTES} UTF-8 bytes"
            ),
        });
    }
    if waited_frames > MAX_INTERACTION_WAITED_FRAMES {
        return Err(StorybookAutomationError::InvalidInteractionRequest {
            message: format!(
                "interaction frame waits exceed the limit of {MAX_INTERACTION_WAITED_FRAMES} frames"
            ),
        });
    }

    Ok(())
}

fn validate_point(point: StoryPoint, step_index: usize) -> Result<(), StorybookAutomationError> {
    if !point.x.is_finite() || !point.y.is_finite() {
        return invalid_step(step_index, "point coordinates must be finite");
    }

    let in_range = match point.space {
        StoryPointSpace::Normalized => {
            (0.0..=1.0).contains(&point.x) && (0.0..=1.0).contains(&point.y)
        },
        StoryPointSpace::LogicalPixels => point.x >= 0.0 && point.y >= 0.0,
    };
    if !in_range {
        return invalid_step(
            step_index,
            "point coordinates are outside their coordinate space",
        );
    }

    Ok(())
}

fn invalid_step<T>(
    step_index: usize,
    message: impl Into<String>,
) -> Result<T, StorybookAutomationError> {
    Err(StorybookAutomationError::InvalidInteractionStep {
        step_index,
        message: message.into(),
    })
}

fn invalid_postcondition<T>(
    postcondition_index: usize,
    message: impl Into<String>,
) -> Result<T, StorybookAutomationError> {
    Err(StorybookAutomationError::InvalidInteractionPostcondition {
        postcondition_index,
        message: message.into(),
    })
}

pub(crate) fn list_registered_actions(cx: &App) -> Vec<StoryActionSnapshot> {
    let mut generator = schemars::generate::SchemaSettings::draft2020_12()
        .with(|settings| settings.inline_subschemas = true)
        .into_generator();
    let documentation = cx.action_documentation();

    cx.action_schemas(&mut generator)
        .into_iter()
        .filter(|(name, _)| automation_action_is_visible(name))
        .map(|(name, schema)| StoryActionSnapshot {
            name: name.to_owned(),
            documentation: documentation.get(name).map(|value| (*value).to_owned()),
            argument_schema: schema.map(Value::from),
        })
        .collect()
}

pub(super) fn automation_action_is_visible(name: &str) -> bool {
    !matches!(name, "zed::NoAction" | "zed::Unbind") && !name.starts_with("storybook_workbench::")
}

pub(crate) enum PreparedInteractionStep {
    FocusNext,
    FocusPrevious,
    Blur,
    Keystrokes(Vec<Keystroke>),
    Text(Keystroke),
    DispatchAction(Box<dyn Action>),
    PointerMove(StoryPoint),
    PointerClick {
        point: StoryPoint,
        button: StoryMouseButton,
        click_count: u8,
        modifiers: StoryModifiers,
    },
    ClickTarget {
        key: String,
        button: StoryMouseButton,
        click_count: u8,
        modifiers: StoryModifiers,
    },
    Scroll {
        point: StoryPoint,
        delta_x: f32,
        delta_y: f32,
    },
    WaitFrames(u16),
}

pub(crate) fn prepare_interaction_steps(
    steps: &[StoryInteractionStep],
    cx: &App,
) -> Result<Vec<PreparedInteractionStep>, StorybookAutomationError> {
    steps
        .iter()
        .enumerate()
        .map(|(step_index, step)| match step {
            StoryInteractionStep::FocusNext => Ok(PreparedInteractionStep::FocusNext),
            StoryInteractionStep::FocusPrevious => Ok(PreparedInteractionStep::FocusPrevious),
            StoryInteractionStep::Blur => Ok(PreparedInteractionStep::Blur),
            StoryInteractionStep::Keystrokes { keys } => keys
                .iter()
                .map(|key| {
                    Keystroke::parse(key).map_err(|error| {
                        StorybookAutomationError::InvalidInteractionStep {
                            step_index,
                            message: format!("invalid keystroke `{key}`: {error}"),
                        }
                    })
                })
                .collect::<Result<Vec<_>, _>>()
                .map(PreparedInteractionStep::Keystrokes),
            StoryInteractionStep::Text { value } => Ok(PreparedInteractionStep::Text(Keystroke {
                modifiers: Modifiers::none(),
                key: value.clone(),
                key_char: Some(value.clone()),
            })),
            StoryInteractionStep::DispatchAction { name, args } => cx
                .build_action(name, args.clone())
                .map(PreparedInteractionStep::DispatchAction)
                .map_err(|error| StorybookAutomationError::InvalidInteractionStep {
                    step_index,
                    message: format!("action `{name}` could not be built: {error}"),
                }),
            StoryInteractionStep::PointerMove { point } => {
                Ok(PreparedInteractionStep::PointerMove(*point))
            },
            StoryInteractionStep::PointerClick {
                point,
                button,
                click_count,
                modifiers,
            } => Ok(PreparedInteractionStep::PointerClick {
                point: *point,
                button: *button,
                click_count: *click_count,
                modifiers: modifiers.clone(),
            }),
            StoryInteractionStep::ClickTarget {
                target_key,
                button,
                click_count,
                modifiers,
            } => Ok(PreparedInteractionStep::ClickTarget {
                key: target_key.clone(),
                button: *button,
                click_count: *click_count,
                modifiers: modifiers.clone(),
            }),
            StoryInteractionStep::Scroll {
                point,
                delta_x,
                delta_y,
            } => Ok(PreparedInteractionStep::Scroll {
                point: *point,
                delta_x: *delta_x,
                delta_y: *delta_y,
            }),
            StoryInteractionStep::WaitFrames { count } => {
                Ok(PreparedInteractionStep::WaitFrames(*count))
            },
        })
        .collect()
}
