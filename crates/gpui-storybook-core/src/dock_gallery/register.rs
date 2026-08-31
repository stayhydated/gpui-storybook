use super::*;

/// Register StoryContainer panel for deserialization
pub fn register_story_panels(cx: &mut App) {
    register_panel(cx, "StoryContainer", |context, window, cx| {
        let PanelInfo::Panel(panel_value) = context.info() else {
            panic!("StoryContainer panel state must be PanelInfo::Panel");
        };

        let story_state = serde_json::from_value::<StoryState>(panel_value.clone())
            .expect("StoryContainer panel state must contain StoryState");
        let dock_area = context.dock_area();

        panel_handle(
            StorySidebar::create_story_by_klass(
                story_state.story_klass.as_ref(),
                &dock_area,
                window,
                cx,
            )
            .expect("StoryContainer panel state must reference a registered story"),
        )
    });

    // Register StorySidebar panel
    register_panel(cx, "StorySidebar", |context, window, cx| {
        let dock_area = context.dock_area();
        let stories = StorySidebar::seeded_stories(&dock_area, window, cx);

        panel_handle(cx.new(|cx| StorySidebar::new(stories, dock_area, None, window, cx)))
    });

    register_panel(cx, "StoryWorkbench", |context, window, cx| {
        let selected_tab = StoryWorkbench::selected_tab_from_panel(context.info());
        let dock_area = context.dock_area();
        let state = workbench_state(&dock_area)
            .expect("StoryWorkbench panel must have a registered window state");
        panel_handle(cx.new(|cx| StoryWorkbench::new(state, selected_tab, window, cx)))
    });
}
