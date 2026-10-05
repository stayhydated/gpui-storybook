use crate::*;
use serde_json::{Value, json};
use std::collections::BTreeMap;

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
            StoryInteractionStep::FocusNext {};
            MAX_INTERACTION_STEPS
        ]))
        .is_ok()
    );
    assert!(matches!(
        validate_interaction_request(&request(vec![
            StoryInteractionStep::FocusNext {};
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
    let mut request = request(vec![StoryInteractionStep::FocusNext {}]);
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
fn capabilities_reject_the_whole_batch_before_dispatch() {
    let capabilities = AutomationCapabilities::new([AutomationCapability::Focus]);
    let batch = request(vec![
        StoryInteractionStep::FocusNext {},
        StoryInteractionStep::Text {
            value: "later unsupported input".into(),
        },
    ]);
    assert_eq!(
        capabilities.validate_interaction(&batch),
        Err(StorybookAutomationError::UnsupportedCapability {
            capability: AutomationCapability::TextInsertion
        })
    );
    let mut sized = request(vec![StoryInteractionStep::FocusNext {}]);
    sized.viewport = Some(StoryViewportPreset::Mobile);
    assert_eq!(
        capabilities.validate_interaction(&sized),
        Err(StorybookAutomationError::UnsupportedCapability {
            capability: AutomationCapability::StorySizing
        })
    );
}

#[test]
fn observed_device_capture_rejects_desktop_sizing_and_control_mutation() {
    let capabilities = AutomationCapabilities::new([AutomationCapability::StoryCapture]);
    assert_eq!(
        capabilities.validate_capture(&StoryScreenshotRequest::default()),
        Ok(())
    );
    assert_eq!(
        capabilities.validate_capture(&StoryScreenshotRequest {
            width: Some(390),
            height: Some(844),
            ..Default::default()
        }),
        Err(StorybookAutomationError::UnsupportedCapability {
            capability: AutomationCapability::StorySizing
        })
    );
    assert_eq!(
        capabilities.validate_capture(&StoryScreenshotRequest {
            controls: [("enabled".into(), ControlValue::Boolean(true))].into(),
            ..Default::default()
        }),
        Err(StorybookAutomationError::UnsupportedCapability {
            capability: AutomationCapability::ControlMutation
        })
    );
}

#[test]
fn structured_errors_preserve_dispatched_progress() {
    let error = StorybookAutomationError::InteractionFailed {
        request_id: 9,
        steps_dispatched: 3,
        message: "surface replaced".into(),
    };
    let encoded = serde_json::to_value(&error).unwrap();
    assert_eq!(
        encoded,
        json!({ "code": "interaction_failed", "request_id": 9, "steps_dispatched": 3, "message": "surface replaced" })
    );
    assert_eq!(
        serde_json::from_value::<StorybookAutomationError>(encoded).unwrap(),
        error
    );
}

#[test]
fn interaction_decoding_rejects_unknown_fields() {
    assert!(
        serde_json::from_value::<StoryInteractionStep>(
            json!({ "type": "focus_next", "unexpected": true })
        )
        .is_err()
    );
    assert!(
        serde_json::from_value::<StoryInteractionStep>(
            json!({ "type": "text", "value": "hello", "unexpected": true })
        )
        .is_err()
    );
}

fn range_spec() -> ControlSpec {
    ControlSpec {
        key: "padding".into(),
        label: "Padding".into(),
        description: String::new(),
        category: "Properties".into(),
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
    let decoded: ControlValue = serde_json::from_str(&json).expect("control value deserializes");

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
fn control_preflight_rejects_wrong_shapes_and_nonfinite_numbers() {
    let spec = range_spec();
    for value in [
        ControlValue::Boolean(true),
        ControlValue::Text("8".into()),
        ControlValue::Float(f64::NAN),
        ControlValue::Float(f64::INFINITY),
    ] {
        assert!(validate_control_value(&spec, &value).is_err(), "{value:?}");
    }
    assert!(validate_control_value(&spec, &ControlValue::Integer(8)).is_ok());
}

#[test]
fn wire_frames_reject_oversized_truncated_and_unknown_requests() {
    use crate::wire::*;
    assert!(
        read_frame::<DeviceRequest>(&mut &((MAX_WIRE_BYTES as u32 + 1).to_be_bytes())[..]).is_err()
    );
    assert!(read_frame::<DeviceRequest>(&mut &[0, 0, 0, 2, b'{'][..]).is_err());
    assert!(serde_json::from_value::<DeviceRequest>(json!({"protocol_version": 1, "session": "host", "request_id": 1, "command": {"operation": "get_host", "extra": true}})).is_err());
    let request = DeviceRequest::builder()
        .protocol_version(PROTOCOL_VERSION)
        .session("host".to_owned())
        .request_id(2)
        .command(DeviceOperation::OpenStory {
            key: "counter".to_owned(),
        })
        .build();
    let mut encoded = Vec::new();
    write_frame(&mut encoded, &request).unwrap();
    assert_eq!(
        u32::from_be_bytes(encoded[..4].try_into().unwrap()) as usize,
        encoded.len() - 4
    );
    assert_eq!(
        read_frame::<DeviceRequest>(&mut encoded.as_slice()).unwrap(),
        request
    );
    assert!(matches!(
        request.validate_identity("replacement"),
        Err(StorybookAutomationError::StaleHost { .. })
    ));
    let mismatch = DeviceRequest::builder()
        .protocol_version(PROTOCOL_VERSION + 1)
        .session("host".to_owned())
        .request_id(3)
        .command(DeviceOperation::OpenStory {
            key: "counter".to_owned(),
        })
        .build();
    assert!(matches!(
        mismatch.validate_identity("host"),
        Err(StorybookAutomationError::ProtocolMismatch { .. })
    ));
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

#[test]
fn capture_size_validation_preserves_paired_dimension_precedence() {
    assert_eq!(
        validate_capture_target_size(&StoryScreenshotRequest {
            width: Some(800),
            height: Some(600),
            viewport: Some(StoryViewportPreset::Mobile),
            ..Default::default()
        }),
        Ok(Some((800, 600)))
    );
    for (width, height) in [
        (Some(1), None),
        (None, Some(1)),
        (Some(0), Some(1)),
        (Some(1), Some(0)),
    ] {
        assert!(matches!(
            validate_capture_target_size(&StoryScreenshotRequest {
                width,
                height,
                ..Default::default()
            }),
            Err(StorybookAutomationError::InvalidCaptureRequest { .. })
        ));
    }
}

#[test]
fn scenario_lookup_rejects_duplicate_ids() {
    let story = StorySnapshot {
        key: "fixture".into(),
        crate_name: "example".into(),
        story_name: "Fixture".into(),
        title: "Fixture".into(),
        description: String::new(),
        group: None,
        section: None,
        source_file: file!().into(),
        source_line: line!(),
        capture_route_id: "fixture".into(),
        default_size: Default::default(),
        scenarios: vec![
            StoryScenario::new("reset", "Reset"),
            StoryScenario::new("reset", "Duplicate"),
        ],
    };
    assert_eq!(
        find_scenario(&story, "reset"),
        Err(StorybookAutomationError::DuplicateScenarioKey {
            story_key: "fixture".into(),
            scenario_key: "reset".into()
        })
    );
    assert_eq!(
        find_scenario(&story, "missing"),
        Err(StorybookAutomationError::ScenarioNotFound {
            story_key: "fixture".into(),
            scenario_key: "missing".into()
        })
    );
}
