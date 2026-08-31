use super::*;
use super::{
    request::automation_action_is_visible,
    runner::{InteractionRunner, resolve_story_point, schedule_postcondition_check},
};
use crate::capture_region::{StorybookElementExt as _, capture_story_view_with_scroll};
use gpui::{
    AppContext as _, Context, Focusable, InteractiveElement as _, IntoElement, KeyDownEvent,
    ParentElement as _, Render, StatefulInteractiveElement as _, Styled as _, div,
};
use gpui_component::h_flex;
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
    semantic_value: Option<usize>,
}

impl Focusable for InteractionHarness {
    fn focus_handle(&self, _: &App) -> gpui::FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for InteractionHarness {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let harness = div()
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
            }))
            .storybook_target_as("harness", "Interaction harness");
        let harness = match self.semantic_value {
            Some(value) => harness
                .storybook_value_as("late-value", "Late value", value)
                .into_any_element(),
            None => harness.into_any_element(),
        };

        capture_story_view_with_scroll("interaction-test", None, harness)
    }
}

struct DynamicTargetHarness {
    focus_handle: gpui::FocusHandle,
    target_visible: bool,
    target_on_right: bool,
    target_clicks: usize,
    decoy_clicks: usize,
}

impl Focusable for DynamicTargetHarness {
    fn focus_handle(&self, _: &App) -> gpui::FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for DynamicTargetHarness {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let target = div()
            .id("dynamic-target")
            .w(px(80.))
            .h(px(80.))
            .on_click(cx.listener(|this, _, _, cx| {
                this.target_clicks += 1;
                cx.notify();
            }))
            .storybook_target_as("dynamic-target", "Dynamic target");
        let decoy = div()
            .id("dynamic-target-decoy")
            .w(px(80.))
            .h(px(80.))
            .on_click(cx.listener(|this, _, _, cx| {
                this.decoy_clicks += 1;
                cx.notify();
            }));
        let row = h_flex().size_full();
        let row = if !self.target_visible {
            row.child(decoy)
        } else if self.target_on_right {
            row.child(decoy).child(target)
        } else {
            row.child(target).child(decoy)
        };
        let content = row.track_focus(&self.focus_handle).on_action(cx.listener(
            |this, action: &SetCounter, _, cx| {
                this.target_visible = action.value > 0;
                this.target_on_right = action.value > 0;
                cx.notify();
            },
        ));

        capture_story_view_with_scroll("dynamic-target-test", None, content)
    }
}

fn interaction_story_snapshot() -> StorySnapshot {
    StorySnapshot {
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
        scenarios: Vec::new(),
    }
}

fn dynamic_target_story_snapshot() -> StorySnapshot {
    StorySnapshot {
        capture_route_id: "dynamic-target-test".to_owned(),
        key: "dynamic-target-test".to_owned(),
        story_name: "DynamicTargetHarness".to_owned(),
        title: "Dynamic target".to_owned(),
        ..interaction_story_snapshot()
    }
}

async fn run_dynamic_target_interaction(
    target_visible: bool,
    request_id: u64,
    cx: &mut gpui::TestAppContext,
) -> (StoryInteractionSnapshot, usize, usize, bool) {
    let (window, harness, receiver, pending) = cx.update(|cx| {
        let mut harness = None;
        let window = cx
            .open_window(Default::default(), |_, cx| {
                let entity = cx.new(|cx| DynamicTargetHarness {
                    focus_handle: cx.focus_handle().tab_stop(true),
                    target_visible,
                    target_on_right: false,
                    target_clicks: 0,
                    decoy_clicks: 0,
                });
                harness = Some(entity.clone());
                entity
            })
            .expect("dynamic target test window should open");
        let harness = harness.expect("dynamic target harness should be created");
        let (response, receiver) = oneshot::channel();
        let pending = cx
            .update_window(window.into(), |_, window, cx| {
                let steps = prepare_interaction_steps(
                    &[
                        StoryInteractionStep::FocusNext,
                        StoryInteractionStep::DispatchAction {
                            name: "storybook_interaction_test::SetCounter".to_owned(),
                            args: Some(serde_json::json!({ "value": 1 })),
                        },
                        StoryInteractionStep::WaitFrames { count: 1 },
                        StoryInteractionStep::ClickTarget {
                            target_key: "dynamic-target".to_owned(),
                            button: StoryMouseButton::Left,
                            click_count: 1,
                            modifiers: StoryModifiers::default(),
                        },
                    ],
                    cx,
                )
                .expect("dynamic target steps should prepare");
                let pending = Arc::new(AtomicBool::new(true));
                schedule_story_interaction(
                    PreparedStoryInteraction {
                        request_id,
                        story: dynamic_target_story_snapshot(),
                        steps,
                        postconditions: Vec::new(),
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
            .expect("dynamic target runner should schedule");
        (window, harness, receiver, pending)
    });

    for _ in 0..8 {
        cx.update_window(window.into(), |_, window, cx| window.draw(cx).clear(cx))
            .expect("dynamic target harness should draw");
        cx.update_window(window.into(), |_, window, cx| {
            window.simulate_next_frame(cx)
        })
        .expect("dynamic target next-frame callback should run");
    }

    let snapshot = receiver
        .await
        .expect("dynamic target runner should respond")
        .expect("dynamic target runner should complete");
    let (target_clicks, decoy_clicks) = cx.update(|cx| {
        let harness = harness.read(cx);
        (harness.target_clicks, harness.decoy_clicks)
    });
    (
        snapshot,
        target_clicks,
        decoy_clicks,
        pending.load(Ordering::SeqCst),
    )
}

fn request(steps: Vec<StoryInteractionStep>) -> StoryInteractionRequest {
    StoryInteractionRequest {
        story_key: None,
        controls: BTreeMap::new(),
        width: None,
        height: None,
        viewport: None,
        presentation: None,
        steps,
        postconditions: Vec::new(),
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
            target_key: " ".to_owned(),
            button: StoryMouseButton::Left,
            click_count: 1,
            modifiers: StoryModifiers::default(),
        }])),
        Err(StorybookAutomationError::InvalidInteractionStep { step_index: 0, .. })
    ));
}

#[test]
fn postcondition_validation_rejects_ambiguous_or_unbounded_assertions() {
    let mut request = request(vec![StoryInteractionStep::FocusNext]);
    request.postconditions = vec![StoryInteractionPostcondition::new("", Value::Null)];
    assert!(matches!(
        validate_interaction_request(&request),
        Err(StorybookAutomationError::InvalidInteractionPostcondition {
            postcondition_index: 0,
            ..
        })
    ));

    request.postconditions =
        vec![StoryInteractionPostcondition::new("status", Value::Null).json_pointer("status")];
    assert!(matches!(
        validate_interaction_request(&request),
        Err(StorybookAutomationError::InvalidInteractionPostcondition {
            postcondition_index: 0,
            ..
        })
    ));

    request.postconditions =
        vec![StoryInteractionPostcondition::new("status", Value::Null).max_frames(0)];
    assert!(matches!(
        validate_interaction_request(&request),
        Err(StorybookAutomationError::InvalidInteractionPostcondition {
            postcondition_index: 0,
            ..
        })
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
async fn postcondition_retries_a_missing_value_until_it_is_rendered(cx: &mut gpui::TestAppContext) {
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
                    semantic_value: None,
                });
                harness = Some(entity.clone());
                entity
            })
            .expect("postcondition test window should open");
        let harness = harness.expect("harness should be created");
        let (response, receiver) = oneshot::channel();
        let progress = Arc::new(AtomicUsize::new(1));
        let pending = Arc::new(AtomicBool::new(true));
        cx.update_window(window.into(), |_, window, _| {
            schedule_postcondition_check(
                InteractionRunner {
                    request_id: 12,
                    story: interaction_story_snapshot(),
                    steps: VecDeque::new(),
                    postconditions: vec![
                        StoryInteractionPostcondition::new("late-value", serde_json::json!(42))
                            .max_frames(3),
                    ],
                    postcondition_index: 0,
                    postcondition_frames_waited: 0,
                    postcondition_results: Vec::new(),
                    capture: None,
                    response,
                    progress: progress.clone(),
                    observations: Vec::new(),
                    _operation: AutomationOperationGuard {
                        pending: pending.clone(),
                    },
                },
                window,
            );
        })
        .expect("postcondition runner should schedule");
        (window, harness, receiver, progress, pending)
    });

    cx.update_window(window.into(), |_, window, cx| window.draw(cx).clear(cx))
        .expect("first postcondition frame should draw");
    cx.update_window(window.into(), |_, window, cx| {
        window.simulate_next_frame(cx)
    })
    .expect("first postcondition check should run");

    cx.update(|cx| {
        harness.update(cx, |harness, cx| {
            harness.semantic_value = Some(42);
            cx.notify();
        });
    });
    cx.update_window(window.into(), |_, window, cx| window.draw(cx).clear(cx))
        .expect("late-value frame should draw");
    cx.update_window(window.into(), |_, window, cx| {
        window.simulate_next_frame(cx)
    })
    .expect("second postcondition check should run");

    let snapshot = receiver
        .await
        .expect("runner should respond")
        .expect("late semantic value should satisfy the postcondition");
    assert_eq!(snapshot.steps_dispatched, 1);
    assert_eq!(snapshot.postconditions.len(), 1);
    assert_eq!(snapshot.postconditions[0].frames_waited, 2);
    assert_eq!(progress.load(Ordering::SeqCst), 1);
    assert!(!pending.load(Ordering::SeqCst));
}

#[gpui::test]
async fn postcondition_timeout_preserves_dispatched_progress(cx: &mut gpui::TestAppContext) {
    let (window, receiver, progress, pending) = cx.update(|cx| {
        let window = cx
            .open_window(Default::default(), |_, cx| {
                cx.new(|cx| InteractionHarness {
                    focus_handle: cx.focus_handle().tab_stop(true),
                    text: String::new(),
                    clicks: 0,
                    hovered: false,
                    action_value: 0,
                    events: Vec::new(),
                    semantic_value: None,
                })
            })
            .expect("postcondition test window should open");
        let (response, receiver) = oneshot::channel();
        let progress = Arc::new(AtomicUsize::new(1));
        let pending = Arc::new(AtomicBool::new(true));
        cx.update_window(window.into(), |_, window, _| {
            schedule_postcondition_check(
                InteractionRunner {
                    request_id: 13,
                    story: interaction_story_snapshot(),
                    steps: VecDeque::new(),
                    postconditions: vec![
                        StoryInteractionPostcondition::new("missing-value", serde_json::json!(42))
                            .max_frames(2),
                    ],
                    postcondition_index: 0,
                    postcondition_frames_waited: 0,
                    postcondition_results: Vec::new(),
                    capture: None,
                    response,
                    progress: progress.clone(),
                    observations: Vec::new(),
                    _operation: AutomationOperationGuard {
                        pending: pending.clone(),
                    },
                },
                window,
            );
        })
        .expect("postcondition runner should schedule");
        (window, receiver, progress, pending)
    });

    for _ in 0..2 {
        cx.update_window(window.into(), |_, window, cx| window.draw(cx).clear(cx))
            .expect("postcondition frame should draw");
        cx.update_window(window.into(), |_, window, cx| {
            window.simulate_next_frame(cx)
        })
        .expect("postcondition check should run");
    }

    assert!(matches!(
        receiver.await.expect("runner should respond"),
        Err(StorybookAutomationError::InteractionFailed {
            request_id: 13,
            steps_dispatched: 1,
            message,
        }) if message.contains("missing-value") && message.contains("2 frame(s)")
    ));
    assert_eq!(progress.load(Ordering::SeqCst), 1);
    assert!(!pending.load(Ordering::SeqCst));
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
                    semantic_value: None,
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
                        target_key: "harness".to_owned(),
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
                            scenarios: Vec::new(),
                        },
                        steps: prepared,
                        postconditions: Vec::new(),
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
async fn semantic_target_can_be_revealed_before_its_step_is_dispatched(
    cx: &mut gpui::TestAppContext,
) {
    let (snapshot, target_clicks, decoy_clicks, pending) =
        run_dynamic_target_interaction(false, 14, cx).await;

    assert_eq!(snapshot.steps_dispatched, 4);
    assert_eq!(target_clicks, 1);
    assert_eq!(decoy_clicks, 0);
    assert!(!pending);
}

#[gpui::test]
async fn semantic_target_uses_its_latest_bounds_at_dispatch(cx: &mut gpui::TestAppContext) {
    let (snapshot, target_clicks, decoy_clicks, pending) =
        run_dynamic_target_interaction(true, 15, cx).await;

    assert_eq!(snapshot.steps_dispatched, 4);
    assert_eq!(target_clicks, 1);
    assert_eq!(decoy_clicks, 0);
    assert!(!pending);
}

#[gpui::test]
async fn executor_rejects_an_unrendered_route_without_dispatch(cx: &mut gpui::TestAppContext) {
    let (window, receiver, progress, pending) = cx.update(|cx| {
        let window = cx
            .open_window(Default::default(), |_, cx| {
                cx.new(|cx| InteractionHarness {
                    focus_handle: cx.focus_handle().tab_stop(true),
                    text: String::new(),
                    clicks: 0,
                    hovered: false,
                    action_value: 0,
                    events: Vec::new(),
                    semantic_value: None,
                })
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
                        scenarios: Vec::new(),
                    },
                    steps: prepare_interaction_steps(&[StoryInteractionStep::FocusNext], cx)
                        .expect("focus step should prepare"),
                    postconditions: Vec::new(),
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
                    semantic_value: None,
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
                        scenarios: Vec::new(),
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
                    postconditions: Vec::new(),
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
