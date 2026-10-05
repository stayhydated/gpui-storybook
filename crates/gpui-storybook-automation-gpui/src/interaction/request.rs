use super::*;

pub fn list_registered_actions(cx: &App) -> Vec<StoryActionSnapshot> {
    let mut generator = schemars::generate::SchemaSettings::draft2020_12()
        .with(|settings| settings.inline_subschemas = true)
        .into_generator();
    let documentation = cx.action_documentation();

    cx.action_schemas(&mut generator)
        .into_iter()
        .filter(|(name, _)| !matches!(*name, "zed::NoAction" | "zed::Unbind"))
        .map(|(name, schema)| StoryActionSnapshot {
            name: name.to_owned(),
            documentation: documentation.get(name).map(|value| (*value).to_owned()),
            argument_schema: schema.map(Value::from),
        })
        .collect()
}

pub enum PreparedInteractionStep {
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

pub fn prepare_interaction_steps(
    steps: &[StoryInteractionStep],
    action_scope: &std::collections::BTreeSet<String>,
    cx: &App,
) -> Result<Vec<PreparedInteractionStep>, StorybookAutomationError> {
    steps
        .iter()
        .enumerate()
        .map(|(step_index, step)| match step {
            StoryInteractionStep::FocusNext {} => Ok(PreparedInteractionStep::FocusNext),
            StoryInteractionStep::FocusPrevious {} => Ok(PreparedInteractionStep::FocusPrevious),
            StoryInteractionStep::Blur {} => Ok(PreparedInteractionStep::Blur),
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
            StoryInteractionStep::DispatchAction { name, args } => {
                if !action_scope.contains(name) {
                    return Err(StorybookAutomationError::InvalidInteractionStep {
                        step_index,
                        message: format!(
                            "action `{name}` is outside the attached root's action scope"
                        ),
                    });
                }
                cx.build_action(name, args.clone())
                    .map(PreparedInteractionStep::DispatchAction)
                    .map_err(|error| StorybookAutomationError::InvalidInteractionStep {
                        step_index,
                        message: format!("action `{name}` could not be built: {error}"),
                    })
            },
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
