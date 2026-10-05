use super::*;
use gpui_kit::TestAppContext;
use gpui_storybook_automation_gpui::AttachedInteraction;
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};
use tokio::sync::oneshot;

struct Lease(Arc<AtomicBool>);
impl Drop for Lease {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

fn setup(
    cx: &mut TestAppContext,
) -> (
    gpui_kit::WindowHandle<DemoRoot>,
    Entity<DemoRoot>,
    GpuiHostAttachment<DemoRoot>,
) {
    let (window, root) = cx.update(|cx| {
        gpui_kit::init(cx);
        let mut root = None;
        let window = cx
            .open_window(Default::default(), |window, cx| {
                let view = cx.new(|cx| DemoRoot::new(window, cx));
                root = Some(view.clone());
                view
            })
            .unwrap();
        (window, root.unwrap())
    });
    let attachment = cx
        .update_window(window.into(), |_, window, cx| attach(&root, window, cx))
        .unwrap();
    (window, root, attachment)
}

fn pump(cx: &mut TestAppContext, window: gpui_kit::WindowHandle<DemoRoot>, frames: usize) {
    for _ in 0..frames {
        cx.update_window(window.into(), |_, window, cx| window.draw(cx).clear(cx))
            .unwrap();
        cx.update_window(window.into(), |_, window, cx| {
            window.simulate_next_frame(cx)
        })
        .unwrap();
    }
}

fn request(steps: Vec<StoryInteractionStep>) -> StoryInteractionRequest {
    StoryInteractionRequest {
        story_key: None,
        controls: Default::default(),
        width: None,
        height: None,
        viewport: None,
        presentation: None,
        steps,
        postconditions: Vec::new(),
        capture: None,
    }
}

fn submit(
    cx: &mut TestAppContext,
    window: gpui_kit::WindowHandle<DemoRoot>,
    attachment: &GpuiHostAttachment<DemoRoot>,
    request: StoryInteractionRequest,
    fresh_fixture: bool,
) -> (
    oneshot::Receiver<Result<StoryInteractionSnapshot, StorybookAutomationError>>,
    Arc<AtomicBool>,
) {
    let (response, receiver) = oneshot::channel();
    let busy = Arc::new(AtomicBool::new(true));
    let lease = Lease(busy.clone());
    cx.update_window(window.into(), |_, window, cx| {
        attachment.run_steps(
            AttachedInteraction::builder()
                .request_id(1)
                .request(request)
                .fresh_fixture(fresh_fixture)
                .response(response)
                .progress(Arc::new(AtomicUsize::new(0)))
                .lease(lease)
                .build(),
            window,
            cx,
        )
    })
    .unwrap();
    (receiver, busy)
}

#[gpui_kit::test]
async fn embedded_attachment_drives_the_production_root_and_fresh_scenarios(
    cx: &mut TestAppContext,
) {
    let (window, root, attachment) = setup(cx);
    assert_eq!(attachment.stories().len(), 2);
    let mut batch = request(vec![click("increment")]);
    batch.postconditions.push(
        StoryInteractionPostcondition::new("public-state", serde_json::json!(1))
            .json_pointer("/count"),
    );
    let (receiver, busy) = submit(cx, window, &attachment, batch, false);
    pump(cx, window, 6);
    let result = receiver.await.unwrap().unwrap();
    assert_eq!(result.steps_dispatched, 1);
    assert_eq!(result.postconditions[0].actual.value["count"], 1);
    assert!(!busy.load(Ordering::SeqCst));
    assert_eq!(cx.update(|cx| root.read(cx).count()), 1);

    let counter = attachment.get_story(COUNTER_ROUTE).unwrap();
    let scenario = find_scenario(&counter, "increment-twice").unwrap();
    for _ in 0..2 {
        let (receiver, _) = submit(
            cx,
            window,
            &attachment,
            scenario.interaction_request(COUNTER_ROUTE),
            true,
        );
        pump(cx, window, 8);
        assert_eq!(
            receiver.await.unwrap().unwrap().postconditions[0]
                .actual
                .value["count"],
            2
        );
        assert_eq!(cx.update(|cx| root.read(cx).count()), 2);
    }

    cx.update_window(window.into(), |_, window, cx| {
        attachment.open_story(NOTES_ROUTE, window, cx)
    })
    .unwrap()
    .unwrap();
    pump(cx, window, 2);
    assert_eq!(
        cx.update(|cx| root.read(cx).route().to_owned()),
        NOTES_ROUTE
    );
    let notes = attachment.get_story(NOTES_ROUTE).unwrap();
    let scenario = find_scenario(&notes, "write-note").unwrap();
    for _ in 0..2 {
        let (receiver, _) = submit(
            cx,
            window,
            &attachment,
            scenario.interaction_request(NOTES_ROUTE),
            true,
        );
        pump(cx, window, 8);
        assert_eq!(
            receiver.await.unwrap().unwrap().postconditions[0]
                .actual
                .value["note"],
            "hello"
        );
        assert_eq!(cx.update(|cx| root.read(cx).note(cx)), "hello");
    }
}

#[gpui_kit::test]
async fn invalid_control_and_action_batches_leave_the_attached_root_unchanged(
    cx: &mut TestAppContext,
) {
    let (window, root, attachment) = setup(cx);
    let mut batch = request(vec![click("increment")]);
    batch.controls.insert(
        "enabled".to_owned(),
        ControlValue::Text("wrong type".to_owned()),
    );
    let (receiver, busy) = submit(cx, window, &attachment, batch, false);
    assert!(matches!(
        receiver.await.unwrap(),
        Err(StorybookAutomationError::ControlOperationFailed { .. })
    ));
    assert!(!busy.load(Ordering::SeqCst));
    assert_eq!(cx.update(|cx| root.read(cx).count()), 0);

    let mut batch = request(vec![StoryInteractionStep::DispatchAction {
        name: "embedded_demo::Increment".to_owned(),
        args: Some(serde_json::json!({})),
    }]);
    batch.story_key = Some(NOTES_ROUTE.to_owned());
    let (receiver, _) = submit(cx, window, &attachment, batch, false);
    assert!(matches!(
        receiver.await.unwrap(),
        Err(StorybookAutomationError::InvalidInteractionStep { step_index: 0, .. })
    ));
    assert_eq!(
        cx.update(|cx| root.read(cx).route().to_owned()),
        COUNTER_ROUTE
    );
    assert_eq!(cx.update(|cx| root.read(cx).count()), 0);
}

#[gpui_kit::test]
fn canceled_responses_retain_the_lease_until_submitted_frames_and_click_settle(
    cx: &mut gpui_kit::TestAppContext,
) {
    let (window, root, attachment) = setup(cx);
    let (receiver, busy) = submit(
        cx,
        window,
        &attachment,
        request(vec![
            StoryInteractionStep::WaitFrames { count: 3 },
            click("increment"),
        ]),
        false,
    );
    drop(receiver);
    assert!(busy.load(Ordering::SeqCst));
    pump(cx, window, 2);
    assert!(busy.load(Ordering::SeqCst));
    assert_eq!(cx.update(|cx| root.read(cx).count()), 0);
    pump(cx, window, 6);
    assert!(!busy.load(Ordering::SeqCst));
    assert_eq!(cx.update(|cx| root.read(cx).count()), 1);
    pump(cx, window, 3);
    assert_eq!(cx.update(|cx| root.read(cx).count()), 1);
}

#[gpui_kit::test]
async fn surface_invalidation_stops_deferred_input_and_releases_its_lease(cx: &mut TestAppContext) {
    let (window, root, attachment) = setup(cx);
    let (receiver, busy) = submit(
        cx,
        window,
        &attachment,
        request(vec![
            StoryInteractionStep::WaitFrames { count: 3 },
            click("increment"),
        ]),
        false,
    );
    pump(cx, window, 2);
    cx.update_window(window.into(), |_, window, cx| {
        attachment.invalidate(window, cx)
    })
    .unwrap();
    pump(cx, window, 6);
    assert!(matches!(
        receiver.await.unwrap(),
        Err(StorybookAutomationError::StaleHost { .. })
            | Err(StorybookAutomationError::InteractionFailed {
                steps_dispatched: 0,
                ..
            })
    ));
    assert_eq!(cx.update(|cx| root.read(cx).count()), 0);
    assert!(!busy.load(Ordering::SeqCst));
}
