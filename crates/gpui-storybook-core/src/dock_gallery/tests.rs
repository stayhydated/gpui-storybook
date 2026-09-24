use super::*;
use crate::controls::StoryControls;
use crate::registry::{RegisteredStoryMetadata, StoryKey, StoryName};
use crate::story::{Story, StoryScenario, StoryScenarioStep};
use gpui_kit::{TestAppContext, VisualTestContext};
use tokio::sync::oneshot;

struct DockScenarioStory {
    focus_handle: FocusHandle,
}

impl StoryControls for DockScenarioStory {}

impl Focusable for DockScenarioStory {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Story for DockScenarioStory {
    fn title(_: &App) -> String {
        "Dock scenario story".to_owned()
    }

    fn new_view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| Self {
            focus_handle: cx.focus_handle(),
        })
    }

    fn scenarios() -> Vec<StoryScenario> {
        vec![
            StoryScenario::new("focus", "Focus").step(StoryScenarioStep::new(
                "Focus the story",
                crate::automation::interaction::StoryInteractionStep::FocusNext,
            )),
        ]
    }
}

impl Render for DockScenarioStory {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

fn scenario_story(
    key: &'static str,
    klass: &'static str,
    window: &mut Window,
    cx: &mut App,
) -> Entity<StoryContainer> {
    let story = StoryContainer::panel::<DockScenarioStory>(window, cx);
    story.update(cx, |story, _| {
        story.story_klass = Some(klass.into());
        story.set_registration_metadata(RegisteredStoryMetadata::new(
            StoryKey::new(key),
            StoryName::new(klass),
            None,
            "crate",
            "/tmp/crate",
            "src/stories.rs",
            1,
        ));
    });
    story
}

/// Runs enough draw and next-frame cycles for scheduled dock and automation
/// work to settle.
fn settle(visual: &mut VisualTestContext) {
    for _ in 0..8 {
        visual.update(|window, cx| {
            _ = window.draw(cx);
        });
        visual.update(|window, cx| {
            window.simulate_next_frame(cx);
        });
    }
}

fn registered_story(dock_area: &Entity<DockArea>, key: &str) -> Entity<StoryContainer> {
    let registries = STORY_PANELS.lock().expect("story panel registry");
    registries
        .get(&dock_area.entity_id())
        .and_then(|panels| panels.get(key))
        .and_then(gpui_kit::WeakEntity::upgrade)
        .unwrap_or_else(|| panic!("story `{key}` should be registered"))
}

fn registered_group(dock_area: &Entity<DockArea>) -> Entity<StoryContainer> {
    let registries = STORY_PANELS.lock().expect("story panel registry");
    registries
        .get(&dock_area.entity_id())
        .and_then(|panels| {
            panels
                .iter()
                .find(|(key, _)| key.starts_with("__gpui_storybook_group__"))
                .and_then(|(_, story)| gpui_kit::WeakEntity::upgrade(story))
        })
        .unwrap_or_else(|| panic!("a variant group should be registered"))
}

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

#[gpui_kit::test]
fn sidebar_opened_story_runs_scenarios_against_the_rendered_route(cx: &mut TestAppContext) {
    cx.update(crate::story::init)
        .expect("Storybook runtime should initialize");
    let automation = crate::automation::StorybookAutomation::new();
    let automation_for_view = automation.clone();

    let window: gpui_kit::WindowHandle<StoryWorkspace> = cx.update(|cx| {
        cx.open_window(Default::default(), move |window, cx| {
            let other = scenario_story("crate-OtherStory", "OtherStory", window, cx);
            let scenario =
                scenario_story("crate-DockScenarioStory", "DockScenarioStory", window, cx);
            cx.new(|cx| {
                StoryWorkspace::new(
                    vec![other, scenario],
                    StorybookWindowUi::default(),
                    Some(automation_for_view),
                    window,
                    cx,
                )
            })
        })
        .expect("dock scenario window should open")
    });

    let mut visual = VisualTestContext::from_window(window.into(), cx);
    let workspace = window
        .root(&mut visual)
        .expect("workspace should be the window root");

    let (dock_area, workbench_state) = workspace.read_with(&visual, |workspace, _| {
        (
            workspace.dock_area.clone(),
            workspace.workbench_state.clone(),
        )
    });
    let scenario = registered_story(&dock_area, "crate-DockScenarioStory");

    visual.update(|window, cx| {
        let dock_area = dock_area.clone();
        let scenario = scenario.clone();
        let automation = automation.clone();
        // Mirror the sidebar's click listener, which defers `open_story`.
        window.defer(cx, move |window, cx| {
            StorySidebar::open_story(
                dock_area.downgrade(),
                scenario,
                Some(automation),
                window,
                cx,
            );
        });
    });
    settle(&mut visual);

    // Open another story, then return to the scenario story through the
    // sidebar so the reveal path (an already mounted, inactive panel) is used.
    let other = registered_story(&dock_area, "crate-OtherStory");
    visual.update(|window, cx| {
        StorySidebar::open_story(
            dock_area.downgrade(),
            other,
            Some(automation.clone()),
            window,
            cx,
        );
    });
    settle(&mut visual);
    visual.update(|window, cx| {
        StorySidebar::open_story(
            dock_area.downgrade(),
            scenario.clone(),
            Some(automation.clone()),
            window,
            cx,
        );
    });
    settle(&mut visual);

    assert_eq!(
        workbench_state.read_with(&visual, |state, cx| state
            .active_story()
            .and_then(|story| story.read(cx).story_key_label().map(str::to_owned))),
        Some("crate-DockScenarioStory".to_owned()),
        "the sidebar selection should drive the workbench's active story"
    );
    assert!(
        crate::capture_region::capture_region_bounds("crate-DockScenarioStory").is_some(),
        "the sidebar-opened story route should be rendered for scenarios"
    );

    let (response, mut result) = oneshot::channel();
    visual.update(|window, cx| {
        workspace.update(cx, |workspace, cx| {
            let request = StoryScenario::new("focus", "Focus")
                .step(StoryScenarioStep::new(
                    "Focus the story",
                    crate::automation::interaction::StoryInteractionStep::FocusNext,
                ))
                .interaction_request("crate-DockScenarioStory");
            workspace.handle_automation_command(
                StorybookAutomationCommand::RunSteps {
                    request_id: 1,
                    request,
                    fresh_story: true,
                    response,
                    progress: std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0)),
                    operation: automation
                        .begin_operation()
                        .expect("interaction operation should start"),
                },
                window,
                cx,
            );
        });
    });

    settle(&mut visual);
    settle(&mut visual);

    match result.try_recv() {
        Ok(Ok(snapshot)) => {
            assert_eq!(snapshot.story.key, "crate-DockScenarioStory");
        },
        Ok(Err(error)) => panic!("sidebar-opened scenario failed: {error}"),
        Err(error) => panic!("scenario did not complete: {error}"),
    }
}

#[gpui_kit::test]
fn sidebar_group_selection_preserves_the_active_variant(cx: &mut TestAppContext) {
    cx.update(crate::story::init)
        .expect("Storybook runtime should initialize");

    let window: gpui_kit::WindowHandle<StoryWorkspace> = cx.update(|cx| {
        cx.open_window(Default::default(), |window, cx| {
            let a = scenario_story("crate-GroupVariantA", "GroupVariantA", window, cx);
            let b = scenario_story("crate-GroupVariantB", "GroupVariantB", window, cx);
            let group = StoryContainer::variant_group("Grouped scenario", vec![a, b], window, cx);
            cx.new(|cx| {
                StoryWorkspace::new(vec![group], StorybookWindowUi::default(), None, window, cx)
            })
        })
        .expect("grouped dock window should open")
    });

    let mut visual = VisualTestContext::from_window(window.into(), cx);
    let workspace = window
        .root(&mut visual)
        .expect("workspace should be the window root");
    let (dock_area, workbench_state) = workspace.read_with(&visual, |workspace, _| {
        (
            workspace.dock_area.clone(),
            workspace.workbench_state.clone(),
        )
    });

    let variants = workbench_state.read_with(&visual, |state, cx| state.variants(cx));
    assert_eq!(variants.len(), 2, "the group should expose both variants");

    visual.update(|_, cx| {
        workbench_state.update(cx, |state, cx| {
            state.set_active_variant(variants[1].clone(), cx);
        });
    });
    settle(&mut visual);
    assert_eq!(
        workbench_state.read_with(&visual, |state, cx| state
            .active_story()
            .and_then(|story| story.read(cx).story_key_label().map(str::to_owned))),
        Some("crate-GroupVariantB".to_owned()),
        "selecting the variant should make it active"
    );

    let group = registered_group(&dock_area);
    visual.update(|window, cx| {
        StorySidebar::open_story(dock_area.downgrade(), group, None, window, cx);
    });
    settle(&mut visual);

    assert_eq!(
        workbench_state.read_with(&visual, |state, cx| state
            .active_story()
            .and_then(|story| story.read(cx).story_key_label().map(str::to_owned))),
        Some("crate-GroupVariantB".to_owned()),
        "clicking the sidebar group should not reset the active variant"
    );
}
