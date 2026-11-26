use super::*;
use gpui_kit::Modifiers;

fn draw(cx: &mut VisualTestContext) {
    cx.run_until_parked();
    cx.update(|window, cx| {
        _ = window.draw(cx);
    });
}

fn open_workbench(
    app: &mut TestAppContext,
    width: f32,
) -> (gpui_kit::WindowHandle<StoryWorkbench>, VisualTestContext) {
    let window = app.open_window(size(px(width), px(600.)), |window, cx| {
        let state = cx.new(|cx| WorkbenchState::new(None, cx));
        StoryWorkbench::new(state, WorkbenchTab::Controls, window, cx)
    });
    let visual_cx = VisualTestContext::from_window(*window, app);
    (window, visual_cx)
}

#[test]
fn workbench_tabs_scroll_horizontally() {
    let mut app = TestAppContext::single();
    app.update(gpui_kit::init);
    let (window, mut visual_cx) = open_workbench(&mut app, 220.);
    let cx = &mut visual_cx;

    draw(cx);
    let workbench = window
        .root(cx)
        .expect("the workbench should be the window root");
    let before = cx
        .debug_bounds("workbench-tab-theme")
        .expect("theme tab should render");
    assert_eq!(
        workbench.read_with(cx, |workbench, _| workbench.tab_scroll_handle.offset().x),
        px(0.),
        "the tab strip should start unscrolled"
    );

    let position = cx
        .debug_bounds("workbench-tab-controls")
        .expect("controls tab should render")
        .center();
    cx.simulate_event(ScrollEvent {
        position,
        delta: ScrollDelta::Pixels(point(px(-60.), px(0.))),
        ..Default::default()
    });
    draw(cx);

    let after = cx
        .debug_bounds("workbench-tab-theme")
        .expect("theme tab should remain rendered");
    assert!(
        after.origin.x < before.origin.x,
        "tabs should scroll horizontally: before={before:?}, after={after:?}"
    );
    assert!(
        workbench.read_with(cx, |workbench, _| workbench.tab_scroll_handle.offset().x) < px(0.),
        "the tracked tab scroll handle should reflect the horizontal scroll"
    );
}

#[test]
fn workbench_tabs_scroll_with_a_vertical_wheel() {
    let mut app = TestAppContext::single();
    app.update(gpui_kit::init);
    let (window, mut visual_cx) = open_workbench(&mut app, 220.);
    let cx = &mut visual_cx;

    draw(cx);
    let workbench = window
        .root(cx)
        .expect("the workbench should be the window root");
    let before = cx
        .debug_bounds("workbench-tab-theme")
        .expect("theme tab should render");

    let position = cx
        .debug_bounds("workbench-tabs-scroll")
        .expect("tab strip should render")
        .center();
    cx.simulate_event(ScrollEvent {
        position,
        delta: ScrollDelta::Pixels(point(px(0.), px(-60.))),
        ..Default::default()
    });
    draw(cx);

    let after = cx
        .debug_bounds("workbench-tab-theme")
        .expect("theme tab should remain rendered");
    assert!(
        after.origin.x < before.origin.x,
        "a vertical wheel should scroll the tab strip horizontally: before={before:?}, after={after:?}"
    );
    assert!(
        workbench.read_with(cx, |workbench, _| workbench.tab_scroll_handle.offset().x) < px(0.),
        "the tracked tab scroll handle should reflect the vertical wheel"
    );
}

#[test]
fn workbench_tab_and_always_visible_scrollbar_remain_clickable() {
    let mut app = TestAppContext::single();
    app.update(|cx| {
        gpui_kit::init(cx);
        gpui_kit::component::Theme::set_scrollbar_mode(
            gpui_kit::component::scroll::ScrollbarMode::Always,
            cx,
        );
    });
    let (window, mut visual_cx) = open_workbench(&mut app, 320.);
    let cx = &mut visual_cx;

    draw(cx);
    let workbench = window
        .root(cx)
        .expect("the workbench should be the window root");
    let strip = cx
        .debug_bounds("workbench-tabs-scroll")
        .expect("tab strip should render");
    let track_height = gpui_kit::component::scroll::Scrollbar::width();
    let tab = cx
        .debug_bounds("workbench-tab-theme")
        .expect("theme tab should render");
    assert!(
        tab.bottom() <= strip.bottom() - track_height,
        "the scrollbar track should not overlap the tab's hit target"
    );
    assert!(
        workbench.read_with(cx, |workbench, _| workbench
            .tab_scroll_handle
            .max_offset()
            .x)
            > px(0.),
        "the bar must actually overflow for this regression test"
    );

    cx.simulate_click(tab.center(), Modifiers::default());
    draw(cx);

    assert_eq!(
        workbench.read_with(cx, |workbench, _| workbench.selected_tab),
        WorkbenchTab::Theme,
        "the visible scrollbar must not swallow a tab click"
    );
}
