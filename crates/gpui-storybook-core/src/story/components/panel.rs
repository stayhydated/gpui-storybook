use super::*;

impl BasePanel for StoryContainer {
    fn panel_name(&self) -> &'static str {
        "StoryContainer"
    }

    fn closable(&self, _cx: &App) -> bool {
        self.closable
    }

    fn zoomable(&self, _cx: &App) -> bool {
        self.zoomable.is_some()
    }

    fn visible(&self, cx: &App) -> bool {
        !AppState::global(cx)
            .invisible_panels
            .read(cx)
            .contains(&self.name)
    }

    fn set_zoomed(&mut self, zoomed: bool, _window: &mut Window, _cx: &mut gpui::Context<Self>) {
        tracing::debug!(panel = %self.name, zoomed, "Storybook panel zoom changed");
    }

    fn set_active(&mut self, active: bool, window: &mut Window, cx: &mut gpui::Context<Self>) {
        tracing::debug!(panel = %self.name, active, "Storybook panel activation changed");
        self.is_active = active;
        if active
            && let Some(state) = self
                .workbench_state
                .as_ref()
                .and_then(gpui::WeakEntity::upgrade)
        {
            let story = cx.entity();
            // Panel activation updates this entity while the dock group is
            // synchronizing its selection. Defer the workbench update so it
            // can inspect the story after the current entity lease is released.
            window.defer(cx, move |_, cx| {
                state.update(cx, |state, cx| state.set_active_story(Some(story), cx));
            });
        }
        if let Some(on_active) = self.on_active
            && let Some(story) = self.story.clone()
        {
            on_active(story, active, window, cx);
        }
    }

    fn on_added_to(
        &mut self,
        tab_group: gpui::WeakEntity<TabGroup>,
        _window: &mut Window,
        _cx: &mut gpui::Context<Self>,
    ) {
        self.tab_group = Some(tab_group);
    }

    fn on_removed(&mut self, _window: &mut Window, _cx: &mut gpui::Context<Self>) {
        self.tab_group = None;
        self.is_active = false;
    }

    fn dump(&self, _cx: &App) -> PanelState {
        let mut state = PanelState::new(self.panel_name());
        if let Some(story_klass) = self.story_klass.clone() {
            let story_state = StoryState { story_klass };
            state.info = PanelInfo::panel(story_state.to_value());
        }
        state
    }
}

impl Panel for StoryContainer {
    fn title(&mut self, _window: &mut Window, _cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let tab_group = self.tab_group.clone();
        let story_panel = _cx.entity().downgrade();
        let title = if self.variant_group.is_some() {
            self.variant_label(_cx)
        } else {
            self.display_title(_cx)
        }
        .into_any_element();

        h_flex()
            .items_center()
            .gap_1()
            .child(title)
            .when(self.closable && self.is_active, |this| {
                this.child(
                    Button::new(format!(
                        "close-story-tab-{}",
                        self.story_klass.clone().unwrap_or_default()
                    ))
                    .icon(IconName::Close)
                    .xsmall()
                    .ghost()
                    .tab_stop(false)
                    .on_click(
                        move |_: &ClickEvent, _window: &mut Window, cx: &mut App| {
                            cx.stop_propagation();
                            let Some(tab_group) = tab_group.clone().and_then(|tab| tab.upgrade())
                            else {
                                return;
                            };
                            let Some(story_panel) = story_panel.upgrade() else {
                                return;
                            };
                            tab_group.update(cx, |tab_group, cx| {
                                tab_group.close_panel(PanelId::from(story_panel.entity_id()), cx);
                            });
                        },
                    ),
                )
            })
    }

    fn title_style(&self, cx: &App) -> Option<TitleStyle> {
        self.title_bg.map(|bg| TitleStyle {
            background: bg,
            foreground: cx.theme().foreground,
        })
    }

    fn zoom_control(&self, _cx: &App) -> Option<PanelControl> {
        self.zoomable
    }

    fn dropdown_menu(
        &mut self,
        menu: PopupMenu,
        _window: &mut Window,
        _cx: &mut gpui::Context<Self>,
    ) -> PopupMenu {
        menu.menu("Info", Box::new(ShowPanelInfo))
    }
}

pub fn reveal_story_panel(
    story: &Entity<StoryContainer>,
    window: &mut Window,
    cx: &mut App,
) -> bool {
    let (is_active, tab_group) = {
        let story = story.read(cx);
        (story.is_active, story.tab_group.clone())
    };

    if is_active {
        return true;
    }

    let Some(tab_group) = tab_group.and_then(|tab| tab.upgrade()) else {
        return false;
    };

    let panel = panel_handle(story.clone());
    tab_group.update(cx, |tab_group, cx| {
        let Some(ix) = tab_group
            .panels()
            .iter()
            .position(|candidate| candidate.panel_id(cx) == panel.panel_id(cx))
        else {
            return;
        };
        tab_group.select_tab(ix, window, cx);
    });

    true
}

impl EventEmitter<PanelEvent> for StoryContainer {}
impl Focusable for StoryContainer {
    fn focus_handle(&self, _: &App) -> gpui::FocusHandle {
        self.focus_handle.clone()
    }
}
