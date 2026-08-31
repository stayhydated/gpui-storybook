use super::*;

impl StoryWorkspace {
    pub fn new(
        stories: Vec<Entity<StoryContainer>>,
        ui: StorybookWindowUi,
        automation: Option<SharedStorybookAutomation>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        Self::new_with_host(
            stories,
            "Storybook".into(),
            ui,
            automation,
            true,
            window,
            cx,
        )
    }

    pub(crate) fn new_without_automation_host(
        stories: Vec<Entity<StoryContainer>>,
        title: SharedString,
        ui: StorybookWindowUi,
        automation: Option<SharedStorybookAutomation>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        Self::new_with_host(stories, title, ui, automation, false, window, cx)
    }

    fn new_with_host(
        stories: Vec<Entity<StoryContainer>>,
        title: SharedString,
        ui: StorybookWindowUi,
        automation: Option<SharedStorybookAutomation>,
        attach_automation_host: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let command_receiver = if attach_automation_host {
            automation
                .as_ref()
                .and_then(|automation| automation.take_command_receiver())
        } else {
            None
        };
        let automation = if attach_automation_host && command_receiver.is_none() {
            None
        } else {
            automation
        };
        if let Some(automation) = &automation {
            automation.set_stories(story_snapshots_from_containers(&stories, cx));
        }

        let (dock_area, dock_skin) =
            DockSkin::dock_area(MAIN_DOCK_AREA.id, Some(MAIN_DOCK_AREA.version), window, cx);
        let weak_dock_area = dock_area.downgrade();
        let workbench_state =
            cx.new(|cx| WorkbenchState::new_with_automation(None, automation.clone(), cx));
        workbench_state.update(cx, |state, cx| {
            state.set_active_story(stories.first().cloned(), cx);
        });
        register_workbench_state(&weak_dock_area, &workbench_state);
        StorySidebar::register_story_seeds(&weak_dock_area, &stories, cx);
        StorySidebar::register_stories(&weak_dock_area, &stories, cx);

        // Load saved layout when available; otherwise build the default layout.
        if PERSIST_DOCK_LAYOUT {
            match Self::load_layout(dock_area.clone(), window, cx) {
                Ok(_) => {
                    // The saved layout is mounted.
                },
                Err(_) => {
                    Self::reset_default_layout(
                        weak_dock_area,
                        &stories,
                        automation.clone(),
                        window,
                        cx,
                    );
                },
            };
        } else {
            Self::reset_default_layout(weak_dock_area, &stories, automation.clone(), window, cx);
        }
        dock_skin.set_toggle_button_visible(false, cx);

        cx.subscribe_in(
            &dock_area,
            window,
            |this, dock_area, ev: &DockEvent, window, cx| {
                if matches!(ev, DockEvent::LayoutChanged) {
                    this.save_layout(dock_area, window, cx);
                }
            },
        )
        .detach();

        let app_quit_subscription = cx.on_app_quit({
            let dock_area = dock_area.clone();
            move |_, cx| {
                let state = PERSIST_DOCK_LAYOUT.then(|| dock_area.read(cx).dump(cx));
                async move {
                    if let Some(state) = state
                        && let Err(err) = Self::save_state(&state)
                    {
                        eprintln!("failed to save dock layout on quit to {STATE_FILE}: {err:#}");
                    }
                }
            }
        });

        let dock_area_for_title_bar = dock_area.clone();
        let title_bar = cx.new(|cx| {
            AppTitleBar::new(title, ui, window, cx)
                .system_child(|_, _| {
                    Button::new("reset-storybook-layout")
                        .label("Reset layout")
                        .xsmall()
                        .ghost()
                        .on_click(|_, window, cx| {
                            window.dispatch_action(Box::new(ResetLayout), cx);
                        })
                })
                .sidebar_child(move |_, cx| {
                    Self::title_bar_sidebar_controls(dock_area_for_title_bar.clone(), cx)
                })
        });

        let mut preference_subscriptions = vec![
            cx.observe_window_appearance(window, |_, window, cx| {
                crate::preferences::window_appearance_changed(window, cx);
            }),
            cx.observe_window_activation(window, |_, window, cx| {
                crate::preferences::window_activated(window, cx);
            }),
        ];
        preference_subscriptions.push(cx.subscribe_in(
            &workbench_state,
            window,
            |this, _, event: &WorkbenchEvent, window, cx| match event {
                WorkbenchEvent::OpenVariant(story) => StorySidebar::open_story(
                    this.dock_area.downgrade(),
                    story.clone(),
                    this.automation.clone(),
                    window,
                    cx,
                ),
            },
        ));
        if let Some(automation) = automation.clone() {
            preference_subscriptions.push(cx.observe(&workbench_state, move |_, state, cx| {
                let Some(story) = state.read(cx).active_story() else {
                    return;
                };
                let Some(key) = story.read(cx).story_key_label().map(str::to_owned) else {
                    return;
                };
                let _ = automation.confirm_current_story(&key);
            }));
        }
        crate::preferences::window_appearance_changed(window, cx);

        let this = Self {
            dock_area,
            dock_skin,
            workbench_state,
            title_bar,
            automation,
            last_layout_state: None,
            toggle_button_visible: false,
            _app_quit_subscription: app_quit_subscription,
            _preference_subscriptions: preference_subscriptions,
        };
        if let Some(command_receiver) = command_receiver {
            this.attach_automation_host(command_receiver, window, cx);
        }

        this
    }
}
