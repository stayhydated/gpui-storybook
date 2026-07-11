use crate::{
    automation::{
        SharedStorybookAutomation, StoryCurrentSnapshot, StoryScreenshotRequest, StorySnapshot,
        StorybookAutomationCommand, StorybookAutomationError, apply_capture_target_size,
        default_storybook_automation, schedule_story_capture, story_snapshots_from_containers,
        validate_capture_target_size,
    },
    capture_region::capture_route_story_key,
    story::StoryContainer,
};
use gpui::prelude::{
    Context, FluentBuilder as _, InteractiveElement as _, IntoElement, ParentElement as _, Render,
    StatefulInteractiveElement as _, Styled as _,
};
use gpui::{
    App, AppContext as _, ClickEvent, Entity, SharedString, Subscription, Window, div, px, relative,
};
use gpui_component::{
    ActiveTheme as _, h_flex,
    input::{Input, InputEvent, InputState},
    resizable::{h_resizable, resizable_panel},
    sidebar::{Sidebar, SidebarGroup, SidebarMenu, SidebarMenuItem},
    v_flex,
};
use std::{borrow::Borrow, collections::BTreeMap};

pub struct Gallery {
    stories: Vec<Entity<StoryContainer>>,
    active_index: Option<usize>,
    collapsed: bool,
    search_input: Entity<InputState>,
    automation: Option<SharedStorybookAutomation>,

    _subscriptions: Vec<Subscription>,
}

impl Gallery {
    pub fn new(
        initial_stories: Vec<Entity<StoryContainer>>,
        init_story_name: Option<&str>,
        automation: Option<SharedStorybookAutomation>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let search_input =
            cx.new(|cx_input| InputState::new(window, cx_input).placeholder("Search..."));

        let subscriptions = vec![
            #[allow(clippy::single_match)]
            cx.subscribe(&search_input, |this, _, event, cx_window| match event {
                InputEvent::Change => {
                    let query = this
                        .search_input
                        .read(cx_window)
                        .value()
                        .trim()
                        .to_lowercase();
                    let filtered_stories_on_change: Vec<_> = this
                        .stories
                        .iter()
                        .filter(|story| {
                            let story_data = story.read(cx_window);
                            let title = story_data.display_title(cx_window);
                            let section = story_data
                                .section
                                .as_ref()
                                .map(|s| s.to_string())
                                .unwrap_or_default();
                            let group = story_data
                                .group
                                .as_ref()
                                .map(|s| s.to_string())
                                .unwrap_or_default();
                            title.to_lowercase().contains(&query)
                                || group.to_lowercase().contains(&query)
                                || section.to_lowercase().contains(&query)
                        })
                        .cloned()
                        .collect();

                    if let Some(first_filtered_story) = filtered_stories_on_change.first()
                        && let Some(original_idx) =
                            this.stories.iter().position(|s| s == first_filtered_story)
                    {
                        this.active_index = Some(original_idx);
                    } else {
                        this.active_index = None;
                    }
                    this.confirm_active_story(cx_window);
                    cx_window.notify();
                },
                _ => {},
            }),
        ];

        let mut this = Self {
            search_input,
            stories: initial_stories.clone(),
            active_index: if initial_stories.is_empty() {
                None
            } else {
                Some(0)
            },
            collapsed: false,
            automation,
            _subscriptions: subscriptions,
        };

        if let Some(name) = init_story_name {
            this.set_active_story(name, cx);
        }

        this.sync_automation_stories(cx);
        this.confirm_active_story(cx);
        if let Some(automation) = this.automation.clone() {
            this.attach_automation_host(automation, window, cx);
        }

        this
    }

    fn set_active_story(&mut self, name: &str, app_cx: &App) {
        let lowercase_name = name.to_lowercase().replace("story", "");
        let story_index = self.stories.iter().position(|story_entity| {
            let story_data = story_entity.read(app_cx);
            let title = story_data.display_title(app_cx);
            title.to_lowercase().replace("story", "") == lowercase_name
        });

        if let Some(index) = story_index {
            self.active_index = Some(index);
        }
    }

    fn active_story_snapshot(&self, cx: &impl Borrow<App>) -> Option<StorySnapshot> {
        let active_index = self.active_index?;
        let story = self.stories.get(active_index)?;
        StorySnapshot::from_container(story.read(cx.borrow()), cx)
    }

    fn sync_automation_stories(&self, cx: &impl Borrow<App>) {
        if let Some(automation) = &self.automation {
            automation.set_stories(story_snapshots_from_containers(&self.stories, cx));
        }
    }

    fn confirm_active_story(&self, cx: &impl Borrow<App>) {
        let Some(automation) = &self.automation else {
            return;
        };
        let Some(snapshot) = self.active_story_snapshot(cx) else {
            return;
        };

        let _ = automation.confirm_current_story(&snapshot.key);
    }

    fn story_contains_key(
        story: &Entity<StoryContainer>,
        key: &str,
        cx: &impl Borrow<App>,
    ) -> bool {
        let (matches, members) = {
            let story = story.read(cx.borrow());
            (
                story
                    .story_key_label()
                    .is_some_and(|story_key| story_key == key),
                story.list_members.clone(),
            )
        };

        matches
            || members
                .iter()
                .any(|member| Self::story_contains_key(member, key, cx))
    }

    fn set_active_story_by_key(
        &mut self,
        key: &str,
        cx: &impl Borrow<App>,
    ) -> Result<StoryCurrentSnapshot, StorybookAutomationError> {
        let story_key = capture_route_story_key(key);
        let Some(index) = self
            .stories
            .iter()
            .position(|story| Self::story_contains_key(story, story_key, cx))
        else {
            return Err(StorybookAutomationError::StoryNotFound {
                key: key.to_string(),
            });
        };

        self.active_index = Some(index);
        self.automation
            .as_ref()
            .expect("automation command requires automation")
            .confirm_current_story(key)
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
                let _ = this.update_in(cx, |gallery, window, cx| {
                    gallery.handle_automation_command(command, window, cx);
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
            StorybookAutomationCommand::OpenStory { key, response } => {
                let result = self.set_active_story_by_key(&key, cx);
                cx.notify();
                let _ = response.send(result);
            },
            StorybookAutomationCommand::CaptureCurrentStory {
                request_id,
                request,
                response,
            } => {
                let quit_after_capture = request.quit_after_capture;
                match self.prepare_capture_current_story(&request, window, cx) {
                    Ok(story) => {
                        schedule_story_capture(
                            request_id,
                            request,
                            story,
                            response,
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
        }
    }

    fn prepare_capture_current_story(
        &mut self,
        request: &StoryScreenshotRequest,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<StorySnapshot, StorybookAutomationError> {
        let story = self
            .automation
            .as_ref()
            .and_then(|automation| automation.current_story().story)
            .or_else(|| self.active_story_snapshot(cx))
            .ok_or_else(|| StorybookAutomationError::CaptureUnavailable {
                message: "no current story is selected for capture".to_string(),
            })?;

        apply_capture_target_size(window, validate_capture_target_size(request)?);
        cx.notify();
        window.refresh();

        Ok(story)
    }

    pub fn view(
        initial_stories: Vec<Entity<StoryContainer>>,
        init_story_name: Option<&str>,
        window: &mut Window,
        cx: &mut App,
    ) -> Entity<Self> {
        let automation = default_storybook_automation(cx);
        cx.new(|cx_self| {
            Self::new(
                initial_stories,
                init_story_name,
                automation,
                window,
                cx_self,
            )
        })
    }

    pub fn view_with_automation(
        initial_stories: Vec<Entity<StoryContainer>>,
        init_story_name: Option<&str>,
        automation: SharedStorybookAutomation,
        window: &mut Window,
        cx: &mut App,
    ) -> Entity<Self> {
        cx.new(|cx_self| {
            Self::new(
                initial_stories,
                init_story_name,
                Some(automation),
                window,
                cx_self,
            )
        })
    }
}

impl crate::window_view::SimpleWindowView for Gallery {}

impl Render for Gallery {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let query = self.search_input.read(cx).value().trim().to_lowercase();

        let filtered_stories: Vec<Entity<StoryContainer>> = self
            .stories
            .iter()
            .filter(|story| {
                let story_data = story.read(cx);
                let title = story_data.display_title(cx);
                let section = story_data
                    .section
                    .as_ref()
                    .map(|s| s.to_string())
                    .unwrap_or_default();
                let group = story_data
                    .group
                    .as_ref()
                    .map(|s| s.to_string())
                    .unwrap_or_default();
                title.to_lowercase().contains(&query)
                    || group.to_lowercase().contains(&query)
                    || section.to_lowercase().contains(&query)
            })
            .cloned()
            .collect();

        let mut active_story_to_render: Option<Entity<StoryContainer>> = None;
        let mut ui_active_index_in_filtered_list: Option<usize> = None;

        if let Some(current_original_idx) = self.active_index
            && let Some(story_from_original_list) = self.stories.get(current_original_idx)
            && let Some(idx_in_filtered) = filtered_stories
                .iter()
                .position(|s| s == story_from_original_list)
        {
            active_story_to_render = Some(story_from_original_list.clone());
            ui_active_index_in_filtered_list = Some(idx_in_filtered);
        }

        let (story_name, description) =
            if let Some(story_to_render_cloned) = active_story_to_render.as_ref() {
                let story_data = story_to_render_cloned.read(cx);
                let title = story_data.display_title(cx);
                let desc = story_data.display_description(cx);
                (title, desc)
            } else {
                ("".to_owned(), "".to_owned())
            };

        h_resizable("gallery-container")
            .child(
                resizable_panel()
                    .size(px(255.))
                    .size_range(px(200.)..px(320.))
                    .child(
                        Sidebar::new("sidebar-gallery")
                            .side(gpui_component::Side::Left)
                            .w(relative(1.))
                            .border_0()
                            .collapsed(self.collapsed)
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
                            .children({
                                // Group stories by crate group, then optional section.
                                let mut groups: BTreeMap<
                                    Option<SharedString>,
                                    BTreeMap<
                                        Option<SharedString>,
                                        Vec<(usize, Entity<StoryContainer>)>,
                                    >,
                                > = BTreeMap::new();

                                for (idx_in_filtered, story_entity) in
                                    filtered_stories.iter().enumerate()
                                {
                                    let (group, section) = {
                                        let story_data = story_entity.read(cx);
                                        (story_data.sidebar_group(), story_data.sidebar_section())
                                    };

                                    groups
                                        .entry(group)
                                        .or_default()
                                        .entry(section)
                                        .or_default()
                                        .push((idx_in_filtered, story_entity.clone()));
                                }

                                // Build sidebar groups with menus.
                                groups
                                    .into_iter()
                                    .map(|(group, sections_in_group)| {
                                        let menu_items: Vec<_> = sections_in_group
                                            .into_iter()
                                            .flat_map(|(section, stories_in_section)| {
                                                let story_items: Vec<_> = stories_in_section
                                                    .into_iter()
                                                    .map(
                                                        |(
                                                            idx_in_filtered,
                                                            story_entity_in_filtered,
                                                        )| {
                                                            let story_data =
                                                                story_entity_in_filtered.read(cx);
                                                            let name: SharedString = story_data
                                                                .display_title(cx)
                                                                .into();
                                                            let is_active =
                                                                ui_active_index_in_filtered_list
                                                                    == Some(idx_in_filtered);

                                                            let story_entity_for_click =
                                                                story_entity_in_filtered.clone();

                                                            SidebarMenuItem::new(name)
                                                                .active(is_active)
                                                                .on_click(cx.listener(
                                                                    move |this,
                                                                          _: &ClickEvent,
                                                                          _,
                                                                          cx_listener| {
                                                                        if let Some(original_idx) =
                                                                            this.stories.iter().position(|s| {
                                                                                s == &story_entity_for_click
                                                                            })
                                                                        {
                                                                            this.active_index = Some(original_idx);
                                                                        }
                                                                        cx_listener.notify();
                                                                    },
                                                                ))
                                                        },
                                                    )
                                                    .collect();

                                                if let Some(section) = section {
                                                    vec![
                                                        SidebarMenuItem::new(section)
                                                            .default_open(true)
                                                            .children(story_items),
                                                    ]
                                                } else {
                                                    story_items
                                                }
                                            })
                                            .collect();

                                        let menu = SidebarMenu::new().children(menu_items);

                                        SidebarGroup::new(group.unwrap_or_default()).child(menu)
                                    })
                                    .collect::<Vec<_>>()
                            }),
                    ),
            )
            .child(
                v_flex()
                    .flex_1()
                    .h_full()
                    .overflow_x_hidden()
                    .child(
                        h_flex()
                            .id("header")
                            .p_4()
                            .border_b_1()
                            .border_color(cx.theme().border)
                            .justify_between()
                            .items_start()
                            .child(
                                v_flex()
                                    .gap_1()
                                    .child(div().text_xl().child(story_name))
                                    .child(
                                        div()
                                            .text_color(cx.theme().muted_foreground)
                                            .child(description),
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .id("story")
                            .flex_1()
                            .overflow_y_scroll()
                            .when_some(active_story_to_render, |this, active_story_ref| {
                                this.child(active_story_ref)
                            }),
                    )
                    .into_any_element(),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::{RegisteredStoryMetadata, StoryKey, StoryName};
    use tokio::sync::oneshot;

    fn story(
        key: &'static str,
        name: &'static str,
        title: &'static str,
        window: &mut Window,
        cx: &mut App,
    ) -> Entity<StoryContainer> {
        cx.new(|cx| {
            let mut story = StoryContainer::new(window, cx);
            story.name = title.into();
            story.set_registration_metadata(RegisteredStoryMetadata::new(
                StoryKey::new(key),
                StoryName::new(name),
                None,
                "crate",
                "src/stories.rs",
                1,
            ));
            story
        })
    }

    #[gpui::test]
    fn gallery_selects_by_title_key_and_automation_command(cx: &mut App) {
        gpui_component::init(cx);
        let automation = crate::automation::StorybookAutomation::new();
        let automation_for_view = automation.clone();
        let window: gpui::WindowHandle<Gallery> = cx
            .open_window(Default::default(), move |window, cx| {
                let button = story("crate-ButtonStory", "ButtonStory", "Button", window, cx);
                let table = story("crate-TableStory", "TableStory", "Table", window, cx);
                Gallery::view_with_automation(
                    vec![button, table],
                    Some("TableStory"),
                    automation_for_view,
                    window,
                    cx,
                )
            })
            .expect("gallery window should open");

        window
            .update(cx, |gallery, window, cx| {
                assert_eq!(gallery.active_index, Some(1));
                assert!(!gallery.collapsed);
                assert_eq!(
                    gallery
                        .active_story_snapshot(cx)
                        .expect("table should be active")
                        .key,
                    "crate-TableStory"
                );

                gallery.set_active_story("ButtonStory", cx);
                assert_eq!(gallery.active_index, Some(0));
                gallery.set_active_story("MissingStory", cx);
                assert_eq!(gallery.active_index, Some(0));

                let selected = gallery
                    .set_active_story_by_key("crate-ButtonStory/with-icon", cx)
                    .expect("substory key should select its parent story");
                assert_eq!(
                    selected
                        .story
                        .expect("selected story should be returned")
                        .capture_route_id,
                    "crate-ButtonStory/with-icon"
                );
                assert!(matches!(
                    gallery.set_active_story_by_key("missing", cx),
                    Err(StorybookAutomationError::StoryNotFound { key }) if key == "missing"
                ));

                let (response, mut result) = oneshot::channel();
                gallery.handle_automation_command(
                    StorybookAutomationCommand::OpenStory {
                        key: "crate-TableStory".to_string(),
                        response,
                    },
                    window,
                    cx,
                );
                assert_eq!(
                    result
                        .try_recv()
                        .expect("open response should be sent")
                        .expect("table should open")
                        .story
                        .expect("table snapshot should exist")
                        .key,
                    "crate-TableStory"
                );

                gallery.stories.clear();
                gallery.active_index = None;
                automation.set_stories(Vec::new());
                let error = gallery
                    .prepare_capture_current_story(&StoryScreenshotRequest::default(), window, cx)
                    .expect_err("capture requires a selected story");
                assert!(matches!(
                    error,
                    StorybookAutomationError::CaptureUnavailable { message }
                        if message.contains("no current story")
                ));

                let (response, mut result) = oneshot::channel();
                gallery.handle_automation_command(
                    StorybookAutomationCommand::CaptureCurrentStory {
                        request_id: 7,
                        request: StoryScreenshotRequest::default(),
                        response,
                    },
                    window,
                    cx,
                );
                assert!(matches!(
                    result.try_recv().expect("capture error should be sent"),
                    Err(StorybookAutomationError::CaptureUnavailable { .. })
                ));
            })
            .expect("gallery should update");
    }

    #[gpui::test]
    fn empty_gallery_has_no_active_story(cx: &mut App) {
        gpui_component::init(cx);
        let window: gpui::WindowHandle<Gallery> = cx
            .open_window(Default::default(), |window, cx| {
                Gallery::view(Vec::new(), Some("Missing"), window, cx)
            })
            .expect("empty gallery window should open");

        window
            .update(cx, |gallery, _, cx| {
                assert_eq!(gallery.active_index, None);
                assert_eq!(gallery.active_story_snapshot(cx), None);
                gallery.sync_automation_stories(cx);
                gallery.confirm_active_story(cx);
            })
            .expect("empty gallery should update");
    }
}
