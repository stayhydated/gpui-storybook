use super::*;

pub(crate) struct PreparedStoryInteraction {
    pub request_id: u64,
    pub story: StorySnapshot,
    pub steps: Vec<PreparedInteractionStep>,
    pub postconditions: Vec<StoryInteractionPostcondition>,
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

pub(super) struct InteractionRunner {
    pub(super) request_id: u64,
    pub(super) story: StorySnapshot,
    pub(super) steps: VecDeque<(usize, PreparedInteractionStep)>,
    pub(super) postconditions: Vec<StoryInteractionPostcondition>,
    pub(super) postcondition_index: usize,
    pub(super) postcondition_frames_waited: u16,
    pub(super) postcondition_results: Vec<StoryInteractionPostconditionSnapshot>,
    pub(super) capture: Option<StoryInteractionCaptureRequest>,
    pub(super) response:
        oneshot::Sender<Result<StoryInteractionSnapshot, StorybookAutomationError>>,
    pub(super) progress: Arc<AtomicUsize>,
    pub(super) observations: Vec<StoryInteractionObservation>,
    pub(super) _operation: AutomationOperationGuard,
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

    let mut steps = VecDeque::with_capacity(interaction.steps.len());
    for (step_index, step) in interaction.steps.into_iter().enumerate() {
        match resolve_step_point(step, step_index, &region.bounds) {
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
            postconditions: interaction.postconditions,
            postcondition_index: 0,
            postcondition_frames_waited: 0,
            postcondition_results: Vec::new(),
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
    bounds: &gpui::Bounds<gpui::Pixels>,
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

fn resolve_target_click(
    step: PreparedInteractionStep,
    step_index: usize,
    story: &StorySnapshot,
) -> Result<PreparedInteractionStep, StorybookAutomationError> {
    let PreparedInteractionStep::ClickTarget {
        key,
        button,
        click_count,
        modifiers,
    } = step
    else {
        return Ok(step);
    };

    let targets = rendered_interaction_targets(story.clone())?.targets;
    let target = targets
        .iter()
        .find(|target| target.key == key)
        .ok_or_else(|| StorybookAutomationError::InteractionTargetNotFound {
            route: story.capture_route_id.clone(),
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

    let region = capture_region_bounds(&story.capture_route_id).ok_or_else(|| {
        StorybookAutomationError::CaptureUnavailable {
            message: format!(
                "capture route `{}` was not rendered by the current story view",
                story.capture_route_id
            ),
        }
    })?;
    let point = resolve_story_point(
        StoryPoint {
            space: StoryPointSpace::LogicalPixels,
            x: target_bounds.x + target_bounds.width / 2.0,
            y: target_bounds.y + target_bounds.height / 2.0,
        },
        &region.bounds,
    )
    .map_err(|message| StorybookAutomationError::InvalidInteractionStep {
        step_index,
        message,
    })?;

    Ok(PreparedInteractionStep::PointerClick {
        point,
        button,
        click_count,
        modifiers,
    })
}

pub(super) fn resolve_story_point(
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

        let step = match resolve_target_click(step, step_index, &runner.story) {
            Ok(step) => step,
            Err(error) => {
                send_interaction_failure(runner, error);
                return;
            },
        };

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
            window.blur(cx);
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
            unreachable!("semantic target clicks are resolved at dispatch")
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
    if !runner.postconditions.is_empty() {
        schedule_postcondition_check(runner, window);
        return;
    }

    finish_interaction_capture(runner, window, cx);
}

pub(super) fn schedule_postcondition_check(mut runner: InteractionRunner, window: &mut Window) {
    if runner.response.is_closed() {
        return;
    }

    runner.postcondition_frames_waited = runner.postcondition_frames_waited.saturating_add(1);
    window.refresh();
    window.on_next_frame(move |window, cx| {
        if runner.response.is_closed() {
            return;
        }

        let (value_key, json_pointer, expected, frame_limit) = {
            let postcondition = &runner.postconditions[runner.postcondition_index];
            (
                postcondition.value_key.clone(),
                postcondition.json_pointer.clone(),
                postcondition.expected.clone(),
                postcondition.frame_limit(),
            )
        };
        let route = runner.story.capture_route_id.clone();
        let values = match rendered_semantic_values(runner.story.clone()) {
            Ok(snapshot) => snapshot.values,
            Err(error) => {
                send_interaction_failure(runner, error);
                return;
            },
        };
        if let Some(actual) = values.into_iter().find(|value| value.key == value_key) {
            let actual_value = json_pointer
                .as_deref()
                .map_or(Some(&actual.value), |pointer| actual.value.pointer(pointer));
            if actual_value == Some(&expected) {
                runner
                    .postcondition_results
                    .push(StoryInteractionPostconditionSnapshot {
                        value_key,
                        json_pointer,
                        expected,
                        actual,
                        frames_waited: runner.postcondition_frames_waited,
                    });
                runner.postcondition_index += 1;
                runner.postcondition_frames_waited = 0;

                if runner.postcondition_index == runner.postconditions.len() {
                    finish_interaction_capture(runner, window, cx);
                } else {
                    schedule_postcondition_check(runner, window);
                }
                return;
            }
        }

        if runner.postcondition_frames_waited >= frame_limit {
            send_interaction_failure(
                runner,
                StorybookAutomationError::SemanticValueWaitTimedOut {
                    route,
                    key: value_key,
                    max_frames: frame_limit,
                },
            );
            return;
        }

        schedule_postcondition_check(runner, window);
    });
}

fn finish_interaction_capture(runner: InteractionRunner, window: &mut Window, cx: &mut App) {
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
                Err(error) => send_interaction_failure(runner, error),
            }
        });
    } else {
        send_interaction_snapshot(runner, None, window, cx);
    }
}

fn send_interaction_failure(runner: InteractionRunner, error: StorybookAutomationError) {
    let steps_dispatched = runner.progress.load(Ordering::SeqCst);
    let _ = runner
        .response
        .send(Err(StorybookAutomationError::InteractionFailed {
            request_id: runner.request_id,
            steps_dispatched,
            message: error.to_string(),
        }));
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
        postconditions: runner.postcondition_results,
        capture,
    }));
}
