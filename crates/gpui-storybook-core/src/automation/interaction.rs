use super::{
    AutomationOperationGuard, StoryCaptureSnapshot, StoryInteractionTargetsSnapshot,
    StoryScreenshotRequest, StorySnapshot, StorybookAutomationError, ensure_capture_target_visible,
    render_story_capture, rendered_interaction_targets, validate_capture_target_size,
};
use crate::{
    capture_region::{capture_region_bounds, scroll_capture_region_into_view},
    controls::ControlValue,
    presentation::StoryViewportPreset,
};
use gpui::{
    Action, App, Keystroke, Modifiers, MouseDownEvent, MouseMoveEvent, MouseUpEvent, PlatformInput,
    ScrollDelta, ScrollWheelEvent, TouchPhase, Window, point, px,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::{BTreeMap, VecDeque},
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};
use tokio::sync::oneshot;

/// Maximum number of steps accepted by one interaction batch.
pub const MAX_INTERACTION_STEPS: usize = 64;
/// Maximum UTF-8 byte length accepted across text values and keystroke strings.
pub const MAX_INTERACTION_TEXT_BYTES: usize = 4 * 1024;
/// Maximum number of rendered frames one batch may explicitly wait for.
pub const MAX_INTERACTION_WAITED_FRAMES: u16 = 120;

/// Runtime-visible GPUI action metadata.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StoryActionSnapshot {
    /// Runtime action name accepted by a `dispatch_action` step.
    pub name: String,
    /// Documentation registered with GPUI, when supplied by the action.
    pub documentation: Option<String>,
    /// JSON argument schema, or `None` for an action without a public schema.
    pub argument_schema: Option<Value>,
}

/// Coordinate space for a point inside the active story route.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StoryPointSpace {
    /// Fractions across the active route bounds in the inclusive range
    /// `0.0..=1.0`.
    #[default]
    Normalized,
    /// GPUI logical pixels measured from the active route origin.
    LogicalPixels,
}

/// A point relative to the active story or substory capture region.
#[derive(Clone, Copy, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StoryPoint {
    /// Coordinate space. Omission defaults to [`StoryPointSpace::Normalized`].
    #[serde(default)]
    pub space: StoryPointSpace,
    /// Horizontal coordinate in `space`.
    pub x: f32,
    /// Vertical coordinate in `space`.
    pub y: f32,
}

/// Mouse buttons supported by in-process story interaction.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StoryMouseButton {
    /// Primary mouse button.
    #[default]
    Left,
    /// Secondary mouse button.
    Right,
    /// Middle mouse button.
    Middle,
}

/// Keyboard modifiers supported by pointer interaction.
#[derive(
    Clone, Copy, Debug, Deserialize, Eq, JsonSchema, Ord, PartialEq, PartialOrd, Serialize,
)]
#[serde(rename_all = "snake_case")]
pub enum StoryModifier {
    /// Control modifier.
    Control,
    /// Alt or Option modifier.
    Alt,
    /// Shift modifier.
    Shift,
    /// Platform command modifier: Command on macOS or Control elsewhere.
    Platform,
    /// Function modifier.
    Function,
}

/// Modifier keys held while dispatching a pointer step.
#[derive(Clone, Debug, Default, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(transparent)]
pub struct StoryModifiers(pub Vec<StoryModifier>);

/// One ordered interaction operation.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum StoryInteractionStep {
    /// Move focus to the next focusable element.
    FocusNext,
    /// Move focus to the previous focusable element.
    FocusPrevious,
    /// Clear window focus.
    Blur,
    /// Parse and dispatch GPUI key-binding strings in order.
    Keystrokes {
        /// GPUI keystroke strings such as `enter` or `shift-tab`.
        keys: Vec<String>,
    },
    /// Insert one UTF-8 value into the focused basic text input.
    Text {
        /// Text value. This is not IME, clipboard, or dead-key simulation.
        value: String,
    },
    /// Build and dispatch one runtime-registered GPUI action.
    DispatchAction {
        /// Runtime name returned by action discovery.
        name: String,
        /// JSON arguments validated by the action builder.
        args: Option<Value>,
    },
    /// Move the pointer inside the active route bounds.
    PointerMove {
        /// Route-relative destination.
        point: StoryPoint,
    },
    /// Dispatch pointer move, button down, and button up at one point.
    PointerClick {
        /// Route-relative click destination.
        point: StoryPoint,
        /// Button, defaulting to left.
        #[serde(default)]
        button: StoryMouseButton,
        /// Positive click count, defaulting to one.
        #[serde(default = "default_click_count")]
        click_count: u8,
        /// Modifiers held for move, down, and up.
        #[serde(default)]
        modifiers: StoryModifiers,
    },
    /// Click the center of one stable semantic target in the active route.
    ClickTarget {
        /// Target key returned by semantic target discovery.
        key: String,
        /// Button, defaulting to left.
        #[serde(default)]
        button: StoryMouseButton,
        /// Positive click count, defaulting to one.
        #[serde(default = "default_click_count")]
        click_count: u8,
        /// Modifiers held for move, down, and up.
        #[serde(default)]
        modifiers: StoryModifiers,
    },
    /// Dispatch one pixel scroll event inside the active route bounds.
    Scroll {
        /// Route-relative event destination.
        point: StoryPoint,
        /// Horizontal logical-pixel delta.
        delta_x: f32,
        /// Vertical logical-pixel delta.
        delta_y: f32,
    },
    /// Refresh and continue after a positive number of rendered frames.
    WaitFrames {
        /// Rendered frames to wait.
        count: u16,
    },
}

const fn default_click_count() -> u8 {
    1
}

/// Optional PNG capture performed after the final interaction step.
#[derive(Clone, Debug, Default, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StoryInteractionCaptureRequest {
    /// PNG destination. The normal capture default is used when omitted.
    pub output_path: Option<PathBuf>,
}

/// One exclusive interaction batch.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StoryInteractionRequest {
    /// Stable story or substory route opened before controls and input.
    pub route: Option<String>,
    /// Typed control values applied before input.
    #[serde(default)]
    pub controls: BTreeMap<String, ControlValue>,
    /// Requested story-region width in physical pixels, supplied with `height`.
    pub width: Option<u32>,
    /// Requested story-region height in physical pixels, supplied with `width`.
    pub height: Option<u32>,
    /// Named viewport used when explicit dimensions are omitted.
    pub viewport: Option<StoryViewportPreset>,
    /// Ordered non-empty interaction steps.
    pub steps: Vec<StoryInteractionStep>,
    /// Optional first-frame capture after the final step or explicit waits.
    pub capture: Option<StoryInteractionCaptureRequest>,
}

/// Dispatch information reported directly by GPUI.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum StoryInteractionDispatch {
    /// The API reports dispatch but no handler outcome.
    Dispatched,
    /// A keystroke or text input dispatch result.
    Input {
        /// Whether GPUI handled the input.
        handled: bool,
    },
    /// A pointer or scroll platform-event result.
    PlatformEvent {
        /// Whether the event propagated through GPUI.
        propagated: bool,
        /// Whether a handler prevented default behavior.
        default_prevented: bool,
    },
}

/// Dispatch observations for one completed interaction step.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StoryInteractionObservation {
    /// Zero-based request step index.
    pub step_index: usize,
    /// One entry per GPUI dispatch made by the step.
    pub dispatches: Vec<StoryInteractionDispatch>,
}

/// Result of an interaction batch executed by the live GPUI host.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StoryInteractionSnapshot {
    /// Monotonic request ID assigned by this automation controller.
    pub request_id: u64,
    /// Story and route used by the executor.
    pub story: StorySnapshot,
    /// Count completed through a safe executor boundary.
    pub steps_dispatched: usize,
    /// Dispatch information for completed steps.
    pub observations: Vec<StoryInteractionObservation>,
    /// Whether any focus handle exists in the window after the batch.
    pub focused: bool,
    /// Optional capture produced within this batch.
    pub capture: Option<StoryCaptureSnapshot>,
}

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
                key,
                click_count: 0,
                ..
            } => {
                if key.trim().is_empty() {
                    return invalid_step(step_index, "target key must not be empty");
                }
                return invalid_step(step_index, "click_count must be greater than zero");
            },
            StoryInteractionStep::ClickTarget { key, .. } => {
                if key.trim().is_empty() {
                    return invalid_step(step_index, "target key must not be empty");
                }
                text_bytes = text_bytes.saturating_add(key.len());
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

fn automation_action_is_visible(name: &str) -> bool {
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
                key,
                button,
                click_count,
                modifiers,
            } => Ok(PreparedInteractionStep::ClickTarget {
                key: key.clone(),
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

pub(crate) struct PreparedStoryInteraction {
    pub request_id: u64,
    pub story: StorySnapshot,
    pub steps: Vec<PreparedInteractionStep>,
    pub capture: Option<StoryInteractionCaptureRequest>,
    pub response: oneshot::Sender<Result<StoryInteractionSnapshot, StorybookAutomationError>>,
    pub progress: Arc<AtomicUsize>,
    pub operation: AutomationOperationGuard,
}

pub(crate) fn interaction_target_size(
    request: &StoryInteractionRequest,
) -> Result<Option<(u32, u32)>, StorybookAutomationError> {
    let size_request = StoryScreenshotRequest {
        width: request.width,
        height: request.height,
        viewport: request.viewport,
        ..StoryScreenshotRequest::default()
    };
    validate_capture_target_size(&size_request)
}

pub(crate) fn schedule_story_interaction(
    interaction: PreparedStoryInteraction,
    window: &mut Window,
) {
    window.on_next_frame(move |window, _cx| {
        if interaction.response.is_closed() {
            return;
        }
        let resized =
            match ensure_capture_target_visible(&interaction.story.capture_route_id, window) {
                Ok(resized) => resized,
                Err(error) => {
                    let _ = interaction.response.send(Err(error));
                    return;
                },
            };
        if resized {
            window.refresh();
            window.on_next_frame(move |window, _cx| prepare_interaction_route(interaction, window));
        } else {
            prepare_interaction_route(interaction, window);
        }
    });
}

pub(crate) fn schedule_interaction_target_listing(
    story: StorySnapshot,
    response: oneshot::Sender<Result<StoryInteractionTargetsSnapshot, StorybookAutomationError>>,
    window: &mut Window,
) {
    window.refresh();
    window.on_next_frame(move |window, _cx| {
        let resized = match ensure_capture_target_visible(&story.capture_route_id, window) {
            Ok(resized) => resized,
            Err(error) => {
                let _ = response.send(Err(error));
                return;
            },
        };
        if resized {
            window.refresh();
            window.on_next_frame(move |window, _cx| {
                prepare_interaction_target_listing(story, response, window);
            });
        } else {
            prepare_interaction_target_listing(story, response, window);
        }
    });
}

fn prepare_interaction_target_listing(
    story: StorySnapshot,
    response: oneshot::Sender<Result<StoryInteractionTargetsSnapshot, StorybookAutomationError>>,
    window: &mut Window,
) {
    if !scroll_capture_region_into_view(&story.capture_route_id) {
        let _ = response.send(Err(
            StorybookAutomationError::InteractionTargetsUnavailable {
                route: story.capture_route_id,
            },
        ));
        return;
    }
    window.refresh();
    window.on_next_frame(move |_window, _cx| {
        let _ = response.send(rendered_interaction_targets(story));
    });
}

fn prepare_interaction_route(interaction: PreparedStoryInteraction, window: &mut Window) {
    if interaction.response.is_closed() {
        return;
    }
    if !scroll_capture_region_into_view(&interaction.story.capture_route_id) {
        let _ = interaction
            .response
            .send(Err(StorybookAutomationError::CaptureUnavailable {
                message: format!(
                    "capture route `{}` was not rendered by the current story view",
                    interaction.story.capture_route_id
                ),
            }));
        return;
    }

    window.refresh();
    window.on_next_frame(move |window, cx| start_interaction_runner(interaction, window, cx));
}

struct InteractionRunner {
    request_id: u64,
    story: StorySnapshot,
    steps: VecDeque<(usize, PreparedInteractionStep)>,
    capture: Option<StoryInteractionCaptureRequest>,
    response: oneshot::Sender<Result<StoryInteractionSnapshot, StorybookAutomationError>>,
    progress: Arc<AtomicUsize>,
    observations: Vec<StoryInteractionObservation>,
    _operation: AutomationOperationGuard,
}

fn start_interaction_runner(
    interaction: PreparedStoryInteraction,
    window: &mut Window,
    cx: &mut App,
) {
    if interaction.response.is_closed() {
        return;
    }

    let Some(region) = capture_region_bounds(&interaction.story.capture_route_id) else {
        let _ = interaction
            .response
            .send(Err(StorybookAutomationError::CaptureUnavailable {
                message: format!(
                    "capture route `{}` was not rendered by the current story view",
                    interaction.story.capture_route_id
                ),
            }));
        return;
    };

    let targets = if interaction
        .steps
        .iter()
        .any(|step| matches!(step, PreparedInteractionStep::ClickTarget { .. }))
    {
        match rendered_interaction_targets(interaction.story.clone()) {
            Ok(snapshot) => Some(snapshot.targets),
            Err(error) => {
                let _ = interaction.response.send(Err(error));
                return;
            },
        }
    } else {
        None
    };
    let mut steps = VecDeque::with_capacity(interaction.steps.len());
    for (step_index, step) in interaction.steps.into_iter().enumerate() {
        match resolve_step_point(
            step,
            step_index,
            &interaction.story.capture_route_id,
            &region.bounds,
            targets.as_deref().unwrap_or_default(),
        ) {
            Ok(step) => steps.push_back((step_index, step)),
            Err(error) => {
                let _ = interaction.response.send(Err(error));
                return;
            },
        }
    }

    run_interaction(
        InteractionRunner {
            request_id: interaction.request_id,
            story: interaction.story,
            steps,
            capture: interaction.capture,
            response: interaction.response,
            progress: interaction.progress,
            observations: Vec::new(),
            _operation: interaction.operation,
        },
        window,
        cx,
    );
}

fn resolve_step_point(
    step: PreparedInteractionStep,
    step_index: usize,
    route: &str,
    bounds: &gpui::Bounds<gpui::Pixels>,
    targets: &[crate::capture_region::StoryInteractionTargetSnapshot],
) -> Result<PreparedInteractionStep, StorybookAutomationError> {
    let resolve = |point: StoryPoint| {
        resolve_story_point(point, bounds).map_err(|message| {
            StorybookAutomationError::InvalidInteractionStep {
                step_index,
                message,
            }
        })
    };
    match step {
        PreparedInteractionStep::PointerMove(point) => {
            resolve(point).map(PreparedInteractionStep::PointerMove)
        },
        PreparedInteractionStep::PointerClick {
            point,
            button,
            click_count,
            modifiers,
        } => resolve(point).map(|point| PreparedInteractionStep::PointerClick {
            point,
            button,
            click_count,
            modifiers,
        }),
        PreparedInteractionStep::ClickTarget {
            key,
            button,
            click_count,
            modifiers,
        } => {
            let target = targets
                .iter()
                .find(|target| target.key == key)
                .ok_or_else(|| StorybookAutomationError::InteractionTargetNotFound {
                    route: route.to_owned(),
                    key: key.clone(),
                })?;
            let target_bounds = target.bounds;
            if !target_bounds.x.is_finite()
                || !target_bounds.y.is_finite()
                || !target_bounds.width.is_finite()
                || !target_bounds.height.is_finite()
                || target_bounds.width <= 0.0
                || target_bounds.height <= 0.0
            {
                return Err(StorybookAutomationError::InvalidInteractionStep {
                    step_index,
                    message: format!("interaction target `{key}` has no usable area"),
                });
            }
            resolve(StoryPoint {
                space: StoryPointSpace::LogicalPixels,
                x: target_bounds.x + target_bounds.width / 2.0,
                y: target_bounds.y + target_bounds.height / 2.0,
            })
            .map(|point| PreparedInteractionStep::PointerClick {
                point,
                button,
                click_count,
                modifiers,
            })
        },
        PreparedInteractionStep::Scroll {
            point,
            delta_x,
            delta_y,
        } => resolve(point).map(|point| PreparedInteractionStep::Scroll {
            point,
            delta_x,
            delta_y,
        }),
        step => Ok(step),
    }
}

fn resolve_story_point(
    point: StoryPoint,
    bounds: &gpui::Bounds<gpui::Pixels>,
) -> Result<StoryPoint, String> {
    let origin_x = f32::from(bounds.origin.x);
    let origin_y = f32::from(bounds.origin.y);
    let width = f32::from(bounds.size.width);
    let height = f32::from(bounds.size.height);
    if !origin_x.is_finite()
        || !origin_y.is_finite()
        || !width.is_finite()
        || !height.is_finite()
        || width <= 0.0
        || height <= 0.0
    {
        return Err("the rendered capture region has no usable area".to_owned());
    }

    let (x, y) = match point.space {
        StoryPointSpace::Normalized => (point.x * width, point.y * height),
        StoryPointSpace::LogicalPixels => {
            if point.x < 0.0 || point.y < 0.0 || point.x > width || point.y > height {
                return Err(format!(
                    "logical point ({}, {}) is outside the rendered route size ({width}, {height})",
                    point.x, point.y
                ));
            }
            (point.x, point.y)
        },
    };

    // Capture bounds are half-open hit-testing regions. Preserve the public
    // inclusive endpoint contract while keeping `1.0`/`extent` on the story
    // side of the boundary instead of targeting adjacent Storybook chrome.
    let right = origin_x + width;
    let bottom = origin_y + height;
    let max_x = right.next_down();
    let max_y = bottom.next_down();
    if max_x < origin_x || max_y < origin_y {
        return Err("the rendered capture region has no usable area".to_owned());
    }

    Ok(StoryPoint {
        space: StoryPointSpace::LogicalPixels,
        x: (origin_x + x).clamp(origin_x, max_x),
        y: (origin_y + y).clamp(origin_y, max_y),
    })
}

fn run_interaction(mut runner: InteractionRunner, window: &mut Window, cx: &mut App) {
    if runner.response.is_closed() {
        return;
    }

    while let Some((step_index, step)) = runner.steps.pop_front() {
        if let PreparedInteractionStep::WaitFrames(count) = step {
            schedule_wait_frames(runner, step_index, count, window);
            return;
        }

        let defer_continuation = matches!(&step, PreparedInteractionStep::DispatchAction(_));
        let dispatches = dispatch_step(step, window, cx);
        runner.observations.push(StoryInteractionObservation {
            step_index,
            dispatches,
        });
        runner.progress.fetch_add(1, Ordering::SeqCst);

        if runner.response.is_closed() {
            return;
        }
        if defer_continuation {
            // GPUI queues action dispatch at the end of the current effect cycle.
            // Queue continuation after it so the next request step cannot overtake
            // the action handler.
            window.defer(cx, move |window, cx| run_interaction(runner, window, cx));
            return;
        }
    }

    finish_interaction(runner, window, cx);
}

fn schedule_wait_frames(
    runner: InteractionRunner,
    step_index: usize,
    remaining: u16,
    window: &mut Window,
) {
    window.refresh();
    window.on_next_frame(move |window, cx| {
        if runner.response.is_closed() {
            return;
        }
        if remaining > 1 {
            schedule_wait_frames(runner, step_index, remaining - 1, window);
        } else {
            let mut runner = runner;
            runner.observations.push(StoryInteractionObservation {
                step_index,
                dispatches: vec![StoryInteractionDispatch::Dispatched],
            });
            runner.progress.fetch_add(1, Ordering::SeqCst);
            run_interaction(runner, window, cx);
        }
    });
}

fn dispatch_step(
    step: PreparedInteractionStep,
    window: &mut Window,
    cx: &mut App,
) -> Vec<StoryInteractionDispatch> {
    match step {
        PreparedInteractionStep::FocusNext => {
            window.focus_next(cx);
            vec![StoryInteractionDispatch::Dispatched]
        },
        PreparedInteractionStep::FocusPrevious => {
            window.focus_prev(cx);
            vec![StoryInteractionDispatch::Dispatched]
        },
        PreparedInteractionStep::Blur => {
            window.blur();
            vec![StoryInteractionDispatch::Dispatched]
        },
        PreparedInteractionStep::Keystrokes(keys) => keys
            .into_iter()
            .map(|key| StoryInteractionDispatch::Input {
                handled: window.dispatch_keystroke(key, cx),
            })
            .collect(),
        PreparedInteractionStep::Text(text) => vec![StoryInteractionDispatch::Input {
            handled: window.dispatch_keystroke(text, cx),
        }],
        PreparedInteractionStep::DispatchAction(action) => {
            window.dispatch_action(action, cx);
            vec![StoryInteractionDispatch::Dispatched]
        },
        PreparedInteractionStep::PointerMove(point) => vec![dispatch_platform_event(
            PlatformInput::MouseMove(MouseMoveEvent {
                position: window_point(point),
                ..MouseMoveEvent::default()
            }),
            window,
            cx,
        )],
        PreparedInteractionStep::PointerClick {
            point,
            button,
            click_count,
            modifiers,
        } => {
            let position = window_point(point);
            let button = mouse_button(button);
            let modifiers = gpui_modifiers(&modifiers);
            vec![
                dispatch_platform_event(
                    PlatformInput::MouseMove(MouseMoveEvent {
                        position,
                        modifiers,
                        ..MouseMoveEvent::default()
                    }),
                    window,
                    cx,
                ),
                dispatch_platform_event(
                    PlatformInput::MouseDown(MouseDownEvent {
                        button,
                        position,
                        modifiers,
                        click_count: usize::from(click_count),
                        first_mouse: false,
                    }),
                    window,
                    cx,
                ),
                dispatch_platform_event(
                    PlatformInput::MouseUp(MouseUpEvent {
                        button,
                        position,
                        modifiers,
                        click_count: usize::from(click_count),
                    }),
                    window,
                    cx,
                ),
            ]
        },
        PreparedInteractionStep::Scroll {
            point: story_point,
            delta_x,
            delta_y,
        } => vec![dispatch_platform_event(
            PlatformInput::ScrollWheel(ScrollWheelEvent {
                position: window_point(story_point),
                delta: ScrollDelta::Pixels(point(px(delta_x), px(delta_y))),
                modifiers: Modifiers::none(),
                touch_phase: TouchPhase::Moved,
            }),
            window,
            cx,
        )],
        PreparedInteractionStep::ClickTarget { .. } => {
            unreachable!("semantic target clicks are resolved before dispatch")
        },
        PreparedInteractionStep::WaitFrames(_) => unreachable!("wait steps are scheduled"),
    }
}

fn dispatch_platform_event(
    event: PlatformInput,
    window: &mut Window,
    cx: &mut App,
) -> StoryInteractionDispatch {
    let result = window.dispatch_event(event, cx);
    StoryInteractionDispatch::PlatformEvent {
        propagated: result.propagate,
        default_prevented: result.default_prevented,
    }
}

fn window_point(story_point: StoryPoint) -> gpui::Point<gpui::Pixels> {
    debug_assert_eq!(story_point.space, StoryPointSpace::LogicalPixels);
    point(px(story_point.x), px(story_point.y))
}

fn mouse_button(button: StoryMouseButton) -> gpui::MouseButton {
    match button {
        StoryMouseButton::Left => gpui::MouseButton::Left,
        StoryMouseButton::Right => gpui::MouseButton::Right,
        StoryMouseButton::Middle => gpui::MouseButton::Middle,
    }
}

fn gpui_modifiers(modifiers: &StoryModifiers) -> Modifiers {
    let mut result = Modifiers::none();
    for modifier in &modifiers.0 {
        match modifier {
            StoryModifier::Control => result.control = true,
            StoryModifier::Alt => result.alt = true,
            StoryModifier::Shift => result.shift = true,
            StoryModifier::Platform => result.platform = true,
            StoryModifier::Function => result.function = true,
        }
    }
    result
}

fn finish_interaction(runner: InteractionRunner, window: &mut Window, cx: &mut App) {
    if runner.response.is_closed() {
        return;
    }

    if let Some(capture) = runner.capture.clone() {
        window.refresh();
        window.on_next_frame(move |window, cx| {
            if runner.response.is_closed() {
                return;
            }
            let capture_result = render_story_capture(
                runner.request_id,
                StoryScreenshotRequest {
                    output_path: capture.output_path,
                    ..StoryScreenshotRequest::default()
                },
                runner.story.clone(),
                window,
            );
            match capture_result {
                Ok(capture) => send_interaction_snapshot(runner, Some(capture), window, cx),
                Err(error) => {
                    let steps_dispatched = runner.progress.load(Ordering::SeqCst);
                    let _ =
                        runner
                            .response
                            .send(Err(StorybookAutomationError::InteractionFailed {
                                request_id: runner.request_id,
                                steps_dispatched,
                                message: error.to_string(),
                            }));
                },
            }
        });
    } else {
        send_interaction_snapshot(runner, None, window, cx);
    }
}

fn send_interaction_snapshot(
    runner: InteractionRunner,
    capture: Option<StoryCaptureSnapshot>,
    window: &Window,
    cx: &App,
) {
    let steps_dispatched = runner.progress.load(Ordering::SeqCst);
    let _ = runner.response.send(Ok(StoryInteractionSnapshot {
        request_id: runner.request_id,
        story: runner.story,
        steps_dispatched,
        observations: runner.observations,
        focused: window.focused(cx).is_some(),
        capture,
    }));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capture_region::{capture_story_view_with_scroll, interaction_target};
    use gpui::{
        AppContext as _, Context, Focusable, InteractiveElement as _, IntoElement, KeyDownEvent,
        Render, StatefulInteractiveElement as _, Styled as _, div,
    };
    use std::sync::atomic::AtomicBool;

    /// Sets the harness counter to a caller-provided value.
    #[derive(gpui::Action, Clone, Debug, Deserialize, Eq, schemars::JsonSchema, PartialEq)]
    #[action(namespace = storybook_interaction_test)]
    struct SetCounter {
        value: usize,
    }

    struct InteractionHarness {
        focus_handle: gpui::FocusHandle,
        text: String,
        clicks: usize,
        hovered: bool,
        action_value: usize,
        events: Vec<&'static str>,
    }

    impl Focusable for InteractionHarness {
        fn focus_handle(&self, _: &App) -> gpui::FocusHandle {
            self.focus_handle.clone()
        }
    }

    impl Render for InteractionHarness {
        fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            capture_story_view_with_scroll(
                "interaction-test",
                None,
                interaction_target(
                    "harness",
                    "Interaction harness",
                    div()
                        .id("interaction-harness")
                        .size_full()
                        .track_focus(&self.focus_handle)
                        .on_action(cx.listener(|this, action: &SetCounter, _, cx| {
                            this.action_value = action.value;
                            this.events.push("action");
                            cx.notify();
                        }))
                        .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                            if let Some(text) = event.keystroke.key_char.as_deref() {
                                this.text.push_str(text);
                                cx.stop_propagation();
                                cx.notify();
                            }
                        }))
                        .on_hover(cx.listener(|this, hovered, _, cx| {
                            this.hovered = *hovered;
                            cx.notify();
                        }))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.clicks += 1;
                            this.events.push("click");
                            cx.notify();
                        })),
                ),
            )
        }
    }

    fn request(steps: Vec<StoryInteractionStep>) -> StoryInteractionRequest {
        StoryInteractionRequest {
            route: None,
            controls: BTreeMap::new(),
            width: None,
            height: None,
            viewport: None,
            steps,
            capture: None,
        }
    }

    #[test]
    fn request_validation_enforces_batch_limits_before_dispatch() {
        assert!(matches!(
            validate_interaction_request(&request(Vec::new())),
            Err(StorybookAutomationError::InvalidInteractionRequest { .. })
        ));
        assert!(matches!(
            validate_interaction_request(&request(vec![StoryInteractionStep::WaitFrames {
                count: 0,
            }])),
            Err(StorybookAutomationError::InvalidInteractionStep { step_index: 0, .. })
        ));
        assert!(matches!(
            validate_interaction_request(&request(vec![StoryInteractionStep::Text {
                value: "x".repeat(MAX_INTERACTION_TEXT_BYTES + 1),
            }])),
            Err(StorybookAutomationError::InvalidInteractionRequest { .. })
        ));
        assert!(matches!(
            validate_interaction_request(&request(vec![StoryInteractionStep::Scroll {
                point: StoryPoint {
                    space: StoryPointSpace::Normalized,
                    x: 0.5,
                    y: 0.5,
                },
                delta_x: f32::NAN,
                delta_y: 1.0,
            }])),
            Err(StorybookAutomationError::InvalidInteractionStep { step_index: 0, .. })
        ));

        assert!(
            validate_interaction_request(&request(vec![
                StoryInteractionStep::FocusNext;
                MAX_INTERACTION_STEPS
            ]))
            .is_ok()
        );
        assert!(matches!(
            validate_interaction_request(&request(vec![
                StoryInteractionStep::FocusNext;
                MAX_INTERACTION_STEPS + 1
            ])),
            Err(StorybookAutomationError::InvalidInteractionRequest { .. })
        ));
        assert!(
            validate_interaction_request(&request(vec![StoryInteractionStep::Text {
                value: "x".repeat(MAX_INTERACTION_TEXT_BYTES),
            }]))
            .is_ok()
        );
        assert!(
            validate_interaction_request(&request(vec![StoryInteractionStep::Keystrokes {
                keys: vec!["a".to_owned(); MAX_INTERACTION_STEPS],
            }]))
            .is_ok()
        );
        assert!(matches!(
            validate_interaction_request(&request(vec![StoryInteractionStep::Keystrokes {
                keys: vec!["a".to_owned(); MAX_INTERACTION_STEPS + 1],
            }])),
            Err(StorybookAutomationError::InvalidInteractionStep { step_index: 0, .. })
        ));
        assert!(matches!(
            validate_interaction_request(&request(vec![StoryInteractionStep::Keystrokes {
                keys: vec!["x".repeat(MAX_INTERACTION_TEXT_BYTES + 1)],
            }])),
            Err(StorybookAutomationError::InvalidInteractionRequest { .. })
        ));
        assert!(
            validate_interaction_request(&request(vec![StoryInteractionStep::WaitFrames {
                count: MAX_INTERACTION_WAITED_FRAMES,
            },]))
            .is_ok()
        );
        assert!(matches!(
            validate_interaction_request(&request(vec![
                StoryInteractionStep::WaitFrames { count: 60 },
                StoryInteractionStep::WaitFrames { count: 61 },
            ])),
            Err(StorybookAutomationError::InvalidInteractionRequest { .. })
        ));
        assert!(matches!(
            validate_interaction_request(&request(vec![StoryInteractionStep::PointerClick {
                point: StoryPoint {
                    space: StoryPointSpace::Normalized,
                    x: 1.0,
                    y: 1.01,
                },
                button: StoryMouseButton::Left,
                click_count: 1,
                modifiers: StoryModifiers::default(),
            }])),
            Err(StorybookAutomationError::InvalidInteractionStep { step_index: 0, .. })
        ));
        assert!(matches!(
            validate_interaction_request(&request(vec![StoryInteractionStep::ClickTarget {
                key: " ".to_owned(),
                button: StoryMouseButton::Left,
                click_count: 1,
                modifiers: StoryModifiers::default(),
            }])),
            Err(StorybookAutomationError::InvalidInteractionStep { step_index: 0, .. })
        ));
    }

    #[test]
    fn points_resolve_from_fresh_logical_bounds() {
        let bounds = gpui::Bounds {
            origin: point(px(10.0), px(20.0)),
            size: gpui::size(px(200.0), px(100.0)),
        };
        assert_eq!(
            resolve_story_point(
                StoryPoint {
                    space: StoryPointSpace::Normalized,
                    x: 0.25,
                    y: 0.5,
                },
                &bounds,
            ),
            Ok(StoryPoint {
                space: StoryPointSpace::LogicalPixels,
                x: 60.0,
                y: 70.0,
            })
        );
        assert!(
            resolve_story_point(
                StoryPoint {
                    space: StoryPointSpace::LogicalPixels,
                    x: 201.0,
                    y: 0.0,
                },
                &bounds,
            )
            .is_err()
        );

        let edge = resolve_story_point(
            StoryPoint {
                space: StoryPointSpace::Normalized,
                x: 1.0,
                y: 1.0,
            },
            &bounds,
        )
        .expect("inclusive endpoints should remain inside the half-open route bounds");
        assert!((10.0..210.0).contains(&edge.x));
        assert!((20.0..120.0).contains(&edge.y));
    }

    #[test]
    fn interaction_wire_types_are_closed_and_tagged() {
        let step = serde_json::from_value::<StoryInteractionStep>(serde_json::json!({
            "type": "pointer_click",
            "point": { "space": "normalized", "x": 0.5, "y": 0.5 },
            "modifiers": ["shift"],
            "unknown": true
        }));
        assert!(step.is_err());

        let text = "héllo 世界".to_owned();
        let prepared = PreparedInteractionStep::Text(Keystroke {
            modifiers: Modifiers::none(),
            key: text.clone(),
            key_char: Some(text.clone()),
        });
        let PreparedInteractionStep::Text(keystroke) = prepared else {
            panic!("text should remain a text keystroke");
        };
        assert_eq!(keystroke.key_char.as_deref(), Some(text.as_str()));
    }

    #[gpui::test]
    fn action_discovery_and_batch_preparation_use_registered_schemas(cx: &mut App) {
        assert!(!automation_action_is_visible("zed::NoAction"));
        assert!(!automation_action_is_visible("zed::Unbind"));
        assert!(!automation_action_is_visible(
            "storybook_workbench::ResetAllControls"
        ));
        assert!(automation_action_is_visible("example::PublicAction"));

        let actions = list_registered_actions(cx);
        let action = actions
            .iter()
            .find(|action| action.name == "storybook_interaction_test::SetCounter")
            .expect("typed test action should be discoverable");
        assert_eq!(
            action.documentation.as_deref(),
            Some("Sets the harness counter to a caller-provided value.")
        );
        assert_eq!(
            action.argument_schema.as_ref().and_then(|schema| schema
                .pointer("/properties/value/type")
                .and_then(Value::as_str)),
            Some("integer")
        );
        assert!(actions.iter().all(|action| {
            action.name != "zed::NoAction"
                && action.name != "zed::Unbind"
                && !action.name.starts_with("storybook_workbench::")
        }));

        assert!(matches!(
            prepare_interaction_steps(
                &[
                    StoryInteractionStep::FocusNext,
                    StoryInteractionStep::DispatchAction {
                        name: "storybook_interaction_test::Missing".to_owned(),
                        args: None,
                    },
                ],
                cx,
            ),
            Err(StorybookAutomationError::InvalidInteractionStep { step_index: 1, .. })
        ));
    }

    #[gpui::test]
    async fn executor_dispatches_unicode_actions_pointer_and_frame_waits_in_process(
        cx: &mut gpui::TestAppContext,
    ) {
        let (window, harness, receiver, pending) = cx.update(|cx| {
            let mut harness = None;
            let window = cx
                .open_window(Default::default(), |_, cx| {
                    let entity = cx.new(|cx| InteractionHarness {
                        focus_handle: cx.focus_handle().tab_stop(true),
                        text: String::new(),
                        clicks: 0,
                        hovered: false,
                        action_value: 0,
                        events: Vec::new(),
                    });
                    harness = Some(entity.clone());
                    entity
                })
                .expect("interaction test window should open");
            let harness = harness.expect("harness should be created");
            let (response, receiver) = oneshot::channel();
            let pending = cx
                .update_window(window.into(), |_, window, cx| {
                    let steps = vec![
                        StoryInteractionStep::FocusNext,
                        StoryInteractionStep::Text {
                            value: "héllo 世界".to_owned(),
                        },
                        StoryInteractionStep::DispatchAction {
                            name: "storybook_interaction_test::SetCounter".to_owned(),
                            args: Some(serde_json::json!({ "value": 7 })),
                        },
                        StoryInteractionStep::PointerMove {
                            point: StoryPoint {
                                space: StoryPointSpace::Normalized,
                                x: 0.5,
                                y: 0.5,
                            },
                        },
                        StoryInteractionStep::PointerClick {
                            point: StoryPoint {
                                space: StoryPointSpace::Normalized,
                                x: 0.5,
                                y: 0.5,
                            },
                            button: StoryMouseButton::Left,
                            click_count: 1,
                            modifiers: StoryModifiers::default(),
                        },
                        StoryInteractionStep::ClickTarget {
                            key: "harness".to_owned(),
                            button: StoryMouseButton::Left,
                            click_count: 1,
                            modifiers: StoryModifiers::default(),
                        },
                        StoryInteractionStep::WaitFrames { count: 1 },
                    ];
                    let prepared = prepare_interaction_steps(&steps, cx)
                        .expect("steps should validate against GPUI registrations");
                    let pending = Arc::new(AtomicBool::new(true));
                    schedule_story_interaction(
                        PreparedStoryInteraction {
                            request_id: 9,
                            story: StorySnapshot {
                                key: "interaction-test".to_owned(),
                                crate_name: "test".to_owned(),
                                story_name: "InteractionHarness".to_owned(),
                                title: "Interaction".to_owned(),
                                description: String::new(),
                                group: None,
                                section: None,
                                source_file: file!().to_owned(),
                                source_line: line!(),
                                capture_route_id: "interaction-test".to_owned(),
                                default_size: super::super::StoryDefaultSize::default(),
                            },
                            steps: prepared,
                            capture: None,
                            response,
                            progress: Arc::new(AtomicUsize::new(0)),
                            operation: AutomationOperationGuard {
                                pending: pending.clone(),
                            },
                        },
                        window,
                    );
                    window.refresh();
                    pending
                })
                .expect("interaction runner should schedule");
            (window, harness, receiver, pending)
        });

        for _ in 0..6 {
            cx.update_window(window.into(), |_, window, cx| window.draw(cx).clear(cx))
                .expect("interaction harness should draw");
            cx.update_window(window.into(), |_, window, cx| {
                window.simulate_next_frame(cx)
            })
            .expect("next-frame callbacks should run");
        }

        let snapshot = receiver
            .await
            .expect("runner should respond")
            .expect("runner should complete");
        assert_eq!(snapshot.request_id, 9);
        assert_eq!(snapshot.steps_dispatched, 7);
        assert_eq!(snapshot.observations.len(), 7);
        assert!(!pending.load(Ordering::SeqCst));
        cx.update(|cx| {
            let harness = harness.read(cx);
            assert_eq!(harness.action_value, 7);
            assert_eq!(harness.clicks, 2);
            assert_eq!(harness.events, ["action", "click", "click"]);
            assert!(harness.hovered);
            assert_eq!(harness.text, "héllo 世界");
        });
    }

    #[gpui::test]
    async fn executor_rejects_an_unrendered_route_without_dispatch(cx: &mut gpui::TestAppContext) {
        let (window, receiver, progress, pending) = cx.update(|cx| {
            let window = cx
                .open_window(Default::default(), |_, cx| {
                    let entity = cx.new(|cx| InteractionHarness {
                        focus_handle: cx.focus_handle().tab_stop(true),
                        text: String::new(),
                        clicks: 0,
                        hovered: false,
                        action_value: 0,
                        events: Vec::new(),
                    });
                    entity
                })
                .expect("interaction test window should open");
            let (response, receiver) = oneshot::channel();
            let progress = Arc::new(AtomicUsize::new(0));
            let pending = Arc::new(AtomicBool::new(true));
            cx.update_window(window.into(), |_, window, cx| {
                schedule_story_interaction(
                    PreparedStoryInteraction {
                        request_id: 10,
                        story: StorySnapshot {
                            key: "missing-route".to_owned(),
                            crate_name: "test".to_owned(),
                            story_name: "MissingRoute".to_owned(),
                            title: "Missing route".to_owned(),
                            description: String::new(),
                            group: None,
                            section: None,
                            source_file: file!().to_owned(),
                            source_line: line!(),
                            capture_route_id: "missing-route".to_owned(),
                            default_size: super::super::StoryDefaultSize::default(),
                        },
                        steps: prepare_interaction_steps(&[StoryInteractionStep::FocusNext], cx)
                            .expect("focus step should prepare"),
                        capture: None,
                        response,
                        progress: progress.clone(),
                        operation: AutomationOperationGuard {
                            pending: pending.clone(),
                        },
                    },
                    window,
                );
                window.refresh();
            })
            .expect("missing-route runner should schedule");
            (window, receiver, progress, pending)
        });

        cx.update_window(window.into(), |_, window, cx| window.draw(cx).clear(cx))
            .expect("interaction harness should draw");
        cx.update_window(window.into(), |_, window, cx| {
            window.simulate_next_frame(cx)
        })
        .expect("next-frame callback should run");

        assert!(matches!(
            receiver.await.expect("runner should respond"),
            Err(StorybookAutomationError::CaptureUnavailable { message })
                if message.contains("missing-route")
        ));
        assert_eq!(progress.load(Ordering::SeqCst), 0);
        assert!(!pending.load(Ordering::SeqCst));
    }

    #[gpui::test]
    async fn capture_failure_reports_partial_dispatch_without_retry(cx: &mut gpui::TestAppContext) {
        let (window, harness, receiver, progress, pending) = cx.update(|cx| {
            let mut harness = None;
            let window = cx
                .open_window(Default::default(), |_, cx| {
                    let entity = cx.new(|cx| InteractionHarness {
                        focus_handle: cx.focus_handle().tab_stop(true),
                        text: String::new(),
                        clicks: 0,
                        hovered: false,
                        action_value: 0,
                        events: Vec::new(),
                    });
                    harness = Some(entity.clone());
                    entity
                })
                .expect("interaction test window should open");
            let harness = harness.expect("harness should be created");
            let (response, receiver) = oneshot::channel();
            let progress = Arc::new(AtomicUsize::new(0));
            let pending = Arc::new(AtomicBool::new(true));
            cx.update_window(window.into(), |_, window, cx| {
                schedule_story_interaction(
                    PreparedStoryInteraction {
                        request_id: 11,
                        story: StorySnapshot {
                            key: "interaction-test".to_owned(),
                            crate_name: "test".to_owned(),
                            story_name: "InteractionHarness".to_owned(),
                            title: "Interaction".to_owned(),
                            description: String::new(),
                            group: None,
                            section: None,
                            source_file: file!().to_owned(),
                            source_line: line!(),
                            capture_route_id: "interaction-test".to_owned(),
                            default_size: super::super::StoryDefaultSize::default(),
                        },
                        steps: prepare_interaction_steps(
                            &[StoryInteractionStep::PointerClick {
                                point: StoryPoint {
                                    space: StoryPointSpace::Normalized,
                                    x: 0.5,
                                    y: 0.5,
                                },
                                button: StoryMouseButton::Left,
                                click_count: 1,
                                modifiers: StoryModifiers::default(),
                            }],
                            cx,
                        )
                        .expect("pointer step should prepare"),
                        capture: Some(StoryInteractionCaptureRequest {
                            // `target` is an existing directory, so PNG save must fail
                            // after input dispatch without mutating repository files.
                            output_path: Some(PathBuf::from("target")),
                        }),
                        response,
                        progress: progress.clone(),
                        operation: AutomationOperationGuard {
                            pending: pending.clone(),
                        },
                    },
                    window,
                );
                window.refresh();
            })
            .expect("capture-failure runner should schedule");
            (window, harness, receiver, progress, pending)
        });

        for _ in 0..4 {
            cx.update_window(window.into(), |_, window, cx| window.draw(cx).clear(cx))
                .expect("interaction harness should draw");
            cx.update_window(window.into(), |_, window, cx| {
                window.simulate_next_frame(cx)
            })
            .expect("next-frame callbacks should run");
        }

        assert!(matches!(
            receiver.await.expect("runner should respond"),
            Err(StorybookAutomationError::InteractionFailed {
                request_id: 11,
                steps_dispatched: 1,
                ..
            })
        ));
        assert_eq!(progress.load(Ordering::SeqCst), 1);
        assert!(!pending.load(Ordering::SeqCst));
        cx.update(|cx| assert_eq!(harness.read(cx).clicks, 1));
    }
}
