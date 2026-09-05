use super::*;
use tokio::sync::oneshot;

#[gpui_kit::test]
fn default_layout_contains_open_versioned_right_workbench(cx: &mut App) {
    gpui_kit::init(cx);
    let window: gpui_kit::WindowHandle<DockArea> = cx
        .open_window(Default::default(), |window, cx| {
            let dock_area = cx.new(|cx| {
                DockArea::new(MAIN_DOCK_AREA.id, Some(MAIN_DOCK_AREA.version), window, cx)
            });
            let state = cx.new(|cx| WorkbenchState::new(None, cx));
            register_workbench_state(&dock_area.downgrade(), &state);
            StoryWorkspace::reset_default_layout(dock_area.downgrade(), &[], None, window, cx);
            dock_area
        })
        .expect("dock test window should open");

    window
        .update(cx, |dock_area, _, cx| {
            let state = dock_area.dump(cx);
            assert_eq!(state.version, Some(7));
            let json = serde_json::to_string(&state).expect("dock layout serializes");
            assert!(json.contains("StoryWorkbench"));
            assert!(json.contains("right_dock"));
            assert!(json.contains("320"));
        })
        .expect("dock test window should update");
}

#[gpui_kit::test]
fn grouped_story_variants_open_as_individual_tabs(cx: &mut App) {
    crate::story::init(cx).expect("Storybook runtime should initialize");
    let window: gpui_kit::WindowHandle<StoryWorkspace> = cx
        .open_window(Default::default(), |window, cx| {
            let mut variant = |description: &str, klass: &str, cx: &mut App| {
                cx.new(|cx| {
                    let mut story = StoryContainer::new(window, cx);
                    story.name = "Button".into();
                    story.description = description.to_owned().into();
                    story.story_klass = Some(klass.to_owned().into());
                    story
                })
            };
            let primary = variant("Primary variant", "PrimaryButtonStory", cx);
            let danger = variant("Danger variant", "DangerButtonStory", cx);
            let group = StoryContainer::variant_group("Button", vec![primary, danger], window, cx);
            cx.new(|cx| {
                StoryWorkspace::new(vec![group], StorybookWindowUi::default(), None, window, cx)
            })
        })
        .expect("grouped dock window should open");

    let (dock_area, workbench_state, group, variants) = window
        .update(cx, |workspace, _, cx| {
            let state = workspace.workbench_state.read(cx);
            (
                workspace.dock_area.clone(),
                workspace.workbench_state.clone(),
                state
                    .active_group()
                    .expect("variant group should be active"),
                state.variants(cx),
            )
        })
        .expect("grouped dock state should be readable");
    window
        .update(cx, |_, window, cx| {
            StorySidebar::open_story(dock_area.downgrade(), group, None, window, cx);
        })
        .expect("first grouped variant should open");
    workbench_state.update(cx, |state, cx| {
        state.set_active_variant(variants[1].clone(), cx);
    });

    window
        .update(cx, |workspace, _, cx| {
            let state = workspace.dock_area.read(cx).dump(cx);
            let json = serde_json::to_string(&state).expect("dock layout serializes");
            assert!(json.contains("PrimaryButtonStory"));
            assert!(json.contains("DangerButtonStory"));
            assert!(!json.contains("__gpui_storybook_group__:"));
        })
        .expect("grouped member tabs should be mounted");
}

#[gpui_kit::test]
fn dock_host_rejects_an_invalid_batch_before_route_preparation(cx: &mut App) {
    gpui_kit::init(cx);
    let automation = crate::automation::StorybookAutomation::new();
    let automation_for_view = automation.clone();
    let window: gpui_kit::WindowHandle<StoryWorkspace> = cx
        .open_window(Default::default(), move |window, cx| {
            StoryWorkspace::view_with_automation(Vec::new(), automation_for_view, window, cx)
        })
        .expect("dock automation test window should open");

    window
        .update(cx, |workspace, window, cx| {
            assert!(workspace.workbench_state.read(cx).automation().is_some());
            let (response, mut result) = oneshot::channel();
            workspace.handle_automation_command(
                StorybookAutomationCommand::RunSteps {
                    request_id: 9,
                    request: crate::automation::StoryInteractionRequest {
                        story_key: Some("missing-route".to_owned()),
                        controls: BTreeMap::new(),
                        width: None,
                        height: None,
                        viewport: None,
                        presentation: None,
                        steps: vec![crate::automation::StoryInteractionStep::DispatchAction {
                            name: "storybook_test::MissingAction".to_owned(),
                            args: None,
                        }],
                        postconditions: Vec::new(),
                        capture: None,
                    },
                    fresh_story: false,
                    response,
                    progress: std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0)),
                    operation: automation
                        .begin_operation()
                        .expect("interaction operation should start"),
                },
                window,
                cx,
            );

            assert!(matches!(
                result.try_recv().expect("interaction error should be sent"),
                Err(StorybookAutomationError::InvalidInteractionStep { step_index: 0, .. })
            ));
            assert_eq!(automation.current_story().story, None);

            let (response, mut result) = oneshot::channel();
            workspace.handle_automation_command(
                StorybookAutomationCommand::RunSteps {
                    request_id: 10,
                    request: crate::automation::StoryInteractionRequest {
                        story_key: None,
                        controls: BTreeMap::new(),
                        width: None,
                        height: None,
                        viewport: None,
                        presentation: None,
                        steps: vec![crate::automation::StoryInteractionStep::FocusNext],
                        postconditions: Vec::new(),
                        capture: None,
                    },
                    fresh_story: false,
                    response,
                    progress: std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0)),
                    operation: automation
                        .begin_operation()
                        .expect("interaction operation should restart"),
                },
                window,
                cx,
            );
            assert!(matches!(
                result
                    .try_recv()
                    .expect("missing-story error should be sent"),
                Err(StorybookAutomationError::NoActiveStory)
            ));
            assert!(
                automation.begin_operation().is_ok(),
                "a preparation failure should release the operation guard"
            );
        })
        .expect("dock host should handle the invalid batch");
}
