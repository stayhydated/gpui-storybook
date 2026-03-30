use crate::{
    registry::StoryEntry,
    story::{StoryContainer, StoryState, reveal_story_panel},
    title_bar::AppTitleBar,
    window_options::default_storybook_window_options,
};
use anyhow::{Context as _, Result};
use gpui::{
    App, AppContext as _, ClickEvent, Context, Edges, Entity, EventEmitter, FocusHandle, Focusable,
    InteractiveElement as _, IntoElement, ParentElement as _, Render, SharedString, Styled as _,
    Subscription, Task, Window, actions, div, px, relative,
};
use gpui_component::{
    ActiveTheme as _, Root,
    dock::{
        ClosePanel, DockArea, DockAreaState, DockEvent, DockItem, DockPlacement, Panel,
        PanelControl, PanelEvent, PanelInfo, PanelView, ToggleZoom, register_panel,
    },
    input::{Input, InputEvent, InputState},
    sidebar::{Sidebar, SidebarGroup},
    v_flex,
};
use gpui_storybook_components::{StoryDrag, StorySidebarItem};
use std::{
    collections::BTreeMap,
    sync::{Arc, LazyLock, Mutex},
    time::Duration,
};

actions!(story, [ToggleDockToggleButton, ResetLayout, ToggleSidebar]);

const MAIN_DOCK_AREA: DockAreaTab = DockAreaTab {
    id: "storybook-main-dock",
    version: 5, // Bumped version to remove sidebar tab title row
};

#[cfg(debug_assertions)]
const STATE_FILE: &str = "target/storybook-docks.json";
#[cfg(not(debug_assertions))]
const STATE_FILE: &str = "storybook-docks.json";

struct DockAreaTab {
    id: &'static str,
    version: usize,
}

static STORY_PANELS: LazyLock<Mutex<BTreeMap<String, gpui::WeakEntity<StoryContainer>>>> =
    LazyLock::new(|| Mutex::new(BTreeMap::new()));

/// Sidebar panel for navigating stories
pub struct StorySidebar {
    focus_handle: FocusHandle,
    search_input: Entity<InputState>,
    stories: Vec<Entity<StoryContainer>>,
    dock_area: gpui::WeakEntity<DockArea>,
    _subscriptions: Vec<Subscription>,
}

impl StorySidebar {
    pub fn new(
        stories: Vec<Entity<StoryContainer>>,
        dock_area: gpui::WeakEntity<DockArea>,
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
            _subscriptions: subscriptions,
        }
    }

    /// Open a story panel and reveal it if it is already mounted in a tab.
    fn open_story(
        dock_area: gpui::WeakEntity<DockArea>,
        story: Entity<StoryContainer>,
        window: &mut Window,
        cx: &mut App,
    ) {
        let Some(dock_area) = dock_area.upgrade() else {
            return;
        };

        if reveal_story_panel(&story, window, cx) {
            return;
        }

        let panel: Arc<dyn PanelView> = Arc::new(story);
        dock_area.update(cx, |dock_area, cx| {
            // Keep the declarative DockItem tree synced with runtime state before adding.
            // This avoids stale split children on gpui-component/main.
            let state = dock_area.dump(cx);
            let _ = dock_area.load(state, window, cx);

            dock_area.remove_panel_from_all_docks(panel.clone(), window, cx);
            dock_area.add_panel(panel, DockPlacement::Center, None, window, cx);
        });
    }

    fn create_story_from_entry(
        entry: &StoryEntry,
        window: &mut Window,
        cx: &mut App,
    ) -> Entity<StoryContainer> {
        if let Some(story) = Self::find_story_by_klass(entry.name) {
            return story;
        }

        let panel = (entry.create_fn)(window, cx);
        if let Some(section) = entry.section {
            panel.update(cx, |c, _| {
                c.section = Some(section.into());
            });
        }
        Self::register_story(&panel, cx);
        panel
    }

    fn create_story_by_klass(
        story_klass: &str,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<Entity<StoryContainer>> {
        inventory::iter::<StoryEntry>()
            .find(|entry| entry.name == story_klass)
            .map(|entry| Self::create_story_from_entry(entry, window, cx))
    }

    fn register_story(story: &Entity<StoryContainer>, cx: &App) {
        let Some(story_klass) = story.read(cx).story_klass.clone() else {
            return;
        };

        if let Ok(mut registry) = STORY_PANELS.lock() {
            registry.insert(story_klass.to_string(), story.downgrade());
        }
    }

    fn register_stories(stories: &[Entity<StoryContainer>], cx: &App) {
        for story in stories {
            Self::register_story(story, cx);
        }
    }

    fn find_story_by_klass(story_klass: &str) -> Option<Entity<StoryContainer>> {
        let mut registry = STORY_PANELS.lock().ok()?;
        if let Some(story) = registry.get(story_klass).and_then(|story| story.upgrade()) {
            return Some(story);
        }

        registry.remove(story_klass);
        None
    }

    fn open_story_by_klass(
        dock_area: gpui::WeakEntity<DockArea>,
        story_klass: &str,
        window: &mut Window,
        cx: &mut App,
    ) {
        if let Some(story) = Self::create_story_by_klass(story_klass, window, cx) {
            Self::open_story(dock_area, story, window, cx);
        }
    }
}

impl Panel for StorySidebar {
    fn panel_name(&self) -> &'static str {
        "StorySidebar"
    }

    fn title(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        "Stories"
    }

    fn closable(&self, _cx: &App) -> bool {
        false
    }

    fn zoomable(&self, _cx: &App) -> Option<PanelControl> {
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
        let query = self.search_input.read(cx).value().trim().to_lowercase();

        let filtered_stories: Vec<Entity<StoryContainer>> = self
            .stories
            .iter()
            .filter(|story| {
                let story_data = story.read(cx);
                let title = if let Some(title_fn) = &story_data.title_fn {
                    title_fn()
                } else {
                    story_data.name.to_string()
                };
                let section = story_data
                    .section
                    .as_ref()
                    .map(|s| s.to_string())
                    .unwrap_or_default();
                title.to_lowercase().contains(&query) || section.to_lowercase().contains(&query)
            })
            .cloned()
            .collect();

        // Group stories by section
        let mut sections: BTreeMap<Option<SharedString>, Vec<Entity<StoryContainer>>> =
            BTreeMap::new();

        for story_entity in filtered_stories.iter() {
            let section = story_entity.read(cx).section.clone();
            sections
                .entry(section)
                .or_default()
                .push(story_entity.clone());
        }

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
                sections
                    .into_iter()
                    .map(|(section, stories_in_section)| {
                        let menu_items: Vec<_> = stories_in_section
                            .into_iter()
                            .map(|story_entity| {
                                let story_data = story_entity.read(cx);
                                let name = if let Some(title_fn) = &story_data.title_fn {
                                    title_fn().into()
                                } else {
                                    story_data.name.clone()
                                };
                                let story_klass_for_drag =
                                    story_data.story_klass.clone().unwrap_or_default();

                                let story_for_click = story_entity.clone();
                                let dock_area_for_click = self.dock_area.clone();
                                StorySidebarItem::new(name, story_klass_for_drag).on_click(
                                    cx.listener(move |_, _: &ClickEvent, window, cx| {
                                        let dock_area_for_open = dock_area_for_click.clone();
                                        let story_for_open = story_for_click.clone();
                                        window.defer(cx, move |window, cx| {
                                            Self::open_story(
                                                dock_area_for_open.clone(),
                                                story_for_open.clone(),
                                                window,
                                                cx,
                                            );
                                        });
                                    }),
                                )
                            })
                            .collect();

                        SidebarGroup::new(section.unwrap_or_default()).children(menu_items)
                    })
                    .collect::<Vec<_>>(),
            )
    }
}

pub struct StoryWorkspace {
    title_bar: Entity<AppTitleBar>,
    dock_area: Entity<DockArea>,
    last_layout_state: Option<DockAreaState>,
    toggle_button_visible: bool,
    _save_layout_task: Option<Task<()>>,
}

impl StoryWorkspace {
    pub fn new(
        stories: Vec<Entity<StoryContainer>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let dock_area =
            cx.new(|cx| DockArea::new(MAIN_DOCK_AREA.id, Some(MAIN_DOCK_AREA.version), window, cx));
        let weak_dock_area = dock_area.downgrade();
        StorySidebar::register_stories(&stories, cx);

        // Try to load saved layout, fall back to default
        match Self::load_layout(dock_area.clone(), window, cx) {
            Ok(_) => {
                // Layout loaded successfully
            },
            Err(_) => {
                Self::reset_default_layout(weak_dock_area.clone(), &stories, window, cx);
            },
        };

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
                cx.background_executor().spawn(async move {
                    let _ = Self::save_state(&state);
                })
            }
        })
        .detach();

        let title_bar = cx.new(|cx| AppTitleBar::new("Storybook", window, cx));

        Self {
            dock_area,
            title_bar,
            last_layout_state: None,
            toggle_button_visible: true,
            _save_layout_task: None,
        }
    }

    fn save_layout(
        &mut self,
        dock_area: &Entity<DockArea>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let dock_area = dock_area.clone();
        self._save_layout_task = Some(cx.spawn_in(window, async move |this, window| {
            window
                .background_executor()
                .timer(Duration::from_secs(5))
                .await;

            _ = this.update_in(window, move |this, _, cx| {
                let dock_area = dock_area.read(cx);
                let state = dock_area.dump(cx);

                let last_layout_state = this.last_layout_state.clone();
                if Some(&state) == last_layout_state.as_ref() {
                    return;
                }

                let _ = Self::save_state(&state);
                this.last_layout_state = Some(state);
            });
        }));
    }

    fn save_state(state: &DockAreaState) -> Result<()> {
        let json = serde_json::to_string_pretty(state)?;
        std::fs::write(STATE_FILE, json)?;
        Ok(())
    }

    fn load_layout(
        dock_area: Entity<DockArea>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<()> {
        let json = std::fs::read_to_string(STATE_FILE)?;
        let state = serde_json::from_str::<DockAreaState>(&json)?;

        // Check if the saved layout version matches
        if state.version != Some(MAIN_DOCK_AREA.version) {
            anyhow::bail!("Layout version mismatch");
        }

        dock_area.update(cx, |dock_area, cx| {
            dock_area.load(state, window, cx).context("load layout")?;
            dock_area.set_dock_collapsible(
                Edges {
                    left: true,
                    bottom: true,
                    right: true,
                    ..Default::default()
                },
                window,
                cx,
            );
            Ok::<(), anyhow::Error>(())
        })
    }

    fn reset_default_layout(
        dock_area: gpui::WeakEntity<DockArea>,
        stories: &[Entity<StoryContainer>],
        window: &mut Window,
        cx: &mut App,
    ) {
        let dock_item = Self::build_center_layout(stories, &dock_area, window, cx);

        // Create sidebar panel for the left dock
        let sidebar_panel = Self::build_sidebar(stories, &dock_area, window, cx);

        _ = dock_area.update(cx, |view, cx| {
            view.set_version(MAIN_DOCK_AREA.version, window, cx);
            view.set_center(dock_item, window, cx);
            view.set_left_dock(sidebar_panel, Some(px(260.)), true, window, cx);
            view.set_dock_collapsible(
                Edges {
                    left: true,
                    bottom: true,
                    right: true,
                    ..Default::default()
                },
                window,
                cx,
            );
        });
    }

    fn build_sidebar(
        stories: &[Entity<StoryContainer>],
        dock_area: &gpui::WeakEntity<DockArea>,
        window: &mut Window,
        cx: &mut App,
    ) -> DockItem {
        let sidebar: Arc<dyn PanelView> = Arc::new(
            cx.new(|cx| StorySidebar::new(stories.to_vec(), dock_area.clone(), window, cx)),
        );

        DockItem::panel(sidebar)
    }

    fn build_center_layout(
        _stories: &[Entity<StoryContainer>],
        dock_area: &gpui::WeakEntity<DockArea>,
        window: &mut Window,
        cx: &mut App,
    ) -> DockItem {
        // Wrap center tabs in a split so TabPanel gets a parent StackPanel.
        // This enables tab drag/drop and split indicators.
        DockItem::v_split(
            vec![DockItem::tabs(vec![], dock_area, window, cx)],
            dock_area,
            window,
            cx,
        )
    }

    pub fn view(
        stories: Vec<Entity<StoryContainer>>,
        window: &mut Window,
        cx: &mut App,
    ) -> Entity<Self> {
        cx.new(|cx| Self::new(stories, window, cx))
    }

    fn on_action_toggle_dock_toggle_button(
        &mut self,
        _: &ToggleDockToggleButton,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.toggle_button_visible = !self.toggle_button_visible;

        self.dock_area.update(cx, |dock_area, cx| {
            dock_area.set_toggle_button_visible(self.toggle_button_visible, cx);
        });
    }

    fn on_action_reset_layout(
        &mut self,
        _: &ResetLayout,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Collect stories from the registry
        let entries: Vec<_> = inventory::iter::<StoryEntry>().collect();
        let stories: Vec<Entity<StoryContainer>> =
            entries.iter().map(|e| (e.create_fn)(window, cx)).collect();
        StorySidebar::register_stories(&stories, cx);

        let weak_dock_area = self.dock_area.downgrade();
        Self::reset_default_layout(weak_dock_area, &stories, window, cx);

        // Delete saved state file
        let _ = std::fs::remove_file(STATE_FILE);

        cx.notify();
    }
}

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
    register_panel(
        cx,
        "StoryContainer",
        |_dock_area, state, info, window, cx| {
            // Try to recreate the story from saved state
            // Extract the panel info value from the Panel variant
            let panel_value = match info {
                PanelInfo::Panel(value) => Some(value.clone()),
                _ => None,
            };

            if let Some(story_state) =
                panel_value.and_then(|v| serde_json::from_value::<StoryState>(v).ok())
                && let Some(container) = StorySidebar::create_story_by_klass(
                    story_state.story_klass.as_ref(),
                    window,
                    cx,
                )
            {
                return Box::new(container);
            }

            // Fallback: create an empty container with the panel name
            Box::new(cx.new(|cx| {
                let mut container = StoryContainer::new(window, cx);
                container.name = state.panel_name.clone().into();
                container
            }))
        },
    );

    // Register StorySidebar panel
    register_panel(
        cx,
        "StorySidebar",
        |dock_area, _state, _info, window, cx| {
            // Recreate the sidebar with all stories from the registry
            let entries: Vec<_> = inventory::iter::<StoryEntry>().collect();
            let stories: Vec<Entity<StoryContainer>> = entries
                .iter()
                .map(|entry| StorySidebar::create_story_from_entry(entry, window, cx))
                .collect();

            Box::new(cx.new(|cx| StorySidebar::new(stories, dock_area, window, cx)))
        },
    );
}

/// Create a new dock-based storybook window
pub fn create_dock_window<F, E>(title: &str, create_view_fn: F, cx: &mut App)
where
    E: Into<gpui::AnyView>,
    F: FnOnce(&mut Window, &mut App) -> E + Send + 'static,
{
    let options = default_storybook_window_options(cx);
    let title = SharedString::from(title.to_string());

    cx.bind_keys(vec![
        gpui::KeyBinding::new("shift-escape", ToggleZoom, None),
        gpui::KeyBinding::new("ctrl-w", ClosePanel, None),
    ]);

    cx.spawn(async move |cx| {
        let window = cx
            .open_window(options, |window, cx| {
                let view = create_view_fn(window, cx);
                cx.new(|cx| Root::new(view, window, cx))
            })
            .expect("failed to open window");

        window
            .update(cx, |_, window, _| {
                window.activate_window();
                window.set_window_title(&title);
            })
            .expect("failed to update window");

        Ok::<_, anyhow::Error>(())
    })
    .detach();
}
