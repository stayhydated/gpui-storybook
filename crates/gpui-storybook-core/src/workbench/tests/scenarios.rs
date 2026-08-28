use super::*;

#[test]
fn scenario_progress_reports_only_completed_steps() {
    assert_eq!(
        StoryWorkbench::scenario_progress(&StorybookAutomationError::InteractionFailed {
            request_id: 7,
            steps_dispatched: 2,
            message: "postcondition failed".to_owned(),
        }),
        2
    );
    assert_eq!(
        StoryWorkbench::scenario_progress(&StorybookAutomationError::AutomationBusy),
        0
    );
}

#[test]
fn scenario_runner_uses_the_controller_attached_to_its_host() {
    let mut app = TestAppContext::single();
    app.update(gpui_component::init);
    let automation = crate::automation::StorybookAutomation::new();
    let window = app.open_window(size(px(400.), px(600.)), move |window, cx| {
        let state = cx.new(|cx| WorkbenchState::new_with_automation(None, Some(automation), cx));
        StoryWorkbench::new(state, WorkbenchTab::Scenarios, window, cx)
    });
    let mut visual_cx = VisualTestContext::from_window(*window, &app);
    let workbench = window
        .root(&mut visual_cx)
        .expect("workbench should be the window root");

    visual_cx.update(|window, cx| {
        workbench.update(cx, |workbench, cx| {
            workbench.run_scenario(
                "attached-story".to_owned(),
                StoryScenario::new("attached-scenario", "Attached scenario"),
                window,
                cx,
            );
        });
    });
    visual_cx.run_until_parked();

    assert!(workbench.read_with(&visual_cx, |workbench, _| {
        matches!(
            &workbench.scenario_run,
            Some(ScenarioRunState::Finished { result, .. })
                if matches!(
                    result.as_ref(),
                    Err(StorybookAutomationError::StoryNotFound { key })
                        if key == "attached-story"
                )
        )
    }));
}

#[gpui::test]
fn scenario_reset_toolbar_stays_visible_recreates_the_story_and_clears_the_result(
    cx: &mut TestAppContext,
) {
    SCENARIO_RESET_STORY_CREATIONS.store(0, Ordering::SeqCst);
    let window = cx.update(|cx| {
        gpui_component::init(cx);
        cx.open_window(Default::default(), |window, cx| {
            let story = StoryContainer::panel::<ScenarioResetStory>(window, cx);
            let state = cx.new(|cx| WorkbenchState::new(Some(story), cx));
            cx.new(|cx| StoryWorkbench::new(state, WorkbenchTab::Scenarios, window, cx))
        })
        .expect("scenario reset test window")
    });
    let mut visual_cx = VisualTestContext::from_window(window.into(), cx);
    let workbench = window
        .root(&mut visual_cx)
        .expect("workbench should be the window root");
    visual_cx.update(|window, cx| {
        workbench.update(cx, |workbench, cx| {
            workbench.scenario_run = Some(ScenarioRunState::Finished {
                story_key: String::new(),
                scenario: StoryScenario::new("restore-defaults", "Restore defaults"),
                result: Box::new(Err(StorybookAutomationError::AutomationBusy)),
            });
            cx.notify();
        });
        _ = window.draw(cx);
    });
    assert_eq!(SCENARIO_RESET_STORY_CREATIONS.load(Ordering::SeqCst), 1);

    let header_before = visual_cx
        .debug_bounds("workbench-scenarios-sticky-header")
        .expect("scenario toolbar should render");
    let reset = visual_cx
        .debug_bounds("workbench-scenarios-reset")
        .expect("scenario toolbar Reset button should render");
    let run = visual_cx
        .debug_bounds("run-scenario-restore-defaults")
        .expect("scenario Run fresh button should render");
    assert!(
        reset.bottom() <= run.top(),
        "Reset should render above scenario actions: reset={reset:?}, run={run:?}"
    );
    assert_eq!(
        visual_cx.debug_bounds("reset-scenario-restore-defaults"),
        None,
        "scenario rows should not repeat Reset"
    );

    let items = visual_cx
        .debug_bounds("workbench-scenarios-items")
        .expect("scenario rows should render in their own scroll region");
    visual_cx.simulate_event(ScrollWheelEvent {
        position: items.center(),
        delta: ScrollDelta::Pixels(point(px(0.), px(-180.))),
        ..Default::default()
    });
    visual_cx.update(|window, cx| {
        _ = window.draw(cx);
    });
    let header_after = visual_cx
        .debug_bounds("workbench-scenarios-sticky-header")
        .expect("scenario toolbar should remain rendered after scrolling");
    let run_after = visual_cx
        .debug_bounds("run-scenario-restore-defaults")
        .expect("scenario rows should remain laid out after scrolling");
    assert_eq!(header_after.origin, header_before.origin);
    assert!(
        run_after.origin.y < run.origin.y,
        "scenario rows should move beneath the fixed toolbar: before={run:?}, after={run_after:?}"
    );

    let reset = visual_cx
        .debug_bounds("workbench-scenarios-reset")
        .expect("scenario toolbar Reset button should remain visible");
    visual_cx.simulate_click(reset.center(), gpui::Modifiers::none());
    visual_cx.run_until_parked();
    visual_cx.update(|window, cx| {
        _ = window.draw(cx);
    });

    assert_eq!(SCENARIO_RESET_STORY_CREATIONS.load(Ordering::SeqCst), 2);
    assert!(workbench.read_with(&visual_cx, |workbench, _| {
        workbench.scenario_run.is_none()
    }));

    visual_cx.update(|window, cx| {
        workbench.update(cx, |workbench, cx| {
            workbench.scenario_run = Some(ScenarioRunState::Running {
                story_key: String::new(),
                scenario: StoryScenario::new("restore-defaults", "Restore defaults"),
            });
            cx.notify();
        });
        _ = window.draw(cx);
    });
    let reset = visual_cx
        .debug_bounds("workbench-scenarios-reset")
        .expect("disabled scenario Reset button should remain visible");
    visual_cx.simulate_click(reset.center(), gpui::Modifiers::none());
    visual_cx.run_until_parked();
    assert_eq!(
        SCENARIO_RESET_STORY_CREATIONS.load(Ordering::SeqCst),
        2,
        "Reset must be inert while a scenario is running"
    );
}
