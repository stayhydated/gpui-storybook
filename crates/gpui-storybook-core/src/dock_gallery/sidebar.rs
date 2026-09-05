use super::*;

pub struct StorySidebar {
    focus_handle: FocusHandle,
    search_input: Entity<InputState>,
    stories: Vec<Entity<StoryContainer>>,
    dock_area: gpui_kit::WeakEntity<DockArea>,
    automation: Option<SharedStorybookAutomation>,
    _subscriptions: Vec<Subscription>,
}

impl StorySidebar {
    pub fn new(
        stories: Vec<Entity<StoryContainer>>,
        dock_area: gpui_kit::WeakEntity<DockArea>,
        automation: Option<SharedStorybookAutomation>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let search_input =
            cx.new(|cx_input| InputState::new(window, cx_input).placeholder("Search..."));

        let subscriptions = vec![
            #[allow(clippy::single_match)]
            cx.subscribe(&search_input, |_this, _, event, cx| match event {
                InputEvent::Change => {
                    cx.notify();
                },
                _ => {},
            }),
        ];

        Self {
            focus_handle: cx.focus_handle(),
            search_input,
            stories,
            dock_area,
            automation,
            _subscriptions: subscriptions,
        }
    }

    /// Open a story panel and reveal it if it is already mounted in a tab.
    pub(super) fn open_story(
        dock_area: gpui_kit::WeakEntity<DockArea>,
        story: Entity<StoryContainer>,
        automation: Option<SharedStorybookAutomation>,
        window: &mut Window,
        cx: &mut App,
    ) {
        let Some(dock_area) = dock_area.upgrade() else {
            return;
        };
        let concrete_story = story
            .read(cx)
            .variants
            .first()
            .cloned()
            .unwrap_or_else(|| story.clone());
        let story_key = concrete_story.read(cx).story_key_label().map(str::to_owned);
        if let Some(workbench_state) = workbench_state(&dock_area.downgrade()) {
            workbench_state.update(cx, |state, cx| {
                state.set_active_story(Some(story.clone()), cx);
            });
        }

        if reveal_story_panel(&concrete_story, window, cx) {
            if let Some(automation) = automation
                && let Some(story_key) = story_key
            {
                let _ = automation.confirm_current_story(story_key.as_ref());
            }
            return;
        }

        let panel = panel_handle(concrete_story);
        let state = dock_area.update(cx, |dock_area, cx| {
            // Normalize the persisted tree before extending the live layout.
            // This avoids retaining stale split children across additions.
            let state = dock_area.dump(cx);
            if let Ok(state) = DockLayoutStore::sanitize_state(state) {
                let _ = dock_area.load(state, window, cx);
            }

            dock_area.add_panel_view(panel, DockPlacement::Center, None, window, cx);
            dock_area.dump(cx)
        });

        if PERSIST_DOCK_LAYOUT && let Err(err) = DockLayoutStore::save_to_path(STATE_FILE, &state) {
            eprintln!("failed to save dock layout after open to {STATE_FILE}: {err:#}");
        }

        if let Some(automation) = automation
            && let Some(story_key) = story_key
        {
            let _ = automation.confirm_current_story(story_key.as_ref());
        }
    }

    fn register_story(
        dock_area: &gpui_kit::WeakEntity<DockArea>,
        story: &Entity<StoryContainer>,
        cx: &mut App,
    ) {
        let variants = story.read(cx).variants.clone();
        if let Some(state) = workbench_state(dock_area) {
            story.update(cx, |story, _| {
                story.set_workbench_state(state.downgrade());
            });
        }
        let Some(story_klass) = story.read(cx).story_klass.clone() else {
            return;
        };

        if let Ok(mut registries) = STORY_PANELS.lock() {
            let registry = registries.entry(dock_area.entity_id()).or_default();
            registry.insert(story_klass.to_string(), story.downgrade());
            if let Some(story_key) = story.read(cx).story_key_label() {
                registry.insert(story_key.to_string(), story.downgrade());
            }
        }

        for variant in variants {
            Self::register_story(dock_area, &variant, cx);
        }
    }

    pub(super) fn register_stories(
        dock_area: &gpui_kit::WeakEntity<DockArea>,
        stories: &[Entity<StoryContainer>],
        cx: &mut App,
    ) {
        for story in stories {
            Self::register_story(dock_area, story, cx);
        }
    }

    pub(super) fn register_story_seeds(
        dock_area: &gpui_kit::WeakEntity<DockArea>,
        stories: &[Entity<StoryContainer>],
        cx: &App,
    ) {
        let seeds = stories
            .iter()
            .filter_map(|story| {
                let story_data = story.read(cx);
                let story_klass = story_data.story_klass.as_ref()?.to_string();

                Some(StorySeed {
                    name: story_data.display_title(cx),
                    story_key: story_data.story_key_label().map(str::to_owned),
                    story_klass,
                    group: story_data.group.as_ref().map(ToString::to_string),
                    section: story_data.section.as_ref().map(ToString::to_string),
                })
            })
            .collect();

        if let Ok(mut registries) = STORY_SEEDS.lock() {
            registries.insert(dock_area.entity_id(), seeds);
        }
    }

    fn story_seed_by_key(
        dock_area: &gpui_kit::WeakEntity<DockArea>,
        story_key: &str,
    ) -> Option<StorySeed> {
        let registries = STORY_SEEDS.lock().ok()?;
        registries
            .get(&dock_area.entity_id())?
            .iter()
            .find(|seed| seed.story_key.as_deref() == Some(story_key))
            .cloned()
    }

    fn story_seed(
        dock_area: &gpui_kit::WeakEntity<DockArea>,
        story_klass: &str,
    ) -> Option<StorySeed> {
        let registries = STORY_SEEDS.lock().ok()?;
        registries
            .get(&dock_area.entity_id())?
            .iter()
            .find(|seed| seed.story_klass == story_klass)
            .cloned()
    }

    pub(super) fn create_story_by_klass(
        story_klass: &str,
        dock_area: &gpui_kit::WeakEntity<DockArea>,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<Entity<StoryContainer>> {
        if let Some(story) = Self::find_story_by_klass(dock_area, story_klass) {
            return Some(story);
        }

        let story_seed = Self::story_seed(dock_area, story_klass);
        if let Some(member_klasses) = parse_story_group_klass(story_klass) {
            let story_seed = story_seed?;
            let member_stories = member_klasses
                .iter()
                .filter_map(|member_klass| {
                    Self::create_story_panel_by_klass(
                        member_klass,
                        story_seed.group.as_deref(),
                        story_seed.section.as_deref(),
                        dock_area,
                        window,
                        cx,
                    )
                })
                .collect::<Vec<_>>();

            if member_stories.is_empty() {
                return None;
            }

            let panel = StoryContainer::variant_group(story_seed.name, member_stories, window, cx);
            panel.update(cx, |c, _| {
                c.group = story_seed.group.clone().map(Into::into);
                c.section = story_seed.section.clone().map(Into::into);
            });
            Self::register_story(dock_area, &panel, cx);
            return Some(panel);
        }

        Self::create_story_panel_by_klass(
            story_klass,
            story_seed.as_ref().and_then(|seed| seed.group.as_deref()),
            story_seed.as_ref().and_then(|seed| seed.section.as_deref()),
            dock_area,
            window,
            cx,
        )
    }

    fn create_story_panel_by_klass(
        story_klass: &str,
        group: Option<&str>,
        section: Option<&str>,
        dock_area: &gpui_kit::WeakEntity<DockArea>,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<Entity<StoryContainer>> {
        if let Some(story) = Self::find_story_by_klass(dock_area, story_klass) {
            return Some(story);
        }

        let entry =
            inventory::iter::<StoryEntry>().find(|entry| entry.name.as_str() == story_klass)?;
        let panel = (entry.create_fn)(window, cx);
        panel.update(cx, |c, _| {
            c.group = group.map(Into::into);
            c.section = section.map(Into::into);
            c.set_registration_metadata(entry.metadata());
        });
        Self::register_story(dock_area, &panel, cx);
        Some(panel)
    }

    fn create_story_by_key(
        story_key: &str,
        dock_area: &gpui_kit::WeakEntity<DockArea>,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<Entity<StoryContainer>> {
        if let Some(story) = Self::find_story_by_klass(dock_area, story_key) {
            return Some(story);
        }

        let story_seed = Self::story_seed_by_key(dock_area, story_key);
        Self::create_story_panel_by_key(
            story_key,
            story_seed.as_ref().and_then(|seed| seed.group.as_deref()),
            story_seed.as_ref().and_then(|seed| seed.section.as_deref()),
            dock_area,
            window,
            cx,
        )
    }

    fn create_story_panel_by_key(
        story_key: &str,
        group: Option<&str>,
        section: Option<&str>,
        dock_area: &gpui_kit::WeakEntity<DockArea>,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<Entity<StoryContainer>> {
        if let Some(story) = Self::find_story_by_klass(dock_area, story_key) {
            return Some(story);
        }

        let entry =
            inventory::iter::<StoryEntry>().find(|entry| entry.key().as_str() == story_key)?;
        let panel = (entry.create_fn)(window, cx);
        panel.update(cx, |c, _| {
            c.group = group.map(Into::into);
            c.section = section.map(Into::into);
            c.set_registration_metadata(entry.metadata());
        });
        Self::register_story(dock_area, &panel, cx);
        Some(panel)
    }

    pub(super) fn seeded_stories(
        dock_area: &gpui_kit::WeakEntity<DockArea>,
        window: &mut Window,
        cx: &mut App,
    ) -> Vec<Entity<StoryContainer>> {
        let seeds = STORY_SEEDS
            .lock()
            .ok()
            .and_then(|registries| registries.get(&dock_area.entity_id()).cloned())
            .unwrap_or_default();

        seeds
            .into_iter()
            .filter_map(|seed| {
                Self::create_story_by_klass(&seed.story_klass, dock_area, window, cx)
            })
            .collect()
    }

    fn find_story_by_klass(
        dock_area: &gpui_kit::WeakEntity<DockArea>,
        story_klass: &str,
    ) -> Option<Entity<StoryContainer>> {
        let mut registries = STORY_PANELS.lock().ok()?;
        let dock_area_id = dock_area.entity_id();
        let registry = registries.get_mut(&dock_area_id)?;

        if let Some(story) = registry.get(story_klass).and_then(|story| story.upgrade()) {
            return Some(story);
        }

        registry.remove(story_klass);
        if registry.is_empty() {
            registries.remove(&dock_area_id);
        }

        None
    }

    pub(super) fn open_story_by_klass(
        dock_area: gpui_kit::WeakEntity<DockArea>,
        story_klass: &str,
        window: &mut Window,
        cx: &mut App,
    ) {
        if let Some(story) = Self::create_story_by_klass(story_klass, &dock_area, window, cx) {
            Self::open_story(dock_area, story, None, window, cx);
        }
    }

    pub(super) fn open_story_by_key(
        dock_area: gpui_kit::WeakEntity<DockArea>,
        story_key: &str,
        automation: Option<SharedStorybookAutomation>,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<Entity<StoryContainer>> {
        let story = Self::create_story_by_key(story_key, &dock_area, window, cx)?;
        Self::open_story(dock_area, story.clone(), automation, window, cx);
        Some(story)
    }
}

impl BasePanel for StorySidebar {
    fn panel_name(&self) -> &'static str {
        "StorySidebar"
    }

    fn closable(&self, _cx: &App) -> bool {
        false
    }

    fn zoomable(&self, _cx: &App) -> bool {
        false
    }
}

impl Panel for StorySidebar {
    fn title(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        "Stories"
    }

    fn zoom_control(&self, _cx: &App) -> Option<PanelControl> {
        None
    }
}

impl EventEmitter<PanelEvent> for StorySidebar {}

impl Focusable for StorySidebar {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for StorySidebar {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let query = self.search_input.read(cx).value();
        let story_metadata = self
            .stories
            .iter()
            .map(|story| {
                let story_data = story.read(cx);
                SidebarStoryMetadata {
                    title: story_data.display_title(cx),
                    group: story_data.sidebar_group().map(|group| group.to_string()),
                    section: story_data
                        .sidebar_section()
                        .map(|section| section.to_string()),
                }
            })
            .collect::<Vec<_>>();
        let show_group_labels = story_metadata
            .iter()
            .map(|story| story.group.as_deref())
            .collect::<BTreeSet<_>>()
            .len()
            > 1;
        let groups = group_matching_stories(&story_metadata, query.as_ref());

        Sidebar::new("story-sidebar")
            .side(gpui_kit::component::Side::Left)
            .w(relative(1.))
            .border_0()
            .header(
                v_flex().w_full().child(
                    div()
                        .bg(cx.theme().sidebar_border)
                        .px_1()
                        .rounded_full()
                        .flex_1()
                        .mx_1()
                        .gap_4()
                        .child(
                            Input::new(&self.search_input)
                                .appearance(false)
                                .cleanable(true),
                        ),
                ),
            )
            .children(
                groups
                    .into_iter()
                    .map(|(group, sections_in_group)| {
                        let menu_items: Vec<_> = sections_in_group
                            .into_iter()
                            .flat_map(|(section, stories_in_section)| {
                                let mut items = Vec::new();
                                let has_section = section.is_some();

                                if let Some(section) = section {
                                    items.push(
                                        StorySidebarItem::new(section, "")
                                            .disable(true)
                                            .section_heading(true),
                                    );
                                }

                                items.extend(stories_in_section.into_iter().map(|story_index| {
                                    let story_entity = &self.stories[story_index];
                                    let story_data = story_entity.read(cx);
                                    let name: SharedString = story_data.display_title(cx).into();
                                    let story_klass_for_drag =
                                        story_data.story_klass.clone().unwrap_or_default();

                                    let story_for_click = story_entity.clone();
                                    let dock_area_for_click = self.dock_area.clone();
                                    let automation_for_click = self.automation.clone();
                                    StorySidebarItem::new(name, story_klass_for_drag)
                                        .indented(has_section)
                                        .on_click(cx.listener(
                                            move |_, _: &ClickEvent, window, cx| {
                                                let dock_area_for_open =
                                                    dock_area_for_click.clone();
                                                let story_for_open = story_for_click.clone();
                                                let automation_for_open =
                                                    automation_for_click.clone();
                                                window.defer(cx, move |window, cx| {
                                                    Self::open_story(
                                                        dock_area_for_open,
                                                        story_for_open,
                                                        automation_for_open,
                                                        window,
                                                        cx,
                                                    );
                                                });
                                            },
                                        ))
                                }));

                                items
                            })
                            .collect();

                        SidebarGroup::new(group.filter(|_| show_group_labels).unwrap_or_default())
                            .children(menu_items)
                    })
                    .collect::<Vec<_>>(),
            )
    }
}
