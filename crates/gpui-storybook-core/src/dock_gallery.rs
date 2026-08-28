use crate::{
    automation::{
        SharedStorybookAutomation, StoryCurrentSnapshot, StoryScreenshotRequest, StorySnapshot,
        StorybookAutomationCommand, StorybookAutomationError, default_storybook_automation,
        schedule_story_capture, set_capture_target_size, story_snapshots_from_containers,
        validate_capture_target_size,
    },
    capture_region::capture_route_story_key,
    dock_layout_store::DockLayoutStore,
    dock_sidebar_index::{SidebarStoryMetadata, group_matching_stories},
    registry::StoryEntry,
    story::{StoryContainer, StoryState, parse_story_list_klass, reveal_story_panel},
    storybook_window_ui::StorybookWindowUi,
    title_bar::{AppTitleBar, sidebar_toggle_button},
    window_options::default_storybook_window_options,
    window_view::DockWindowView,
    workbench::{StoryWorkbench, WorkbenchState, WorkbenchTab},
};
use anyhow::{Context as _, Result};
use gpui::{
    Action, AnyElement, App, AppContext as _, ClickEvent, Context, Entity, EntityId, EventEmitter,
    FocusHandle, Focusable, InteractiveElement as _, IntoElement, ParentElement as _, Render,
    SharedString, Styled as _, Subscription, Window, div, px, relative,
};
use gpui_component::{
    ActiveTheme as _, Root, Side, Sizable as _,
    button::{Button, ButtonVariants as _},
    dock::{
        BasePanel, ClosePanel, DockArea, DockAreaState, DockEvent, DockLayout, DockPlacement,
        DockSkin, Panel, PanelControl, PanelEvent, PanelInfo, ToggleZoom, panel_handle,
        register_panel,
    },
    h_flex,
    input::{Input, InputEvent, InputState},
    sidebar::{Sidebar, SidebarGroup},
    v_flex,
};
use gpui_storybook_components::{StoryDrag, StorySidebarItem};
use std::{
    collections::{BTreeMap, BTreeSet},
    rc::Rc,
    sync::{LazyLock, Mutex},
};

#[derive(Action, Clone, Debug, Default, Eq, PartialEq)]
#[action(namespace = story)]
pub struct ToggleDockToggleButton;

#[derive(Action, Clone, Debug, Default, Eq, PartialEq)]
#[action(namespace = story)]
pub struct ResetLayout;

#[derive(Action, Clone, Debug, Default, Eq, PartialEq)]
#[action(namespace = story)]
pub struct ToggleSidebar;

const MAIN_DOCK_AREA: DockAreaTab = DockAreaTab {
    id: "storybook-main-dock",
    version: 6,
};

#[cfg(debug_assertions)]
const STATE_FILE: &str = "target/storybook-docks.json";
#[cfg(not(debug_assertions))]
const STATE_FILE: &str = "storybook-docks.json";

struct DockAreaTab {
    id: &'static str,
    version: usize,
}

type StoryPanelMap = BTreeMap<String, gpui::WeakEntity<StoryContainer>>;
type StoryPanelRegistries = BTreeMap<EntityId, StoryPanelMap>;
type StorySeedRegistries = BTreeMap<EntityId, Vec<StorySeed>>;
type WorkbenchStateRegistries = BTreeMap<EntityId, gpui::WeakEntity<WorkbenchState>>;

static STORY_PANELS: LazyLock<Mutex<StoryPanelRegistries>> =
    LazyLock::new(|| Mutex::new(BTreeMap::new()));
static STORY_SEEDS: LazyLock<Mutex<StorySeedRegistries>> =
    LazyLock::new(|| Mutex::new(BTreeMap::new()));
static WORKBENCH_STATES: LazyLock<Mutex<WorkbenchStateRegistries>> =
    LazyLock::new(|| Mutex::new(BTreeMap::new()));

fn register_workbench_state(
    dock_area: &gpui::WeakEntity<DockArea>,
    state: &Entity<WorkbenchState>,
) {
    if let Ok(mut states) = WORKBENCH_STATES.lock() {
        states.insert(dock_area.entity_id(), state.downgrade());
    }
}

fn workbench_state(dock_area: &gpui::WeakEntity<DockArea>) -> Option<Entity<WorkbenchState>> {
    let mut states = WORKBENCH_STATES.lock().ok()?;
    let state = states
        .get(&dock_area.entity_id())
        .and_then(gpui::WeakEntity::upgrade);
    if state.is_none() {
        states.remove(&dock_area.entity_id());
    }
    state
}

#[derive(Clone, Debug)]
struct StorySeed {
    name: String,
    story_key: Option<String>,
    story_klass: String,
    group: Option<String>,
    section: Option<String>,
}

/// Sidebar panel for navigating stories
pub struct StorySidebar {
    focus_handle: FocusHandle,
    search_input: Entity<InputState>,
    stories: Vec<Entity<StoryContainer>>,
    dock_area: gpui::WeakEntity<DockArea>,
    automation: Option<SharedStorybookAutomation>,
    _subscriptions: Vec<Subscription>,
}

impl StorySidebar {
    pub fn new(
        stories: Vec<Entity<StoryContainer>>,
        dock_area: gpui::WeakEntity<DockArea>,
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
    fn open_story(
        dock_area: gpui::WeakEntity<DockArea>,
        story: Entity<StoryContainer>,
        automation: Option<SharedStorybookAutomation>,
        window: &mut Window,
        cx: &mut App,
    ) {
        let Some(dock_area) = dock_area.upgrade() else {
            return;
        };
        let story_key = story.read(cx).story_key_label().map(str::to_owned);
        if let Some(workbench_state) = workbench_state(&dock_area.downgrade()) {
            workbench_state.update(cx, |state, cx| {
                state.set_active_story(Some(story.clone()), cx);
            });
        }

        if reveal_story_panel(&story, window, cx) {
            if let Some(automation) = automation
                && let Some(story_key) = story_key
            {
                let _ = automation.confirm_current_story(story_key.as_ref());
            }
            return;
        }

        let panel = panel_handle(story.clone());
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

        if let Err(err) = DockLayoutStore::save_to_path(STATE_FILE, &state) {
            eprintln!("failed to save dock layout after open to {STATE_FILE}: {err:#}");
        }

        if let Some(automation) = automation
            && let Some(story_key) = story_key
        {
            let _ = automation.confirm_current_story(story_key.as_ref());
        }
    }

    fn register_story(
        dock_area: &gpui::WeakEntity<DockArea>,
        story: &Entity<StoryContainer>,
        cx: &mut App,
    ) {
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
    }

    fn register_stories(
        dock_area: &gpui::WeakEntity<DockArea>,
        stories: &[Entity<StoryContainer>],
        cx: &mut App,
    ) {
        for story in stories {
            Self::register_story(dock_area, story, cx);
        }
    }

    fn register_story_seeds(
        dock_area: &gpui::WeakEntity<DockArea>,
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
        dock_area: &gpui::WeakEntity<DockArea>,
        story_key: &str,
    ) -> Option<StorySeed> {
        let registries = STORY_SEEDS.lock().ok()?;
        registries
            .get(&dock_area.entity_id())?
            .iter()
            .find(|seed| seed.story_key.as_deref() == Some(story_key))
            .cloned()
    }

    fn story_seed(dock_area: &gpui::WeakEntity<DockArea>, story_klass: &str) -> Option<StorySeed> {
        let registries = STORY_SEEDS.lock().ok()?;
        registries
            .get(&dock_area.entity_id())?
            .iter()
            .find(|seed| seed.story_klass == story_klass)
            .cloned()
    }

    fn create_story_by_klass(
        story_klass: &str,
        dock_area: &gpui::WeakEntity<DockArea>,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<Entity<StoryContainer>> {
        if let Some(story) = Self::find_story_by_klass(dock_area, story_klass) {
            return Some(story);
        }

        let story_seed = Self::story_seed(dock_area, story_klass);
        if let Some(member_klasses) = parse_story_list_klass(story_klass) {
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

            let panel = StoryContainer::list_panel(story_seed.name, member_stories, window, cx);
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
        dock_area: &gpui::WeakEntity<DockArea>,
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
        dock_area: &gpui::WeakEntity<DockArea>,
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
        dock_area: &gpui::WeakEntity<DockArea>,
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

    fn seeded_stories(
        dock_area: &gpui::WeakEntity<DockArea>,
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
        dock_area: &gpui::WeakEntity<DockArea>,
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

    fn open_story_by_klass(
        dock_area: gpui::WeakEntity<DockArea>,
        story_klass: &str,
        window: &mut Window,
        cx: &mut App,
    ) {
        if let Some(story) = Self::create_story_by_klass(story_klass, &dock_area, window, cx) {
            Self::open_story(dock_area, story, None, window, cx);
        }
    }

    fn open_story_by_key(
        dock_area: gpui::WeakEntity<DockArea>,
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
            .side(gpui_component::Side::Left)
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

/// Dock workspace whose collapsible side docks leave the story canvas centered
/// within the available center pane.
pub struct StoryWorkspace {
    title_bar: Entity<AppTitleBar>,
    dock_area: Entity<DockArea>,
    dock_skin: Rc<DockSkin>,
    workbench_state: Entity<WorkbenchState>,
    automation: Option<SharedStorybookAutomation>,
    last_layout_state: Option<DockAreaState>,
    toggle_button_visible: bool,
    _preference_subscriptions: Vec<Subscription>,
}

impl StoryWorkspace {
    pub fn new(
        stories: Vec<Entity<StoryContainer>>,
        ui: StorybookWindowUi,
        automation: Option<SharedStorybookAutomation>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        if let Some(automation) = &automation {
            automation.set_stories(story_snapshots_from_containers(&stories, cx));
        }

        let (dock_area, dock_skin) =
            DockSkin::dock_area(MAIN_DOCK_AREA.id, Some(MAIN_DOCK_AREA.version), window, cx);
        let weak_dock_area = dock_area.downgrade();
        let workbench_state = cx.new(|_| WorkbenchState::new(None));
        workbench_state.update(cx, |state, cx| {
            state.set_active_story(stories.first().cloned(), cx);
        });
        register_workbench_state(&weak_dock_area, &workbench_state);
        StorySidebar::register_story_seeds(&weak_dock_area, &stories, cx);
        StorySidebar::register_stories(&weak_dock_area, &stories, cx);

        // Load saved layout when available; otherwise build the default layout.
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

        cx.on_app_quit({
            let dock_area = dock_area.clone();
            move |_, cx| {
                let state = dock_area.read(cx).dump(cx);
                async move {
                    if let Err(err) = Self::save_state(&state) {
                        eprintln!("failed to save dock layout on quit to {STATE_FILE}: {err:#}");
                    }
                }
            }
        })
        .detach();

        let dock_area_for_title_bar = dock_area.clone();
        let title_bar = cx.new(|cx| {
            AppTitleBar::new("Storybook", ui, window, cx)
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
            _preference_subscriptions: preference_subscriptions,
        };
        if let Some(automation) = this.automation.clone() {
            this.attach_automation_host(automation, window, cx);
        }

        this
    }

    fn title_bar_sidebar_controls(dock_area: Entity<DockArea>, cx: &App) -> AnyElement {
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

    fn save_layout(
        &mut self,
        dock_area: &Entity<DockArea>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
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

    fn save_state(state: &DockAreaState) -> Result<()> {
        DockLayoutStore::save_to_path(STATE_FILE, state)
    }

    fn load_layout(
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

    fn reset_default_layout(
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

    fn attach_automation_host(
        &self,
        automation: SharedStorybookAutomation,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(mut receiver) = automation.take_command_receiver() else {
            return;
        };

        cx.spawn_in(window, async move |this, cx| {
            while let Some(command) = receiver.recv().await {
                let _ = this.update_in(cx, |workspace, window, cx| {
                    workspace.handle_automation_command(command, window, cx);
                });
            }
        })
        .detach();
    }

    fn handle_automation_command(
        &mut self,
        command: StorybookAutomationCommand,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match command {
            StorybookAutomationCommand::OpenStory { key, response, .. } => {
                let result = self.open_story_by_key(&key, window, cx);
                let _ = response.send(result);
            },
            StorybookAutomationCommand::CaptureCurrentStory {
                request_id,
                request,
                response,
                operation,
            } => {
                let quit_after_capture = request.quit_after_capture;
                match self.prepare_capture_current_story(&request, window, cx) {
                    Ok(story) => {
                        schedule_story_capture(
                            request_id,
                            request,
                            story,
                            response,
                            operation,
                            quit_after_capture,
                            window,
                        );
                    },
                    Err(error) => {
                        eprintln!("gpui-storybook capture session failed: {error}");
                        let _ = response.send(Err(error));
                        if quit_after_capture {
                            std::process::exit(1);
                        }
                    },
                }
            },
            StorybookAutomationCommand::ReadControls { response } => {
                let result = self.workbench_state.read(cx).controls_snapshot(cx);
                let _ = response.send(result);
            },
            StorybookAutomationCommand::SetControl {
                key,
                value,
                response,
                ..
            } => {
                let result = self
                    .workbench_state
                    .update(cx, |state, cx| state.set_control(&key, value, cx));
                cx.notify();
                let _ = response.send(result);
            },
            StorybookAutomationCommand::ResetControl { key, response, .. } => {
                let result = self
                    .workbench_state
                    .update(cx, |state, cx| state.reset_control(key.as_deref(), cx));
                cx.notify();
                let _ = response.send(result);
            },
            StorybookAutomationCommand::ListActions { response } => {
                let _ = response.send(Ok(crate::automation::interaction::list_registered_actions(
                    cx,
                )));
            },
            StorybookAutomationCommand::ListInteractionTargets { response } => {
                let result = self
                    .automation
                    .as_ref()
                    .and_then(|automation| automation.current_story().story)
                    .ok_or(StorybookAutomationError::NoActiveStory);
                match result {
                    Ok(story) => {
                        crate::automation::interaction::schedule_interaction_target_listing(
                            story, response, window,
                        );
                    },
                    Err(error) => {
                        let _ = response.send(Err(error));
                    },
                }
            },
            StorybookAutomationCommand::ReadSemanticValues { response } => {
                let result = self
                    .automation
                    .as_ref()
                    .and_then(|automation| automation.current_story().story)
                    .ok_or(StorybookAutomationError::NoActiveStory);
                match result {
                    Ok(story) => {
                        crate::automation::schedule_semantic_value_read(story, response, window)
                    },
                    Err(error) => {
                        let _ = response.send(Err(error));
                    },
                }
            },
            StorybookAutomationCommand::RunSteps {
                request_id,
                request,
                fresh_story,
                response,
                progress,
                operation,
            } => {
                if response.is_closed() {
                    return;
                }
                let prepared = (|| {
                    crate::automation::interaction::validate_interaction_request(&request)?;
                    let steps = crate::automation::interaction::prepare_interaction_steps(
                        &request.steps,
                        cx,
                    )?;
                    if let Some(route) = &request.story_key {
                        self.open_story_by_key(route, window, cx)?;
                    }
                    if fresh_story {
                        let story_entity = self
                            .workbench_state
                            .read(cx)
                            .active_story()
                            .ok_or(StorybookAutomationError::NoActiveStory)?;
                        story_entity.update(cx, |story, cx| {
                            story.recreate_for_scenario(window, cx);
                        });
                    }
                    if let Some(presentation) = request.presentation {
                        self.workbench_state.update(cx, |state, cx| {
                            state.set_viewport(presentation.viewport, cx);
                            state.set_background(presentation.background, cx);
                        });
                    }
                    self.workbench_state
                        .update(cx, |state, cx| state.apply_controls(&request.controls, cx))?;
                    let story = self
                        .automation
                        .as_ref()
                        .and_then(|automation| automation.current_story().story)
                        .ok_or(StorybookAutomationError::NoActiveStory)?;
                    let target_size =
                        crate::automation::interaction::interaction_target_size(&request)?;
                    let story_entity = self
                        .workbench_state
                        .read(cx)
                        .active_story()
                        .ok_or(StorybookAutomationError::NoActiveStory)?;
                    set_capture_target_size(&story_entity, window, target_size, cx);
                    if request.story_key.is_some() {
                        gpui::Focusable::focus_handle(&story_entity, cx).focus(window, cx);
                    }
                    cx.notify();
                    window.refresh();
                    Ok((story, steps, request.postconditions, request.capture))
                })();

                match prepared {
                    Ok((story, steps, postconditions, capture)) => {
                        if response.is_closed() {
                            return;
                        }
                        crate::automation::interaction::schedule_story_interaction(
                            crate::automation::interaction::PreparedStoryInteraction {
                                request_id,
                                story,
                                steps,
                                postconditions,
                                capture,
                                response,
                                progress,
                                operation,
                            },
                            window,
                        );
                    },
                    Err(error) => {
                        let _ = response.send(Err(error));
                    },
                }
            },
        }
    }

    fn open_story_by_key(
        &mut self,
        key: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<StoryCurrentSnapshot, StorybookAutomationError> {
        let story_key = capture_route_story_key(key);
        StorySidebar::open_story_by_key(
            self.dock_area.downgrade(),
            story_key,
            self.automation.clone(),
            window,
            cx,
        )
        .ok_or_else(|| StorybookAutomationError::StoryNotFound {
            key: key.to_string(),
        })?;

        self.automation
            .as_ref()
            .expect("automation command requires automation")
            .confirm_current_story(key)
    }

    fn prepare_capture_current_story(
        &mut self,
        request: &StoryScreenshotRequest,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<StorySnapshot, StorybookAutomationError> {
        self.workbench_state
            .update(cx, |state, cx| state.apply_controls(&request.controls, cx))?;
        let story = self
            .automation
            .as_ref()
            .and_then(|automation| automation.current_story().story)
            .ok_or_else(|| StorybookAutomationError::CaptureUnavailable {
                message: "no current story is selected for capture".to_string(),
            })?;

        let target_size = validate_capture_target_size(request)?;
        let story_entity = self
            .workbench_state
            .read(cx)
            .active_story()
            .ok_or(StorybookAutomationError::NoActiveStory)?;
        set_capture_target_size(&story_entity, window, target_size, cx);
        cx.notify();
        window.refresh();

        Ok(story)
    }

    fn on_action_toggle_dock_toggle_button(
        &mut self,
        _: &ToggleDockToggleButton,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.toggle_button_visible = !self.toggle_button_visible;

        self.dock_skin
            .set_toggle_button_visible(self.toggle_button_visible, cx);
    }

    fn on_action_reset_layout(
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
        // Delete saved state file
        let _ = std::fs::remove_file(STATE_FILE);

        cx.notify();
    }
}

impl DockWindowView for StoryWorkspace {}

impl Render for StoryWorkspace {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let sheet_layer = Root::render_sheet_layer(window, cx);
        let dialog_layer = Root::render_dialog_layer(window, cx);
        let notification_layer = Root::render_notification_layer(window, cx);

        div()
            .id("story-workspace")
            .on_action(cx.listener(Self::on_action_toggle_dock_toggle_button))
            .on_action(cx.listener(Self::on_action_reset_layout))
            .on_drop(cx.listener(|this, drag: &StoryDrag, window, cx| {
                StorySidebar::open_story_by_klass(
                    this.dock_area.downgrade(),
                    drag.story_klass(),
                    window,
                    cx,
                );
            }))
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .child(self.title_bar.clone())
            .child(self.dock_area.clone())
            .children(sheet_layer)
            .children(dialog_layer)
            .children(notification_layer)
    }
}

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

/// Create a new dock-based storybook window
pub fn create_dock_window<F, V>(title: &str, create_view_fn: F, cx: &mut App)
where
    V: DockWindowView,
    F: FnOnce(&mut Window, &mut App) -> Entity<V> + Send + 'static,
{
    let options = default_storybook_window_options(cx);
    let title = SharedString::from(title.to_string());

    cx.bind_keys(vec![
        gpui::KeyBinding::new("shift-escape", ToggleZoom, None),
        gpui::KeyBinding::new("ctrl-w", ClosePanel, None),
    ]);

    cx.spawn(async move |cx| {
        let window = cx.open_window(options, |window, cx| {
            let view = create_view_fn(window, cx);
            cx.new(|cx| Root::new(view, window, cx))
        })?;

        window.update(cx, |_, window, _| {
            window.activate_window();
            window.set_window_title(&title);
        })?;

        Ok::<_, anyhow::Error>(())
    })
    .detach();
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::oneshot;

    #[gpui::test]
    fn default_layout_contains_open_versioned_right_workbench(cx: &mut App) {
        gpui_component::init(cx);
        let window: gpui::WindowHandle<DockArea> = cx
            .open_window(Default::default(), |window, cx| {
                let dock_area = cx.new(|cx| {
                    DockArea::new(MAIN_DOCK_AREA.id, Some(MAIN_DOCK_AREA.version), window, cx)
                });
                let state = cx.new(|_| WorkbenchState::new(None));
                register_workbench_state(&dock_area.downgrade(), &state);
                StoryWorkspace::reset_default_layout(dock_area.downgrade(), &[], None, window, cx);
                dock_area
            })
            .expect("dock test window should open");

        window
            .update(cx, |dock_area, _, cx| {
                let state = dock_area.dump(cx);
                assert_eq!(state.version, Some(6));
                let json = serde_json::to_string(&state).expect("dock layout serializes");
                assert!(json.contains("StoryWorkbench"));
                assert!(json.contains("right_dock"));
                assert!(json.contains("320"));
            })
            .expect("dock test window should update");
    }

    #[gpui::test]
    fn dock_host_rejects_an_invalid_batch_before_route_preparation(cx: &mut App) {
        gpui_component::init(cx);
        let automation = crate::automation::StorybookAutomation::new();
        let automation_for_view = automation.clone();
        let window: gpui::WindowHandle<StoryWorkspace> = cx
            .open_window(Default::default(), move |window, cx| {
                StoryWorkspace::view_with_automation(Vec::new(), automation_for_view, window, cx)
            })
            .expect("dock automation test window should open");

        window
            .update(cx, |workspace, window, cx| {
                let (response, mut result) = oneshot::channel();
                workspace.handle_automation_command(
                    StorybookAutomationCommand::RunSteps {
                        request_id: 9,
                        request: crate::automation::StoryInteractionRequest {
                            story_key: Some("missing-route".to_owned()),
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
                assert_eq!(automation.current_story().story, None);

                let (response, mut result) = oneshot::channel();
                workspace.handle_automation_command(
                    StorybookAutomationCommand::RunSteps {
                        request_id: 10,
                        request: crate::automation::StoryInteractionRequest {
                            story_key: None,
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
                            .expect("interaction operation should restart"),
                    },
                    window,
                    cx,
                );
                assert!(matches!(
                    result
                        .try_recv()
                        .expect("missing-story error should be sent"),
                    Err(StorybookAutomationError::NoActiveStory)
                ));
                assert!(
                    automation.begin_operation().is_ok(),
                    "a preparation failure should release the operation guard"
                );
            })
            .expect("dock host should handle the invalid batch");
    }
}
