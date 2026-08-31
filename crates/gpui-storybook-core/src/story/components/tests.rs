use super::container::story_group_klass;
use super::*;
use crate::registry::{StoryKey, StoryName, StorySectionName};
use crate::workbench::WorkbenchState;
use gpui::{
    Modifiers, MouseButton, ScrollDelta, ScrollWheelEvent, TestAppContext, VisualTestContext, div,
    point, px,
};

enum DemoSubstory {
    WithIcon,
}

impl Substory for DemoSubstory {
    fn capture_key(&self) -> &'static str {
        "with-icon"
    }

    fn title(&self) -> SharedString {
        "With Icon".into()
    }
}

struct DefaultStoryContract;

struct TallStoryContent;

impl StoryControls for DefaultStoryContract {}

impl Focusable for DefaultStoryContract {
    fn focus_handle(&self, _: &App) -> gpui::FocusHandle {
        unreachable!("the static Story defaults do not require a focus handle")
    }
}

impl Render for DefaultStoryContract {
    fn render(&mut self, _: &mut Window, _: &mut gpui::Context<Self>) -> impl IntoElement {
        div()
    }
}

impl Story for DefaultStoryContract {
    fn title(_: &App) -> String {
        "Default Story".to_string()
    }

    fn new_view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|_| DefaultStoryContract)
    }
}

struct RecreatedStory {
    focus_handle: gpui::FocusHandle,
    count: usize,
}

impl StoryControls for RecreatedStory {}

impl Focusable for RecreatedStory {
    fn focus_handle(&self, _: &App) -> gpui::FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for RecreatedStory {
    fn render(&mut self, _: &mut Window, _: &mut gpui::Context<Self>) -> impl IntoElement {
        div()
    }
}

impl Story for RecreatedStory {
    fn title(_: &App) -> String {
        "Recreated Story".to_string()
    }

    fn new_view(_: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| Self {
            focus_handle: cx.focus_handle(),
            count: 0,
        })
    }
}

impl Render for TallStoryContent {
    fn render(&mut self, _: &mut Window, _: &mut gpui::Context<Self>) -> impl IntoElement {
        v_flex().w_full().child(div().h(px(1200.))).child(
            div()
                .debug_selector(|| "tall-story-bottom".to_owned())
                .h(px(32.)),
        )
    }
}

#[test]
fn section_titles_support_visible_and_stable_capture_identity() {
    let (title, key) = StorySectionTitle::new("Visible").into_parts();
    assert_eq!(title.as_ref(), "Visible");
    assert_eq!(key, None);

    let (title, key) = StorySectionTitle::with_capture_key("stable", "Visible").into_parts();
    assert_eq!(title.as_ref(), "Visible");
    assert_eq!(key.as_deref(), Some("stable"));

    for descriptor in [
        StorySectionTitle::from("Borrowed"),
        StorySectionTitle::from(String::from("Owned")),
        StorySectionTitle::from(SharedString::from("Shared")),
    ] {
        assert_eq!(descriptor.capture_key, None);
    }

    let descriptor = StorySectionTitle::from(DemoSubstory::WithIcon);
    let base = StorySectionBase::new(descriptor);
    assert_eq!(base.title().as_ref(), "With Icon");
    assert_eq!(base.capture_key().map(AsRef::as_ref), Some("with-icon"));
    let _captured = base.capture(div());

    let visible = StorySectionBase::new("Visible");
    assert_eq!(visible.title().as_ref(), "Visible");
    assert_eq!(visible.capture_key(), None);
    let _captured = visible.capture(div());
}

#[test]
fn styled_section_builder_collects_subtitles_children_and_widths() {
    let mut section = section("Examples")
        .sub_title(div().child("Subtitle"))
        .max_w_md()
        .max_w_lg()
        .max_w_xl()
        .max_w_2xl();
    section.extend([div().child("First").into_any_element()]);
    let _style = section.style();

    assert_eq!(section.capture.title().as_ref(), "Examples");
    assert_eq!(section.sub_title.len(), 1);
    assert_eq!(section.children.len(), 1);
}

#[test]
fn story_group_class_round_trips_sorted_members() {
    assert_eq!(parse_story_group_klass("ButtonStory"), None);
    assert_eq!(
        parse_story_group_klass(STORY_GROUP_KLASS_PREFIX),
        Some(Vec::new())
    );
    assert_eq!(
        parse_story_group_klass("__gpui_storybook_group__:TableStory||ButtonStory"),
        Some(vec!["TableStory".to_string(), "ButtonStory".to_string()])
    );
}

#[gpui::test]
fn story_trait_defaults_are_stable(cx: &mut App) {
    assert_eq!(DefaultStoryContract::klass(), "DefaultStoryContract");
    assert_eq!(DefaultStoryContract::title(cx), "Default Story");
    assert_eq!(DefaultStoryContract::description(cx), "");
    assert!(DefaultStoryContract::closable());
    assert!(DefaultStoryContract::zoomable().is_some());
    assert_eq!(DefaultStoryContract::title_bg(), None);
    let story = DefaultStoryContract;
    assert_eq!(story.action_scope_focus_handle(cx), None);
    assert!(DefaultStoryContract::scenarios().is_empty());
}

#[gpui::test]
fn scenario_recreation_starts_each_run_from_constructor_defaults(cx: &mut App) {
    gpui_component::init(cx);
    let window: gpui::WindowHandle<StoryContainer> = cx
        .open_window(Default::default(), |window, cx| {
            StoryContainer::panel::<RecreatedStory>(window, cx)
        })
        .expect("test window should open");

    window
        .update(cx, |container, window, cx| {
            let story = container
                .story
                .clone()
                .expect("panel should contain the concrete story")
                .downcast::<RecreatedStory>()
                .expect("panel should contain RecreatedStory");
            story.update(cx, |story, _| story.count = 7);
            assert_eq!(story.read(cx).count, 7);

            assert!(container.recreate_for_scenario(window, cx));
            let story = container
                .story
                .clone()
                .expect("recreated panel should contain the concrete story")
                .downcast::<RecreatedStory>()
                .expect("recreated panel should contain RecreatedStory");
            assert_eq!(story.read(cx).count, 0);

            story.update(cx, |story, _| story.count = 11);
            assert_eq!(story.read(cx).count, 11);
            assert!(container.recreate_for_scenario(window, cx));
            let story = container
                .story
                .clone()
                .expect("second recreation should contain the concrete story")
                .downcast::<RecreatedStory>()
                .expect("second recreation should contain RecreatedStory");
            assert_eq!(story.read(cx).count, 0);
        })
        .expect("story should recreate successfully");
}

#[gpui::test]
fn story_container_builders_and_metadata_expose_runtime_contract(cx: &mut App) {
    gpui_component::init(cx);
    let window: gpui::WindowHandle<StoryContainer> = cx
        .open_window(Default::default(), |window, cx| {
            cx.new(|cx| StoryContainer::new(window, cx))
        })
        .expect("test window should open");

    window
        .update(cx, |container, window, cx| {
            assert_eq!(container.sidebar_group(), None);
            assert_eq!(container.sidebar_section(), None);
            assert_eq!(container.display_title(cx), "");
            assert_eq!(container.display_description(cx), "");

            *container = StoryContainer::new(window, cx)
                .group("Examples")
                .section("Components")
                .width(px(800.))
                .height(px(600.));
            container.name = "Button".into();
            container.description = "Button states".into();
            assert_eq!(container.sidebar_group().as_deref(), Some("Examples"));
            assert_eq!(container.sidebar_section().as_deref(), Some("Components"));
            assert_eq!(container.display_title(cx), "Button");
            assert_eq!(container.display_description(cx), "Button states");
            assert_eq!(container.width, Some(px(800.)));
            assert_eq!(container.height, Some(px(600.)));

            let metadata = RegisteredStoryMetadata::new(
                StoryKey::new("crate-ButtonStory"),
                StoryName::new("ButtonStory"),
                Some(StorySectionName::new("Components")),
                "crate",
                "/tmp/crate",
                "src/button.rs",
                42,
            );
            container.set_registration_metadata(metadata);
            assert_eq!(container.registration_metadata(), Some(metadata));
            assert_eq!(
                container.story_key(),
                Some(StoryKey::new("crate-ButtonStory"))
            );
            assert_eq!(container.story_name(), Some(StoryName::new("ButtonStory")));
            assert_eq!(container.story_key_label(), Some("crate-ButtonStory"));
            assert_eq!(container.story_name_label(), Some("ButtonStory"));
            assert_eq!(container.crate_name_label(), Some("crate"));
            assert_eq!(container.source_file_label(), Some("src/button.rs"));
            assert_eq!(container.source_line(), Some(42));

            container.title_fn = Some(Box::new(|_| "Localized Button".to_string()));
            container.description_fn = Some(Box::new(|_| "Localized description".to_string()));
            assert_eq!(container.display_title(cx), "Localized Button");
            assert_eq!(container.display_description(cx), "Localized description");
        })
        .expect("container should update");
}

#[test]
fn tall_stories_scroll_inside_canvas_without_panning_viewport_frame() {
    let mut app = TestAppContext::single();
    app.update(gpui_component::init);
    let (container, cx) = app.add_window_view(|window, cx| -> StoryContainer {
        let story = cx.new(|_| TallStoryContent);
        let mut container = StoryContainer::new(window, cx).story(story.into(), "TallStoryContent");
        container.set_presentation(StoryPresentation {
            viewport: crate::presentation::StoryViewportPreset::Responsive,
            background: StoryCanvasBackground::Theme,
        });
        container
    });

    let draw = |cx: &mut VisualTestContext| {
        cx.run_until_parked();
        cx.update(|window, cx| {
            _ = window.draw(cx);
        });
    };

    draw(cx);
    let story_viewport = cx
        .debug_bounds("story-content-scroll-region")
        .expect("story content viewport should render");
    let bottom_before = cx
        .debug_bounds("tall-story-bottom")
        .expect("tall story content should render beyond the viewport");
    cx.read(|cx| {
        let container = container.read(cx);
        assert!(container.story_scroll_handle.max_offset().y > px(0.));
        assert_eq!(container.scroll_handle.max_offset(), point(px(0.), px(0.)));
    });

    cx.simulate_event(ScrollWheelEvent {
        position: story_viewport.center(),
        delta: ScrollDelta::Pixels(point(px(0.), px(-120.))),
        ..Default::default()
    });
    draw(cx);

    let bottom_after = cx
        .debug_bounds("tall-story-bottom")
        .expect("tall story content should remain rendered after scrolling");
    assert_eq!(bottom_after.origin.y, bottom_before.origin.y - px(120.));
    cx.read(|cx| {
        let container = container.read(cx);
        assert_eq!(
            container.story_scroll_handle.offset(),
            point(px(0.), px(-120.))
        );
        assert_eq!(container.scroll_handle.offset(), point(px(0.), px(0.)));
    });
}

#[test]
fn viewport_presets_size_and_center_the_canvas() {
    let mut app = TestAppContext::single();
    app.update(gpui_component::init);
    let (container, cx) = app.add_window_view(|window, cx| -> StoryContainer {
        let mut container = StoryContainer::new(window, cx);
        container.set_presentation(StoryPresentation {
            viewport: crate::presentation::StoryViewportPreset::Responsive,
            background: StoryCanvasBackground::Theme,
        });
        container
    });

    let draw = |cx: &mut VisualTestContext| {
        cx.run_until_parked();
        cx.update(|window, cx| {
            _ = window.draw(cx);
        });
    };

    draw(cx);
    let initial_canvas_bounds = cx
        .debug_bounds("story-canvas")
        .expect("responsive canvas should render");
    let initial_stage_bounds = cx
        .debug_bounds("story-canvas-stage")
        .expect("responsive canvas stage should render");
    let scroll_region_bounds = cx
        .debug_bounds("story-container-scroll-region")
        .expect("story scroll region should render");
    assert_eq!(initial_stage_bounds, scroll_region_bounds);
    assert_eq!(
        initial_canvas_bounds.size,
        gpui::size(
            initial_stage_bounds.size.width - STORY_CANVAS_RESIZE_GUTTER * 2.,
            initial_stage_bounds.size.height - STORY_CANVAS_RESIZE_GUTTER * 2.
        )
    );
    assert_eq!(
        initial_canvas_bounds.center(),
        initial_stage_bounds.center()
    );
    assert_eq!(
        cx.debug_bounds("story-canvas-border"),
        cx.debug_bounds("story-canvas")
    );
    assert!(cx.debug_bounds("story-canvas-resize-width").is_some());
    assert!(cx.debug_bounds("story-canvas-resize-height").is_some());
    assert!(cx.debug_bounds("story-canvas-resize-corner").is_some());

    for (viewport, expected_size) in [
        (
            crate::presentation::StoryViewportPreset::Mobile,
            gpui::size(px(390.), px(844.)),
        ),
        (
            crate::presentation::StoryViewportPreset::Tablet,
            gpui::size(px(768.), px(1024.)),
        ),
        (
            crate::presentation::StoryViewportPreset::Desktop,
            gpui::size(px(1440.), px(900.)),
        ),
    ] {
        container.update(cx, |container, cx| {
            container.set_presentation(StoryPresentation {
                viewport,
                background: StoryCanvasBackground::Theme,
            });
            cx.notify();
        });
        draw(cx);

        let canvas_bounds = cx
            .debug_bounds("story-canvas")
            .expect("fixed canvas should render");
        let stage_bounds = cx
            .debug_bounds("story-canvas-stage")
            .expect("centered canvas stage should render");
        assert_eq!(canvas_bounds.size, expected_size);
        assert_eq!(canvas_bounds.center(), stage_bounds.center());
        assert_eq!(
            cx.debug_bounds("story-canvas-border")
                .expect("canvas border should render"),
            canvas_bounds
        );
        assert_eq!(cx.debug_bounds("story-canvas-resize-corner"), None);
    }

    let workbench = cx.new(|cx| WorkbenchState::new(Some(container.clone()), cx));
    workbench.update(cx, |state, cx| {
        state.set_viewport(crate::presentation::StoryViewportPreset::Mobile, cx);
        state.set_viewport(crate::presentation::StoryViewportPreset::Responsive, cx);
    });
    draw(cx);

    let canvas_bounds = cx
        .debug_bounds("story-canvas")
        .expect("inherited responsive canvas should render");
    let stage_bounds = cx
        .debug_bounds("story-canvas-stage")
        .expect("responsive canvas stage should render");
    assert_eq!(canvas_bounds.size, gpui::size(px(390.), px(844.)));
    assert_eq!(canvas_bounds.center(), stage_bounds.center());
    assert!(cx.debug_bounds("story-canvas-resize-corner").is_some());

    let resize_start = cx
        .debug_bounds("story-canvas-resize-corner")
        .expect("responsive corner resize handle should render")
        .center();
    cx.simulate_mouse_move(resize_start, None, Modifiers::none());
    cx.simulate_mouse_down(resize_start, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_move(
        point(resize_start.x + px(5.), resize_start.y + px(5.)),
        MouseButton::Left,
        Modifiers::none(),
    );
    cx.simulate_mouse_move(
        point(resize_start.x + px(30.), resize_start.y + px(25.)),
        MouseButton::Left,
        Modifiers::none(),
    );
    draw(cx);
    let first_resized_canvas = cx
        .debug_bounds("story-canvas")
        .expect("responsive canvas should resize during the drag");
    assert_eq!(
        first_resized_canvas.size,
        gpui::size(
            canvas_bounds.size.width + px(60.),
            canvas_bounds.size.height + px(50.)
        )
    );

    cx.simulate_mouse_move(
        point(resize_start.x + px(50.), resize_start.y + px(40.)),
        MouseButton::Left,
        Modifiers::none(),
    );
    draw(cx);
    let second_resized_canvas = cx
        .debug_bounds("story-canvas")
        .expect("responsive canvas should keep resizing during the drag");
    assert_eq!(
        second_resized_canvas.size,
        gpui::size(
            canvas_bounds.size.width + px(100.),
            canvas_bounds.size.height + px(80.)
        )
    );

    cx.simulate_mouse_up(
        point(resize_start.x + px(50.), resize_start.y + px(40.)),
        MouseButton::Left,
        Modifiers::none(),
    );
    draw(cx);
    let resized_canvas = cx
        .debug_bounds("story-canvas")
        .expect("resized responsive canvas should render");
    assert_eq!(resized_canvas.size, second_resized_canvas.size);
    assert_eq!(
        cx.read(|cx| workbench.read(cx).responsive_size()),
        Some(resized_canvas.size)
    );

    container.update(cx, |container, cx| {
        container.set_responsive_size(Some(gpui::size(px(2000.), px(1500.))));
        cx.notify();
    });
    draw(cx);
    let oversized_canvas = cx
        .debug_bounds("story-canvas")
        .expect("oversized responsive canvas should render");
    let oversized_stage = cx
        .debug_bounds("story-canvas-stage")
        .expect("oversized responsive canvas stage should render");
    assert_eq!(
        oversized_canvas.origin.x - oversized_stage.origin.x,
        STORY_CANVAS_RESIZE_GUTTER
    );
    assert_eq!(
        oversized_canvas.origin.y - oversized_stage.origin.y,
        STORY_CANVAS_RESIZE_GUTTER
    );
    assert_eq!(
        oversized_stage.right() - oversized_canvas.right(),
        STORY_CANVAS_RESIZE_GUTTER
    );
    assert_eq!(
        oversized_stage.bottom() - oversized_canvas.bottom(),
        STORY_CANVAS_RESIZE_GUTTER
    );
    for selector in [
        "story-canvas-resize-width",
        "story-canvas-resize-height",
        "story-canvas-resize-corner",
    ] {
        let handle = cx
            .debug_bounds(selector)
            .unwrap_or_else(|| panic!("{selector} should render"));
        assert!(
            handle.left() >= oversized_stage.left()
                && handle.top() >= oversized_stage.top()
                && handle.right() <= oversized_stage.right()
                && handle.bottom() <= oversized_stage.bottom(),
            "resize handle {handle:?} must remain inside stage {oversized_stage:?}"
        );
    }

    let scroll_region =
        cx.update(|window, _| gpui::Bounds::new(point(px(0.), px(0.)), window.viewport_size()));
    cx.simulate_event(ScrollWheelEvent {
        position: scroll_region.center(),
        delta: ScrollDelta::Pixels(point(px(-5000.), px(0.))),
        ..Default::default()
    });
    cx.simulate_event(ScrollWheelEvent {
        position: scroll_region.center(),
        delta: ScrollDelta::Pixels(point(px(0.), px(-5000.))),
        ..Default::default()
    });
    draw(cx);
    let scrolled_canvas = cx
        .debug_bounds("story-canvas")
        .expect("scrolled responsive canvas should render");
    let scrolled_stage = cx
        .debug_bounds("story-canvas-stage")
        .expect("scrolled responsive stage should render");
    assert_eq!(
        scroll_region.right() - scrolled_canvas.right(),
        STORY_CANVAS_RESIZE_GUTTER,
        "scroll region {scroll_region:?}, canvas {scrolled_canvas:?}, stage {scrolled_stage:?}"
    );
    assert_eq!(
        scroll_region.bottom() - scrolled_canvas.bottom(),
        STORY_CANVAS_RESIZE_GUTTER,
        "scroll viewport {scroll_region:?}, canvas {scrolled_canvas:?}, stage {scrolled_stage:?}"
    );
}

#[gpui::test]
fn panel_activation_defers_workbench_story_reads(cx: &mut App) {
    gpui_component::init(cx);
    let state = cx.new(|cx| WorkbenchState::new(None, cx));
    let state_for_window = state.clone();
    let window: gpui::WindowHandle<StoryContainer> = cx
        .open_window(Default::default(), move |window, cx| {
            cx.new(|cx| {
                let mut story = StoryContainer::new(window, cx);
                story.set_workbench_state(state_for_window.downgrade());
                story
            })
        })
        .expect("test window should open");

    window
        .update(cx, |story, window, cx| {
            BasePanel::set_active(story, true, window, cx);
            assert!(state.read(cx).active_story().is_none());
        })
        .expect("panel activation should not reenter the story entity");
}

#[gpui::test]
fn story_group_class_sorts_entities_and_ignores_missing_classes(cx: &mut App) {
    gpui_component::init(cx);
    let window: gpui::WindowHandle<StoryContainer> = cx
        .open_window(Default::default(), |window, cx| {
            cx.new(|cx| StoryContainer::new(window, cx))
        })
        .expect("test window should open");

    window
        .update(cx, |_, window, cx| {
            let table = cx.new(|cx| {
                let mut story = StoryContainer::new(window, cx);
                story.story_klass = Some("TableStory".into());
                story
            });
            let button = cx.new(|cx| {
                let mut story = StoryContainer::new(window, cx);
                story.story_klass = Some("ButtonStory".into());
                story
            });
            let missing = cx.new(|cx| StoryContainer::new(window, cx));

            assert_eq!(
                story_group_klass(&[table, missing, button], cx).as_ref(),
                "__gpui_storybook_group__:ButtonStory|TableStory"
            );
        })
        .expect("story classes should be computed");
}

#[test]
fn story_state_serializes_panel_identity() {
    let state = StoryState {
        story_klass: "ButtonStory".into(),
    };
    assert_eq!(
        state.to_value(),
        serde_json::json!({ "story_klass": "ButtonStory" })
    );
}
