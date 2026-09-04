use super::*;

#[test]
fn action_debugger_formats_multi_stroke_bindings() {
    let binding = KeyBinding::new(
        "ctrl-k enter",
        SelectViewport {
            viewport: StoryViewportPreset::Mobile,
        },
        None,
    );
    let expected = if cfg!(target_os = "macos") {
        "^K enter"
    } else {
        "ctrl-K enter"
    };
    assert_eq!(format_key_binding(&binding), expected);
}

#[gpui::test]
fn action_debugger_keeps_only_selected_story_actions(cx: &mut TestAppContext) {
    let window = cx.update(|cx| {
        cx.open_window(Default::default(), |_, cx| cx.new(ActionScopeFixture::new))
            .expect("action scope test window")
    });
    let mut visual_cx = VisualTestContext::from_window(window.into(), cx);
    let fixture = window
        .root(&mut visual_cx)
        .expect("action scope fixture should be the window root");
    visual_cx.update(|window, cx| {
        _ = window.draw(cx);
    });
    let (shell_focus, story_focus) = fixture.read_with(&visual_cx, |fixture, _| {
        (fixture.shell_focus.clone(), fixture.story_focus.clone())
    });

    visual_cx.update(|window, _| {
        assert!(!is_story_scoped_action(
            &ShellAction,
            &story_focus,
            &shell_focus,
            window,
        ));
        assert!(is_story_scoped_action(
            &StoryAction,
            &story_focus,
            &shell_focus,
            window,
        ));
        assert!(!is_story_scoped_action(
            &NestedInputAction,
            &story_focus,
            &shell_focus,
            window,
        ));
    });
    let action_names = visual_cx.update(|window, cx| {
        story_scoped_actions(&story_focus, &shell_focus, window, cx)
            .into_iter()
            .map(|action| action.name())
            .filter(|name| {
                [
                    ShellAction.name(),
                    StoryAction.name(),
                    NestedInputAction.name(),
                ]
                .contains(name)
            })
            .collect::<Vec<_>>()
    });
    assert_eq!(action_names, vec![StoryAction.name()]);
}

#[gpui::test]
fn action_debugger_uses_the_story_root_instead_of_its_nested_interaction_focus(
    cx: &mut TestAppContext,
) {
    let window = cx.update(|cx| {
        gpui_component::init(cx);
        cx.open_window(Default::default(), |window, cx| {
            cx.new(|cx| StoryContainerActionFixture::new(window, cx))
        })
        .expect("story container action fixture window")
    });
    let mut visual_cx = VisualTestContext::from_window(window.into(), cx);
    let fixture = window
        .root(&mut visual_cx)
        .expect("story container action fixture should be the window root");
    visual_cx.update(|window, cx| {
        _ = window.draw(cx);
    });
    let (shell_focus, story) = fixture.read_with(&visual_cx, |fixture, _| {
        (fixture.shell_focus.clone(), fixture.story.clone())
    });
    let (interaction_focus, action_scope_focus) = visual_cx.update(|_, cx| {
        (
            story.focus_handle(cx),
            story
                .read(cx)
                .action_scope_focus_handle()
                .expect("scoped story should expose an action focus handle"),
        )
    });

    visual_cx.update(|window, _| {
        assert!(window.is_action_available_in(&NestedInputAction, &interaction_focus));
        assert!(!window.is_action_available_in(&NestedInputAction, &action_scope_focus));
        assert!(is_story_scoped_action(
            &StoryAction,
            &action_scope_focus,
            &shell_focus,
            window,
        ));
        assert!(!is_story_scoped_action(
            &ShellAction,
            &action_scope_focus,
            &shell_focus,
            window,
        ));
    });

    let action_names = visual_cx.update(|window, cx| {
        story_scoped_actions(&action_scope_focus, &shell_focus, window, cx)
            .into_iter()
            .map(|action| action.name())
            .filter(|name| {
                [
                    ShellAction.name(),
                    StoryAction.name(),
                    NestedInputAction.name(),
                ]
                .contains(name)
            })
            .collect::<Vec<_>>()
    });
    assert_eq!(action_names, vec![StoryAction.name()]);
}

#[gpui::test]
fn action_reset_toolbar_stays_visible_and_recreates_the_story(cx: &mut TestAppContext) {
    ACTION_RESET_STORY_CREATIONS.store(0, Ordering::SeqCst);
    let window = cx.update(|cx| {
        gpui_component::init(cx);
        let options = gpui::WindowOptions {
            window_bounds: Some(gpui::WindowBounds::Windowed(gpui::Bounds::centered(
                None,
                size(px(800.), px(360.)),
                cx,
            ))),
            ..Default::default()
        };
        cx.open_window(options, |window, cx| {
            cx.new(|cx| ActionResetWorkbenchFixture::new(window, cx))
        })
        .expect("action reset test window")
    });
    let mut visual_cx = VisualTestContext::from_window(window.into(), cx);
    let fixture = window
        .root(&mut visual_cx)
        .expect("fixture should be the window root");
    let workbench = fixture.read_with(&visual_cx, |fixture, _| fixture.workbench.clone());
    visual_cx.update(|window, cx| {
        _ = window.draw(cx);
        workbench.update(cx, |workbench, cx| {
            workbench.selected_tab = WorkbenchTab::Actions;
            workbench.scenario_run = Some(ScenarioRunState::Finished {
                story_key: String::new(),
                scenario: StoryScenario::new("finished", "Finished"),
                result: Box::new(Err(StorybookAutomationError::AutomationBusy)),
            });
            cx.notify();
        });
        _ = window.draw(cx);
    });
    assert_eq!(ACTION_RESET_STORY_CREATIONS.load(Ordering::SeqCst), 1);

    let header_before = visual_cx
        .debug_bounds("workbench-actions-sticky-header")
        .expect("Actions toolbar should render");
    let reset = visual_cx
        .debug_bounds("workbench-actions-reset")
        .expect("Actions toolbar Reset button should render");
    let first_action_selector = "dispatch-action-workbench_action_reset_test::ActionReset00";
    let dispatch = visual_cx
        .debug_bounds(first_action_selector)
        .expect("action Dispatch button should render");
    assert!(
        reset.bottom() <= dispatch.top(),
        "Reset should render above action rows: reset={reset:?}, dispatch={dispatch:?}"
    );

    let items = visual_cx
        .debug_bounds("workbench-actions-items")
        .expect("action rows should render in their own scroll region");
    visual_cx.simulate_event(ScrollWheelEvent {
        position: items.center(),
        delta: ScrollDelta::Pixels(point(px(0.), px(-180.))),
        ..Default::default()
    });
    visual_cx.update(|window, cx| {
        _ = window.draw(cx);
    });
    let header_after = visual_cx
        .debug_bounds("workbench-actions-sticky-header")
        .expect("Actions toolbar should remain rendered after scrolling");
    let dispatch_after = visual_cx
        .debug_bounds(first_action_selector)
        .expect("action rows should remain laid out after scrolling");
    assert_eq!(header_after.origin, header_before.origin);
    assert!(
        dispatch_after.origin.y < dispatch.origin.y,
        "action rows should move beneath the fixed toolbar: before={dispatch:?}, after={dispatch_after:?}"
    );

    let reset = visual_cx
        .debug_bounds("workbench-actions-reset")
        .expect("Actions toolbar Reset button should remain visible");
    visual_cx.simulate_click(reset.center(), gpui::Modifiers::none());
    visual_cx.run_until_parked();
    visual_cx.update(|window, cx| {
        _ = window.draw(cx);
    });
    assert_eq!(ACTION_RESET_STORY_CREATIONS.load(Ordering::SeqCst), 2);
    assert!(workbench.read_with(&visual_cx, |workbench, _| {
        workbench.scenario_run.is_none()
    }));
    assert!(
        visual_cx.debug_bounds(first_action_selector).is_some(),
        "Actions should rebind to the recreated story scope"
    );

    visual_cx.update(|window, cx| {
        workbench.update(cx, |workbench, cx| {
            workbench.scenario_run = Some(ScenarioRunState::Running {
                story_key: String::new(),
                scenario: StoryScenario::new("running", "Running"),
            });
            cx.notify();
        });
        _ = window.draw(cx);
    });
    let reset = visual_cx
        .debug_bounds("workbench-actions-reset")
        .expect("disabled Actions Reset button should remain visible");
    visual_cx.simulate_click(reset.center(), gpui::Modifiers::none());
    visual_cx.run_until_parked();
    assert_eq!(
        ACTION_RESET_STORY_CREATIONS.load(Ordering::SeqCst),
        2,
        "Actions Reset must be inert while a scenario is running"
    );
}
