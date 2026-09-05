use super::*;
use crate::controls::{
    ControlBounds, ControlError, ControlKind, ControlSpec, ControlValue, StoryControls,
};
use crate::registry::{RegisteredStoryMetadata, StoryKey, StoryName};
use crate::story::Story;
use crate::storybook_window_ui::StorybookWindowUi;
use gpui_kit::{Focusable, Modifiers, MouseButton, TestAppContext, VisualTestContext, point};
use tokio::sync::oneshot;

struct ControlledStory {
    focus_handle: gpui_kit::FocusHandle,
    enabled: bool,
}

impl StoryControls for ControlledStory {
    fn control_specs(&self) -> Vec<ControlSpec> {
        vec![ControlSpec {
            key: "enabled".to_owned(),
            label: "Enabled".to_owned(),
            description: String::new(),
            category: "Properties".to_owned(),
            kind: ControlKind::Checkbox,
            default: ControlValue::Boolean(false),
            bounds: ControlBounds::default(),
            options: Vec::new(),
        }]
    }

    fn control_value(&self, key: &str) -> Result<ControlValue, ControlError> {
        match key {
            "enabled" => Ok(ControlValue::Boolean(self.enabled)),
            _ => Err(ControlError::UnknownControl {
                key: key.to_owned(),
            }),
        }
    }

    fn set_control_value(&mut self, key: &str, value: ControlValue) -> Result<(), ControlError> {
        match (key, value) {
            ("enabled", ControlValue::Boolean(value)) => {
                self.enabled = value;
                Ok(())
            },
            _ => Err(ControlError::UnknownControl {
                key: key.to_owned(),
            }),
        }
    }
}

impl Focusable for ControlledStory {
    fn focus_handle(&self, _: &App) -> gpui_kit::FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for ControlledStory {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().child(self.enabled.to_string())
    }
}

impl Story for ControlledStory {
    fn title(_: &App) -> String {
        "Controlled".to_owned()
    }

    fn new_view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| Self {
            focus_handle: cx.focus_handle(),
            enabled: false,
        })
    }
}

fn story(
    key: &'static str,
    name: &'static str,
    title: &'static str,
    window: &mut Window,
    cx: &mut App,
) -> Entity<StoryContainer> {
    cx.new(|cx| {
        let mut story = StoryContainer::new(window, cx);
        story.name = title.into();
        story.set_registration_metadata(RegisteredStoryMetadata::new(
            StoryKey::new(key),
            StoryName::new(name),
            None,
            "crate",
            "/tmp/crate",
            "src/stories.rs",
            1,
        ));
        story
    })
}

#[gpui_kit::test]
fn gallery_selects_by_title_key_and_automation_command(cx: &mut App) {
    gpui_kit::init(cx);
    let automation = crate::automation::StorybookAutomation::new();
    let automation_for_view = automation.clone();
    let window: gpui_kit::WindowHandle<Gallery> = cx
        .open_window(Default::default(), move |window, cx| {
            let button = story("crate-ButtonStory", "ButtonStory", "Button", window, cx);
            let table = story("crate-TableStory", "TableStory", "Table", window, cx);
            Gallery::view_with_automation(
                vec![button, table],
                Some("TableStory"),
                automation_for_view,
                window,
                cx,
            )
        })
        .expect("gallery window should open");

    window
        .update(cx, |gallery, window, cx| {
            assert_eq!(gallery.active_index, Some(1));
            assert!(gallery.left_sidebar_visible);
            assert!(gallery.right_sidebar_visible);
            assert!(gallery.workbench_state.read(cx).automation().is_some());
            assert_eq!(
                gallery
                    .active_story_snapshot(cx)
                    .expect("table should be active")
                    .key,
                "crate-TableStory"
            );

            gallery.set_active_story("ButtonStory", cx);
            assert_eq!(gallery.active_index, Some(0));
            gallery.set_active_story("MissingStory", cx);
            assert_eq!(gallery.active_index, Some(0));

            let selected = gallery
                .set_active_story_by_key("crate-ButtonStory/with-icon", cx)
                .expect("substory key should select its parent story");
            assert_eq!(
                selected
                    .story
                    .expect("selected story should be returned")
                    .capture_route_id,
                "crate-ButtonStory/with-icon"
            );
            assert!(matches!(
                gallery.set_active_story_by_key("missing", cx),
                Err(StorybookAutomationError::StoryNotFound { key }) if key == "missing"
            ));

            let (response, mut result) = oneshot::channel();
            gallery.handle_automation_command(
                StorybookAutomationCommand::OpenStory {
                    key: "crate-TableStory".to_string(),
                    response,
                    _operation: automation
                        .begin_operation()
                        .expect("open operation should start"),
                },
                window,
                cx,
            );
            assert_eq!(
                result
                    .try_recv()
                    .expect("open response should be sent")
                    .expect("table should open")
                    .story
                    .expect("table snapshot should exist")
                    .key,
                "crate-TableStory"
            );

            let (response, mut result) = oneshot::channel();
            gallery.handle_automation_command(
                StorybookAutomationCommand::RunSteps {
                    request_id: 8,
                    request: crate::automation::StoryInteractionRequest {
                        story_key: Some("crate-ButtonStory".to_owned()),
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
            assert_eq!(
                gallery
                    .active_story_snapshot(cx)
                    .expect("invalid batch must not change routes")
                    .key,
                "crate-TableStory"
            );

            let (response, cancelled) = oneshot::channel();
            drop(cancelled);
            gallery.handle_automation_command(
                StorybookAutomationCommand::RunSteps {
                    request_id: 9,
                    request: crate::automation::StoryInteractionRequest {
                        story_key: Some("crate-ButtonStory".to_owned()),
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
                        .expect("cancelled interaction operation should start"),
                },
                window,
                cx,
            );
            assert_eq!(
                gallery
                    .active_story_snapshot(cx)
                    .expect("cancelled batch must not change routes")
                    .key,
                "crate-TableStory"
            );
            assert!(
                automation.begin_operation().is_ok(),
                "cancelled batch should release its guard"
            );

            gallery.stories.clear();
            gallery.active_index = None;
            gallery.workbench_state.update(cx, |state, cx| {
                state.set_active_story(None, cx);
            });
            automation.set_stories(Vec::new());
            let error = gallery
                .prepare_capture_current_story(&StoryScreenshotRequest::default(), window, cx)
                .expect_err("capture requires a selected story");
            assert!(matches!(
                error,
                StorybookAutomationError::CaptureUnavailable { message }
                    if message.contains("no current story")
            ));

            let (response, mut result) = oneshot::channel();
            gallery.handle_automation_command(
                StorybookAutomationCommand::CaptureCurrentStory {
                    request_id: 7,
                    request: StoryScreenshotRequest::default(),
                    response,
                    operation: automation
                        .begin_operation()
                        .expect("capture operation should start"),
                },
                window,
                cx,
            );
            assert!(matches!(
                result.try_recv().expect("capture error should be sent"),
                Err(StorybookAutomationError::CaptureUnavailable { .. })
            ));
        })
        .expect("gallery should update");
}

#[test]
fn sidebar_toggles_keep_responsive_canvas_centered_with_its_resize_gutter() {
    let mut app = TestAppContext::single();
    app.update(gpui_kit::init);
    let (_, cx) = app.add_window_view(move |window, cx| {
        let gallery = cx.new(|gallery_cx| {
            let story = story(
                "crate-ButtonStory",
                "ButtonStory",
                "Button",
                window,
                gallery_cx,
            );
            Gallery::new(vec![story], None, None, window, gallery_cx)
        });
        crate::story::StoryRoot::new(
            "Storybook",
            gallery,
            StorybookWindowUi::default(),
            window,
            cx,
        )
    });
    let draw = |cx: &mut VisualTestContext| {
        cx.run_until_parked();
        cx.update(|window, cx| {
            _ = window.draw(cx);
        });
        cx.run_until_parked();
        cx.update(|window, cx| {
            _ = window.draw(cx);
        });
    };
    let assert_canvas_centered_with_resize_gutter = |cx: &mut VisualTestContext| {
        let canvas = cx
            .debug_bounds("story-canvas")
            .expect("story canvas should render");
        let stage = cx
            .debug_bounds("story-canvas-stage")
            .expect("story canvas stage should render");
        let main_content = cx
            .debug_bounds("gallery-main-content")
            .expect("main content pane should render");
        assert_eq!(
            stage.left(),
            main_content.left(),
            "responsive stage {stage:?} must not have a left inset in {main_content:?}"
        );
        assert_eq!(
            stage.right(),
            main_content.right(),
            "responsive stage {stage:?} must not have a right inset in {main_content:?}"
        );
        assert_eq!(
            canvas.left() - stage.left(),
            stage.right() - canvas.right(),
            "responsive canvas {canvas:?} must have a symmetric gutter in {stage:?}"
        );
        assert!(
            canvas.left() > stage.left(),
            "responsive canvas {canvas:?} must keep its resize gutter in {stage:?}"
        );
        assert_eq!(
            canvas.center().x,
            main_content.center().x,
            "responsive canvas {canvas:?} must be centered in {main_content:?}"
        );
        if let Some(left_sidebar) = cx.debug_bounds("gallery-left-sidebar") {
            assert!(
                canvas.left() >= left_sidebar.right(),
                "canvas {canvas:?} must stay to the right of {left_sidebar:?}"
            );
        }
        if let Some(right_sidebar) = cx.debug_bounds("gallery-right-sidebar") {
            assert!(
                canvas.right() <= right_sidebar.left(),
                "canvas {canvas:?} must stay to the left of {right_sidebar:?}"
            );
        }
    };
    let click_toggle = |selector, cx: &mut VisualTestContext| {
        let bounds = cx
            .debug_bounds(selector)
            .expect("sidebar toggle should render");
        cx.simulate_click(bounds.center(), Modifiers::none());
        draw(cx);
    };

    draw(cx);
    assert!(
        cx.debug_bounds("workbench-viewport").is_some(),
        "viewport selector should remain in the workbench header"
    );
    assert_eq!(
        cx.debug_bounds("workbench-background"),
        None,
        "canvas background selector should not render in the workbench"
    );
    let left_toggle_bounds = cx
        .debug_bounds("gallery-toggle-left-sidebar")
        .expect("left sidebar toggle should render in the title bar");
    let right_toggle_bounds = cx
        .debug_bounds("gallery-toggle-right-sidebar")
        .expect("right sidebar toggle should render in the title bar");
    let settings_bounds = cx
        .debug_bounds("storybook-settings")
        .expect("settings should render in the title bar");
    assert!(left_toggle_bounds.right() <= right_toggle_bounds.left());
    assert!(right_toggle_bounds.right() <= settings_bounds.left());

    let left_width = cx
        .debug_bounds("gallery-left-sidebar")
        .expect("left sidebar should render")
        .size
        .width;
    let right_width = cx
        .debug_bounds("gallery-right-sidebar")
        .expect("right sidebar should render")
        .size
        .width;
    assert_canvas_centered_with_resize_gutter(cx);

    click_toggle("gallery-toggle-left-sidebar", cx);
    assert_eq!(cx.debug_bounds("gallery-left-sidebar"), None);
    assert!(cx.debug_bounds("gallery-right-sidebar").is_some());
    assert_canvas_centered_with_resize_gutter(cx);

    click_toggle("gallery-toggle-right-sidebar", cx);
    assert_eq!(cx.debug_bounds("gallery-left-sidebar"), None);
    assert_eq!(cx.debug_bounds("gallery-right-sidebar"), None);
    assert_canvas_centered_with_resize_gutter(cx);

    click_toggle("gallery-toggle-left-sidebar", cx);
    assert!(cx.debug_bounds("gallery-left-sidebar").is_some());
    assert_eq!(cx.debug_bounds("gallery-right-sidebar"), None);
    assert_canvas_centered_with_resize_gutter(cx);

    click_toggle("gallery-toggle-right-sidebar", cx);
    assert!(cx.debug_bounds("gallery-left-sidebar").is_some());
    assert!(cx.debug_bounds("gallery-right-sidebar").is_some());
    assert_canvas_centered_with_resize_gutter(cx);

    let left_bounds = cx
        .debug_bounds("gallery-left-sidebar")
        .expect("left sidebar should render before resizing");
    let resize_start = point(left_bounds.right(), left_bounds.center().y);
    cx.simulate_mouse_move(resize_start, None, Modifiers::none());
    cx.simulate_mouse_down(resize_start, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_move(
        point(resize_start.x + px(5.), resize_start.y),
        MouseButton::Left,
        Modifiers::none(),
    );
    cx.simulate_mouse_move(
        point(resize_start.x + px(30.), resize_start.y),
        MouseButton::Left,
        Modifiers::none(),
    );
    cx.simulate_mouse_up(
        point(resize_start.x + px(30.), resize_start.y),
        MouseButton::Left,
        Modifiers::none(),
    );
    draw(cx);
    let resized_left_width = cx
        .debug_bounds("gallery-left-sidebar")
        .expect("left sidebar should render after resizing")
        .size
        .width;
    assert!(resized_left_width > left_width);
    assert_canvas_centered_with_resize_gutter(cx);

    let right_bounds = cx
        .debug_bounds("gallery-right-sidebar")
        .expect("right sidebar should render before resizing");
    let resize_start = point(right_bounds.left(), right_bounds.center().y);
    cx.simulate_mouse_move(resize_start, None, Modifiers::none());
    cx.simulate_mouse_down(resize_start, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_move(
        point(resize_start.x - px(5.), resize_start.y),
        MouseButton::Left,
        Modifiers::none(),
    );
    cx.simulate_mouse_move(
        point(resize_start.x - px(30.), resize_start.y),
        MouseButton::Left,
        Modifiers::none(),
    );
    cx.simulate_mouse_up(
        point(resize_start.x - px(30.), resize_start.y),
        MouseButton::Left,
        Modifiers::none(),
    );
    draw(cx);
    let resized_right_width = cx
        .debug_bounds("gallery-right-sidebar")
        .expect("right sidebar should render after resizing")
        .size
        .width;
    assert!(resized_right_width > right_width);
    assert_canvas_centered_with_resize_gutter(cx);
}

#[gpui_kit::test]
fn empty_gallery_has_no_active_story(cx: &mut App) {
    gpui_kit::init(cx);
    let window: gpui_kit::WindowHandle<Gallery> = cx
        .open_window(Default::default(), |window, cx| {
            Gallery::view(Vec::new(), Some("Missing"), window, cx)
        })
        .expect("empty gallery window should open");

    window
        .update(cx, |gallery, _, cx| {
            assert_eq!(gallery.active_index, None);
            assert_eq!(gallery.active_story_snapshot(cx), None);
            gallery.sync_automation_stories(cx);
            gallery.confirm_active_story(cx);
        })
        .expect("empty gallery should update");
}

#[gpui_kit::test]
fn automation_controls_read_set_and_reset_the_live_entity(cx: &mut App) {
    gpui_kit::init(cx);
    let automation = crate::automation::StorybookAutomation::new();
    let automation_for_view = automation.clone();
    let window: gpui_kit::WindowHandle<Gallery> = cx
        .open_window(Default::default(), move |window, cx| {
            let story = StoryContainer::panel::<ControlledStory>(window, cx);
            story.update(cx, |story, _| {
                story.set_registration_metadata(RegisteredStoryMetadata::new(
                    StoryKey::new("crate-ControlledStory"),
                    StoryName::new("ControlledStory"),
                    None,
                    "crate",
                    "/tmp/crate",
                    "src/controlled.rs",
                    1,
                ));
            });
            Gallery::view_with_automation(vec![story], None, automation_for_view, window, cx)
        })
        .expect("gallery window should open");

    window
        .update(cx, |gallery, window, cx| {
            let (response, mut result) = oneshot::channel();
            gallery.handle_automation_command(
                StorybookAutomationCommand::ReadControls { response },
                window,
                cx,
            );
            let snapshot = result
                .try_recv()
                .expect("read response is sent")
                .expect("controls are available");
            assert_eq!(snapshot.controls[0].value, ControlValue::Boolean(false));

            let (response, mut result) = oneshot::channel();
            gallery.handle_automation_command(
                StorybookAutomationCommand::SetControl {
                    key: "enabled".to_owned(),
                    value: ControlValue::Boolean(true),
                    response,
                    _operation: automation
                        .begin_operation()
                        .expect("control operation should start"),
                },
                window,
                cx,
            );
            let snapshot = result
                .try_recv()
                .expect("set response is sent")
                .expect("control update succeeds");
            assert_eq!(snapshot.controls[0].value, ControlValue::Boolean(true));

            let (response, mut result) = oneshot::channel();
            gallery.handle_automation_command(
                StorybookAutomationCommand::ResetControl {
                    key: None,
                    response,
                    _operation: automation
                        .begin_operation()
                        .expect("reset operation should start"),
                },
                window,
                cx,
            );
            let snapshot = result
                .try_recv()
                .expect("reset response is sent")
                .expect("control reset succeeds");
            assert_eq!(snapshot.controls[0].value, ControlValue::Boolean(false));
        })
        .expect("gallery should update");
}

#[gpui_kit::test]
fn grouped_route_selects_the_exact_workbench_variant(cx: &mut App) {
    gpui_kit::init(cx);
    let automation = crate::automation::StorybookAutomation::new();
    let window: gpui_kit::WindowHandle<Gallery> = cx
        .open_window(Default::default(), move |window, cx| {
            let primary = story(
                "crate-PrimaryButtonStory",
                "PrimaryButtonStory",
                "Button",
                window,
                cx,
            );
            let danger = story(
                "crate-DangerButtonStory",
                "DangerButtonStory",
                "Button",
                window,
                cx,
            );
            let grouped =
                StoryContainer::variant_group("Button", vec![primary, danger], window, cx);
            Gallery::view_with_automation(vec![grouped], None, automation, window, cx)
        })
        .expect("grouped gallery window should open");

    window
        .update(cx, |gallery, _, cx| {
            gallery
                .set_active_story_by_key("crate-DangerButtonStory", cx)
                .expect("member route should select its group");
            let active = gallery
                .workbench_state
                .read(cx)
                .active_story()
                .expect("active member exists");
            assert_eq!(
                active.read(cx).story_key_label(),
                Some("crate-DangerButtonStory")
            );
        })
        .expect("grouped gallery should update");
}

#[gpui_kit::test]
fn separate_windows_keep_control_entities_independent(cx: &mut App) {
    gpui_kit::init(cx);
    let open = |cx: &mut App| {
        cx.open_window(Default::default(), |window, cx| {
            let story = StoryContainer::panel::<ControlledStory>(window, cx);
            Gallery::view(vec![story], None, window, cx)
        })
        .expect("gallery window should open")
    };
    let first: gpui_kit::WindowHandle<Gallery> = open(cx);
    let second: gpui_kit::WindowHandle<Gallery> = open(cx);

    first
        .update(cx, |gallery, _, cx| {
            let story = gallery
                .workbench_state
                .read(cx)
                .active_story()
                .expect("first story is active");
            story
                .read(cx)
                .control_target()
                .expect("first story has controls")
                .set("enabled", ControlValue::Boolean(true), cx)
                .expect("first control update succeeds");
        })
        .expect("first gallery should update");

    second
        .update(cx, |gallery, _, cx| {
            let story = gallery
                .workbench_state
                .read(cx)
                .active_story()
                .expect("second story is active");
            assert_eq!(
                story
                    .read(cx)
                    .control_target()
                    .expect("second story has controls")
                    .value("enabled", cx),
                Ok(ControlValue::Boolean(false))
            );
        })
        .expect("second gallery should update");
}

#[gpui_kit::test]
fn only_the_window_that_claims_the_default_controller_can_run_scenarios(cx: &mut App) {
    gpui_kit::init(cx);
    let automation = crate::automation::StorybookAutomation::new();
    crate::automation::set_default_storybook_automation(cx, automation);
    let open = |cx: &mut App| {
        cx.open_window(Default::default(), |window, cx| {
            Gallery::view(Vec::new(), None, window, cx)
        })
        .expect("gallery window should open")
    };
    let first: gpui_kit::WindowHandle<Gallery> = open(cx);
    let second: gpui_kit::WindowHandle<Gallery> = open(cx);

    first
        .update(cx, |gallery, _, cx| {
            assert!(gallery.automation.is_some());
            assert!(gallery.workbench_state.read(cx).automation().is_some());
        })
        .expect("first gallery should own the controller host");
    second
        .update(cx, |gallery, _, cx| {
            assert!(gallery.automation.is_none());
            assert!(gallery.workbench_state.read(cx).automation().is_none());
        })
        .expect("second gallery should reject the claimed controller");
}
