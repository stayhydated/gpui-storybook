use super::*;

impl StoryWorkspace {
    pub(super) fn title_bar_sidebar_controls(dock_area: Entity<DockArea>, cx: &App) -> AnyElement {
        let (left_collapsed, right_collapsed) = {
            let dock_area = dock_area.read(cx);
            (
                !dock_area.is_dock_open(DockPlacement::Left),
                !dock_area.is_dock_open(DockPlacement::Right),
            )
        };
        let dock_area_for_left = dock_area.clone();
        let dock_area_for_right = dock_area;

        h_flex()
            .gap_1()
            .child(
                div()
                    .debug_selector(|| "dock-toggle-left-sidebar".to_owned())
                    .child(
                        sidebar_toggle_button(
                            "dock-toggle-left-sidebar-button",
                            Side::Left,
                            left_collapsed,
                        )
                        .tooltip(if left_collapsed {
                            "Show story navigation"
                        } else {
                            "Hide story navigation"
                        })
                        .on_click(move |_, window, cx| {
                            dock_area_for_left.update(cx, |dock_area, cx| {
                                dock_area.toggle_dock(DockPlacement::Left, window, cx);
                            });
                            window.refresh();
                        }),
                    ),
            )
            .child(
                div()
                    .debug_selector(|| "dock-toggle-right-sidebar".to_owned())
                    .child(
                        sidebar_toggle_button(
                            "dock-toggle-right-sidebar-button",
                            Side::Right,
                            right_collapsed,
                        )
                        .tooltip(if right_collapsed {
                            "Show story workbench"
                        } else {
                            "Hide story workbench"
                        })
                        .on_click(move |_, window, cx| {
                            dock_area_for_right.update(cx, |dock_area, cx| {
                                dock_area.toggle_dock(DockPlacement::Right, window, cx);
                            });
                            window.refresh();
                        }),
                    ),
            )
            .into_any_element()
    }

    pub(super) fn save_layout(
        &mut self,
        dock_area: &Entity<DockArea>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !PERSIST_DOCK_LAYOUT {
            return;
        }
        let state = dock_area.read(cx).dump(cx);

        if Some(&state) == self.last_layout_state.as_ref() {
            return;
        }

        match Self::save_state(&state) {
            Ok(()) => {
                self.last_layout_state = Some(state);
            },
            Err(err) => {
                eprintln!("failed to save dock layout to {STATE_FILE}: {err:#}");
            },
        }
    }

    pub(super) fn save_state(state: &DockAreaState) -> Result<()> {
        DockLayoutStore::save_to_path(STATE_FILE, state)
    }

    pub(super) fn load_layout(
        dock_area: Entity<DockArea>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<()> {
        let state: DockAreaState = DockLayoutStore::load_from_path(STATE_FILE)?;

        // Saved layouts must match the active dock schema.
        if state.version != Some(MAIN_DOCK_AREA.version) {
            anyhow::bail!("Layout version mismatch");
        }

        dock_area.update(cx, |dock_area, cx| {
            dock_area.load(state, window, cx).context("load layout")?;
            for placement in [
                DockPlacement::Left,
                DockPlacement::Bottom,
                DockPlacement::Right,
            ] {
                dock_area.set_dock_collapsible(placement, true, window, cx);
            }
            Ok::<(), anyhow::Error>(())
        })
    }

    pub(super) fn reset_default_layout(
        dock_area: gpui::WeakEntity<DockArea>,
        stories: &[Entity<StoryContainer>],
        automation: Option<SharedStorybookAutomation>,
        window: &mut Window,
        cx: &mut App,
    ) {
        let dock_layout = Self::build_center_layout();

        // Create sidebar panel for the left dock
        let sidebar_panel = Self::build_sidebar(stories, &dock_area, automation, window, cx);
        let workbench_panel = Self::build_workbench(&dock_area, WorkbenchTab::Controls, window, cx);

        _ = dock_area.update(cx, |view, cx| {
            view.set_version(Some(MAIN_DOCK_AREA.version), cx);
            view.set_center(dock_layout, window, cx);
            for (placement, layout, size) in [
                (DockPlacement::Left, sidebar_panel, px(260.)),
                (DockPlacement::Right, workbench_panel, px(320.)),
            ] {
                view.set_dock(placement, layout, window, cx);
                view.set_dock_size(placement, size, window, cx);
                view.set_dock_collapsible(placement, true, window, cx);
                if !view.is_dock_open(placement) {
                    view.toggle_dock(placement, window, cx);
                }
            }
        });
    }

    fn build_sidebar(
        stories: &[Entity<StoryContainer>],
        dock_area: &gpui::WeakEntity<DockArea>,
        automation: Option<SharedStorybookAutomation>,
        window: &mut Window,
        cx: &mut App,
    ) -> DockLayout {
        let sidebar = cx.new(|cx| {
            StorySidebar::new(stories.to_vec(), dock_area.clone(), automation, window, cx)
        });

        DockLayout::tabs().panel_view(panel_handle(sidebar), cx)
    }

    fn build_workbench(
        dock_area: &gpui::WeakEntity<DockArea>,
        selected_tab: WorkbenchTab,
        window: &mut Window,
        cx: &mut App,
    ) -> DockLayout {
        let state = workbench_state(dock_area)
            .expect("dock workbench state must be registered before building its panel");
        let workbench = cx.new(|cx| StoryWorkbench::new(state, selected_tab, window, cx));
        DockLayout::tabs().panel_view(panel_handle(workbench), cx)
    }

    fn build_center_layout() -> DockLayout {
        // Wrap center tabs in a split so the tab group gets a parent split.
        // This enables tab drag/drop and split indicators.
        DockLayout::v_split().child(DockLayout::tabs(), None)
    }

    pub fn view(
        stories: Vec<Entity<StoryContainer>>,
        window: &mut Window,
        cx: &mut App,
    ) -> Entity<Self> {
        Self::view_with_ui(stories, StorybookWindowUi::default(), window, cx)
    }

    pub fn view_with_automation(
        stories: Vec<Entity<StoryContainer>>,
        automation: SharedStorybookAutomation,
        window: &mut Window,
        cx: &mut App,
    ) -> Entity<Self> {
        Self::view_with_ui_and_automation(
            stories,
            StorybookWindowUi::default(),
            automation,
            window,
            cx,
        )
    }

    pub fn view_with_ui(
        stories: Vec<Entity<StoryContainer>>,
        ui: StorybookWindowUi,
        window: &mut Window,
        cx: &mut App,
    ) -> Entity<Self> {
        let automation = default_storybook_automation(cx);
        cx.new(|cx| Self::new(stories, ui, automation, window, cx))
    }

    pub fn view_with_ui_and_automation(
        stories: Vec<Entity<StoryContainer>>,
        ui: StorybookWindowUi,
        automation: SharedStorybookAutomation,
        window: &mut Window,
        cx: &mut App,
    ) -> Entity<Self> {
        cx.new(|cx| Self::new(stories, ui, Some(automation), window, cx))
    }
}
