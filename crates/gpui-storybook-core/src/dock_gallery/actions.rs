use super::*;

impl StoryWorkspace {
    pub(super) fn on_action_toggle_dock_toggle_button(
        &mut self,
        _: &ToggleDockToggleButton,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.toggle_button_visible = !self.toggle_button_visible;

        self.dock_skin
            .set_toggle_button_visible(self.toggle_button_visible, cx);
    }

    pub(super) fn on_action_reset_layout(
        &mut self,
        _: &ResetLayout,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let weak_dock_area = self.dock_area.downgrade();
        let stories = StorySidebar::seeded_stories(&weak_dock_area, window, cx);
        Self::reset_default_layout(
            weak_dock_area,
            &stories,
            self.automation.clone(),
            window,
            cx,
        );
        if PERSIST_DOCK_LAYOUT {
            // Delete saved state file after resetting the live layout.
            let _ = std::fs::remove_file(STATE_FILE);
        }

        cx.notify();
    }
}
