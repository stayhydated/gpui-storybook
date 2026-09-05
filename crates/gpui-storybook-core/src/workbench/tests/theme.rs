use super::*;

#[test]
fn theme_header_stays_fixed_while_color_items_scroll() {
    let mut app = TestAppContext::single();
    app.update(gpui_kit::init);
    let window = app.open_window(size(px(400.), px(600.)), |window, cx| {
        let state = cx.new(|cx| WorkbenchState::new(None, cx));
        StoryWorkbench::new(state, WorkbenchTab::Theme, window, cx)
    });
    let mut visual_cx = VisualTestContext::from_window(*window, &app);
    let cx = &mut visual_cx;
    let draw = |cx: &mut VisualTestContext| {
        cx.run_until_parked();
        cx.update(|window, cx| {
            _ = window.draw(cx);
        });
    };

    draw(cx);
    let header_before = cx
        .debug_bounds("workbench-theme-sticky-header")
        .expect("theme header should render");
    let items = cx
        .debug_bounds("workbench-theme-items")
        .expect("theme items should render");
    let first_item_before = cx
        .debug_bounds("workbench-theme-first-item")
        .expect("theme color items should render");

    cx.simulate_event(ScrollWheelEvent {
        position: items.center(),
        delta: ScrollDelta::Pixels(point(px(0.), px(-120.))),
        ..Default::default()
    });
    draw(cx);

    let header_after = cx
        .debug_bounds("workbench-theme-sticky-header")
        .expect("theme header should remain rendered");
    let first_item_after = cx
        .debug_bounds("workbench-theme-first-item")
        .expect("theme color items should remain rendered");
    assert_eq!(header_after.origin, header_before.origin);
    assert!(
        first_item_after.origin.y < first_item_before.origin.y,
        "theme items should move after scrolling: before={first_item_before:?}, after={first_item_after:?}, viewport={items:?}"
    );
}
