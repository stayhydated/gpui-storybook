use super::*;

fn geometry(width: u32, height: u32, display_width: u32, display_height: u32) -> SurfaceGeometry {
    SurfaceGeometry::builder()
        .x(0)
        .y(0)
        .width(width)
        .height(height)
        .scale(1.)
        .display_width(display_width)
        .display_height(display_height)
        .build()
}
fn observe(shell: &NativeShellHandle, geometry: SurfaceGeometry, appearance: &str) {
    shell
        .publish(NativeShellEvent::Frame {
            request_id: 0,
            observation: NativeObservation::builder()
                .route("account".to_owned())
                .appearance(appearance.to_owned())
                .geometry(geometry)
                .build(),
        })
        .unwrap();
}
fn ready(appearances: Vec<HostAppearance>) -> (NativeShellHandle, OperationGate) {
    let gate = OperationGate::default();
    let shell = NativeShellHandle::new(gate.clone(), appearances).unwrap();
    shell
        .publish(NativeShellEvent::Active { active: true })
        .unwrap();
    observe(&shell, geometry(400, 800, 400, 900), "light");
    (shell, gate)
}
fn selection(
    shell: &NativeShellHandle,
    gate: &OperationGate,
    id: u64,
) -> (NativeSelection, MutationLease) {
    let lease = gate.acquire().unwrap();
    (
        NativeSelection {
            route: "account".to_owned(),
            appearance: HostAppearance::dark(),
            request_id: id,
            surface: shell.snapshot().surface(),
            permit: lease.permit(),
            action: None,
        },
        lease,
    )
}

#[test]
fn committed_geometry_distinguishes_ime_from_display_replacement() {
    let (shell, gate) = ready(HostAppearance::standard());
    let initial = shell.snapshot();
    observe(&shell, geometry(400, 800, 400, 900), "light");
    assert_eq!(
        shell.snapshot().revision(),
        initial.revision(),
        "identical committed frames do not churn revisions"
    );
    let lease = gate.acquire().unwrap();
    observe(&shell, geometry(400, 500, 400, 900), "light");
    assert!(
        lease.is_current(),
        "IME viewport changes preserve the surface session"
    );
    assert_eq!(shell.snapshot().surface(), initial.surface());
    observe(&shell, geometry(800, 300, 900, 400), "light");
    assert!(
        !lease.is_current(),
        "display replacement revokes geometry-scoped work"
    );
    assert_eq!(shell.snapshot().surface(), initial.surface() + 1);
    assert!(
        shell.snapshot().active(),
        "surface replacement preserves foreground lifecycle"
    );
    assert_eq!(shell.snapshot().route(), "account");
}

#[test]
fn release_clears_geometry_without_inventing_a_background_transition() {
    let (shell, _) = ready(HostAppearance::standard());
    shell.publish(NativeShellEvent::SurfaceReleased {}).unwrap();
    assert!(shell.snapshot().active());
    assert!(shell.snapshot().geometry().is_none());
}

#[test]
fn native_selection_is_dispatched_once_and_retains_unknown_submission() {
    let (shell, gate) = ready(HostAppearance::standard());
    let (selection, lease) = selection(&shell, &gate, 7);
    let result = shell.submit(selection, |payload| {
        assert_eq!(payload.request_id(), 7);
        assert_eq!(payload.appearance(), "dark");
        assert!(shell.is_current(7), "permit is retained before enqueueing");
        Err(NativeSubmissionError::OutcomeUnknown(
            "lost JNI reply".to_owned(),
        ))
    });
    assert!(matches!(
        result,
        Err(NativeSubmissionError::OutcomeUnknown(_))
    ));
    assert!(lease.is_current());
    assert!(!shell.is_input_allowed());
    shell
        .publish(NativeShellEvent::Active { active: false })
        .unwrap();
    assert!(!shell.is_current(7));
    assert_eq!(shell.snapshot().ack(), 7);
    assert!(!shell.snapshot().applied);
    drop(lease);
    assert!(!shell.is_input_allowed());
    shell
        .publish(NativeShellEvent::Active { active: true })
        .unwrap();
    assert!(shell.is_input_allowed());
}

#[test]
fn a_late_frame_cannot_acknowledge_work_on_a_replacement_surface() {
    let (shell, gate) = ready(HostAppearance::standard());
    let (selection, _lease) = selection(&shell, &gate, 9);
    shell.submit(selection, |_| Ok(())).unwrap();
    shell
        .publish(NativeShellEvent::Frame {
            request_id: 9,
            observation: NativeObservation::builder()
                .route("account".to_owned())
                .appearance("dark".to_owned())
                .geometry(geometry(800, 300, 900, 400))
                .build(),
        })
        .unwrap();
    assert_eq!(shell.snapshot().ack(), 0);
    assert!(!shell.is_current(9));
    shell
        .publish(NativeShellEvent::Settled {
            request_id: 9,
            applied: true,
        })
        .unwrap();
    assert_eq!(shell.snapshot().ack(), 0);
}

#[test]
fn appearance_ids_preserve_distinct_dark_schemes() {
    let oled = HostAppearance::builder()
        .id("oled".to_owned())
        .label("OLED".to_owned())
        .dark(true)
        .build();
    let (shell, _) = ready(vec![
        HostAppearance::light(),
        HostAppearance::dark(),
        oled.clone(),
    ]);
    observe(&shell, geometry(400, 800, 400, 900), "oled");
    assert_eq!(shell.snapshot().appearance(), &oled);
    let selected = super::super::requested_appearance(
        Some(StoryPresentation {
            background: StoryCanvasBackground::Dark,
            viewport: StoryViewportPreset::Responsive,
        }),
        &shell.snapshot(),
    )
    .unwrap();
    assert_eq!(
        selected.id(),
        "oled",
        "a contrast request preserves the exact matching scheme"
    );
    assert!(NativeShellHandle::new(OperationGate::default(), vec![oled.clone(), oled]).is_err());
}

#[test]
fn malformed_native_events_leave_public_state_unchanged() {
    let (shell, _) = ready(HostAppearance::standard());
    let before = shell.snapshot();
    for json in [
        "{}".to_owned(),
        "x".repeat(16 * 1024 + 1),
        r#"{"event":"active","active":false,"extra":true}"#.to_owned(),
    ] {
        assert!(shell.publish_json(&json).is_err());
        assert_eq!(shell.snapshot().revision(), before.revision());
        assert!(shell.snapshot().active());
    }
}

#[test]
fn native_long_encoding_preserves_every_unsigned_request_bit() {
    let (shell, gate) = ready(HostAppearance::standard());
    let (selection, _lease) = selection(&shell, &gate, u64::MAX);
    shell
        .submit(selection, |dispatch| {
            let encoded = serde_json::to_value(&dispatch).unwrap();
            assert_eq!(encoded["request_id"], -1);
            let decoded: NativeDispatch = serde_json::from_value(encoded).unwrap();
            assert_eq!(decoded.request_id(), u64::MAX);
            assert!(shell.is_current(decoded.request_id()));
            Ok(())
        })
        .unwrap();
    shell
        .publish_json(r#"{"event":"settled","request_id":-1,"applied":true}"#)
        .unwrap();
    assert_eq!(shell.snapshot().ack(), u64::MAX);
    assert!(!shell.is_current(u64::MAX));
}
