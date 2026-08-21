use crate::{
    automation::{
        SharedStorybookAutomation, StoryCurrentSnapshot, StoryScreenshotRequest, StorySnapshot,
        StorybookAutomationCommand, StorybookAutomationError, default_storybook_automation,
        schedule_story_capture, set_capture_target_size, story_snapshots_from_containers,
        validate_capture_target_size,
    },
    capture_region::capture_route_story_key,
    story::StoryContainer,
    title_bar::sidebar_toggle_button,
    workbench::{StoryWorkbench, WorkbenchState, WorkbenchTab},
};
use gpui::prelude::{
    Context, FluentBuilder as _, InteractiveElement as _, IntoElement, ParentElement as _, Render,
    Styled as _,
};
use gpui::{
    AnyElement, App, AppContext as _, ClickEvent, Entity, Pixels, SharedString, Subscription,
    Window, div, px, relative,
};
use gpui_component::{
    ActiveTheme as _, ElementExt as _, Side, h_flex,
    input::{Input, InputEvent, InputState},
    resizable::{h_resizable, resizable_panel},
    sidebar::{Sidebar, SidebarGroup, SidebarMenu, SidebarMenuItem},
    v_flex,
};
use std::{borrow::Borrow, collections::BTreeMap};

/// Searchable gallery host with independently toggled navigation and workbench
/// sidebars around a story canvas centered within the available main pane.
pub struct Gallery {
    stories: Vec<Entity<StoryContainer>>,
    active_index: Option<usize>,
    left_sidebar_visible: bool,
    right_sidebar_visible: bool,
    left_sidebar_width: Pixels,
    right_sidebar_width: Pixels,
    search_input: Entity<InputState>,
    automation: Option<SharedStorybookAutomation>,
    workbench_state: Entity<WorkbenchState>,
    workbench: Entity<StoryWorkbench>,

    _subscriptions: Vec<Subscription>,
}

const DEFAULT_LEFT_SIDEBAR_WIDTH: Pixels = px(255.);
const DEFAULT_RIGHT_SIDEBAR_WIDTH: Pixels = px(320.);

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
        let workbench_state = cx.new(|_| WorkbenchState::new(None));
        let workbench = cx.new(|cx| {
            StoryWorkbench::new(workbench_state.clone(), WorkbenchTab::Controls, window, cx)
        });
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
                    this.sync_workbench_active(cx_window);
                    this.confirm_active_story(cx_window);
                    cx_window.notify();
                },
                _ => {},
            }),
            cx.observe(&workbench_state, |this, state, cx| {
                let Some(automation) = &this.automation else {
                    return;
                };
                let Some(story) = state.read(cx).active_story() else {
                    return;
                };
                let Some(key) = story.read(cx).story_key_label() else {
                    return;
                };
                let _ = automation.confirm_current_story(key);
                cx.notify();
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
            left_sidebar_visible: true,
            right_sidebar_visible: true,
            left_sidebar_width: DEFAULT_LEFT_SIDEBAR_WIDTH,
            right_sidebar_width: DEFAULT_RIGHT_SIDEBAR_WIDTH,
            automation,
            workbench_state,
            workbench,
            _subscriptions: subscriptions,
        };

        if let Some(name) = init_story_name {
            this.set_active_story(name, cx);
        }

        this.sync_automation_stories(cx);
        this.sync_workbench_active(cx);
        this.confirm_active_story(cx);
        if let Some(automation) = this.automation.clone() {
            this.attach_automation_host(automation, window, cx);
        }

        this
    }

    fn set_left_sidebar_width(&mut self, width: Pixels) {
        if self.left_sidebar_width == width {
            return;
        }
        self.left_sidebar_width = width;
    }

    fn set_right_sidebar_width(&mut self, width: Pixels) {
        if self.right_sidebar_width == width {
            return;
        }
        self.right_sidebar_width = width;
    }

    fn toggle_left_sidebar(&mut self, cx: &mut Context<Self>) {
        self.left_sidebar_visible = !self.left_sidebar_visible;
        cx.notify();
    }

    fn toggle_right_sidebar(&mut self, cx: &mut Context<Self>) {
        self.right_sidebar_visible = !self.right_sidebar_visible;
        cx.notify();
    }

    pub(crate) fn title_bar_sidebar_controls(gallery: Entity<Self>, cx: &App) -> AnyElement {
        let (left_collapsed, right_collapsed) = {
            let gallery = gallery.read(cx);
            (
                !gallery.left_sidebar_visible,
                !gallery.right_sidebar_visible,
            )
        };
        let gallery_for_left = gallery.clone();
        let gallery_for_right = gallery;

        h_flex()
            .gap_1()
            .child(
                div()
                    .debug_selector(|| "gallery-toggle-left-sidebar".to_owned())
                    .child(
                        sidebar_toggle_button(
                            "gallery-toggle-left-sidebar-button",
                            Side::Left,
                            left_collapsed,
                        )
                        .tooltip(if left_collapsed {
                            "Show story navigation"
                        } else {
                            "Hide story navigation"
                        })
                        .on_click(move |_, window, cx| {
                            gallery_for_left.update(cx, |gallery, cx| {
                                gallery.toggle_left_sidebar(cx);
                            });
                            window.refresh();
                        }),
                    ),
            )
            .child(
                div()
                    .debug_selector(|| "gallery-toggle-right-sidebar".to_owned())
                    .child(
                        sidebar_toggle_button(
                            "gallery-toggle-right-sidebar-button",
                            Side::Right,
                            right_collapsed,
                        )
                        .tooltip(if right_collapsed {
                            "Show story workbench"
                        } else {
                            "Hide story workbench"
                        })
                        .on_click(move |_, window, cx| {
                            gallery_for_right.update(cx, |gallery, cx| {
                                gallery.toggle_right_sidebar(cx);
                            });
                            window.refresh();
                        }),
                    ),
            )
            .into_any_element()
    }

    fn render_story_sidebar(
        &self,
        filtered_stories: &[Entity<StoryContainer>],
        active_filtered_index: Option<usize>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let mut groups: BTreeMap<
            Option<SharedString>,
            BTreeMap<Option<SharedString>, Vec<(usize, Entity<StoryContainer>)>>,
        > = BTreeMap::new();
        for (filtered_index, story) in filtered_stories.iter().enumerate() {
            let story_data = story.read(cx);
            groups
                .entry(story_data.sidebar_group())
                .or_default()
                .entry(story_data.sidebar_section())
                .or_default()
                .push((filtered_index, story.clone()));
        }

        let groups = groups.into_iter().map(|(group, sections)| {
            let menu_items = sections.into_iter().flat_map(|(section, stories)| {
                let story_items = stories
                    .into_iter()
                    .map(|(filtered_index, story)| {
                        let story_data = story.read(cx);
                        let name: SharedString = story_data.display_title(cx).into();
                        let is_active = active_filtered_index == Some(filtered_index);

                        SidebarMenuItem::new(name)
                            .active(is_active)
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                if let Some(original_index) = this
                                    .stories
                                    .iter()
                                    .position(|candidate| candidate == &story)
                                {
                                    this.active_index = Some(original_index);
                                }
                                this.sync_workbench_active(cx);
                                cx.notify();
                            }))
                    })
                    .collect::<Vec<_>>();

                if let Some(section) = section {
                    vec![
                        SidebarMenuItem::new(section)
                            .default_open(true)
                            .children(story_items),
                    ]
                } else {
                    story_items
                }
            });

            SidebarGroup::new(group.unwrap_or_default())
                .child(SidebarMenu::new().children(menu_items))
        });

        Sidebar::new("sidebar-gallery")
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
            .children(groups)
    }

    fn set_active_story(&mut self, name: &str, app_cx: &mut App) {
        let lowercase_name = name.to_lowercase().replace("story", "");
        let story_index = self.stories.iter().position(|story_entity| {
            let story_data = story_entity.read(app_cx);
            let title = story_data.display_title(app_cx);
            title.to_lowercase().replace("story", "") == lowercase_name
        });

        if let Some(index) = story_index {
            self.active_index = Some(index);
            self.sync_workbench_active(app_cx);
        }
    }

    fn sync_workbench_active(&self, cx: &mut App) {
        let story = self
            .active_index
            .and_then(|index| self.stories.get(index))
            .cloned();
        self.workbench_state.update(cx, |state, cx| {
            state.set_active_story(story, cx);
        });
    }

    fn active_story_snapshot(&self, cx: &impl Borrow<App>) -> Option<StorySnapshot> {
        let story = self.workbench_state.read(cx.borrow()).active_story()?;
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
        cx: &mut App,
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
        let group = self.stories[index].clone();
        self.workbench_state.update(cx, |state, cx| {
            state.set_active_story_by_key(group, story_key, cx);
        });
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
            StorybookAutomationCommand::OpenStory { key, response, .. } => {
                let result = self.set_active_story_by_key(&key, cx);
                cx.notify();
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
                    .or_else(|| self.active_story_snapshot(cx))
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
                    .or_else(|| self.active_story_snapshot(cx))
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
                        self.set_active_story_by_key(route, cx)?;
                    }
                    self.workbench_state
                        .update(cx, |state, cx| state.apply_controls(&request.controls, cx))?;
                    let story = self
                        .automation
                        .as_ref()
                        .and_then(|automation| automation.current_story().story)
                        .or_else(|| self.active_story_snapshot(cx))
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
                    Ok((story, steps, request.capture))
                })();

                match prepared {
                    Ok((story, steps, capture)) => {
                        if response.is_closed() {
                            return;
                        }
                        crate::automation::interaction::schedule_story_interaction(
                            crate::automation::interaction::PreparedStoryInteraction {
                                request_id,
                                story,
                                steps,
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
            .or_else(|| self.active_story_snapshot(cx))
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

        let left_sidebar_visible = self.left_sidebar_visible;
        let right_sidebar_visible = self.right_sidebar_visible;
        let left_sidebar_width = self.left_sidebar_width;
        let right_sidebar_width = self.right_sidebar_width;
        let gallery_for_left_bounds = cx.entity();
        let gallery_for_right_bounds = cx.entity();
        let story_sidebar =
            self.render_story_sidebar(&filtered_stories, ui_active_index_in_filtered_list, cx);

        h_resizable(format!(
            "gallery-container-{left_sidebar_visible}-{right_sidebar_visible}"
        ))
        .when(left_sidebar_visible, |this| {
            this.child(
                resizable_panel()
                    .size(left_sidebar_width)
                    .size_range(px(200.)..px(320.))
                    .flex_none()
                    .child(
                        div()
                            .size_full()
                            .debug_selector(|| "gallery-left-sidebar".to_owned())
                            .on_prepaint(move |bounds, _, cx| {
                                gallery_for_left_bounds.update(cx, |gallery, _| {
                                    gallery.set_left_sidebar_width(bounds.size.width);
                                });
                            })
                            .child(story_sidebar),
                    ),
            )
        })
        .child(
            resizable_panel().child(
                v_flex()
                    .flex_1()
                    .h_full()
                    .min_w_0()
                    .min_h_0()
                    .overflow_hidden()
                    .debug_selector(|| "gallery-main-content".to_owned())
                    .child(
                        h_flex()
                            .id("header")
                            .p_4()
                            .border_b_1()
                            .border_color(cx.theme().border)
                            .items_start()
                            .child(
                                h_flex().items_start().gap_3().child(
                                    v_flex()
                                        .gap_1()
                                        .child(div().text_xl().child(story_name))
                                        .child(
                                            div()
                                                .text_color(cx.theme().muted_foreground)
                                                .child(description),
                                        ),
                                ),
                            ),
                    )
                    .child(
                        div()
                            .id("story")
                            .flex_1()
                            .min_w_0()
                            .min_h_0()
                            .overflow_hidden()
                            .when_some(active_story_to_render, |this, active_story_ref| {
                                this.child(active_story_ref)
                            }),
                    )
                    .into_any_element(),
            ),
        )
        .when(right_sidebar_visible, |this| {
            this.child(
                resizable_panel()
                    .size(right_sidebar_width)
                    .size_range(px(280.)..px(520.))
                    .flex_none()
                    .child(
                        div()
                            .size_full()
                            .debug_selector(|| "gallery-right-sidebar".to_owned())
                            .on_prepaint(move |bounds, _, cx| {
                                gallery_for_right_bounds.update(cx, |gallery, _| {
                                    gallery.set_right_sidebar_width(bounds.size.width);
                                });
                            })
                            .child(self.workbench.clone()),
                    ),
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::controls::{
        ControlBounds, ControlError, ControlKind, ControlSpec, ControlValue, StoryControls,
    };
    use crate::registry::{RegisteredStoryMetadata, StoryKey, StoryName};
    use crate::story::Story;
    use crate::storybook_window_ui::StorybookWindowUi;
    use gpui::{Focusable, Modifiers, MouseButton, TestAppContext, VisualTestContext, point};
    use tokio::sync::oneshot;

    struct ControlledStory {
        focus_handle: gpui::FocusHandle,
        enabled: bool,
    }

    impl StoryControls for ControlledStory {
        fn control_specs(&self) -> Vec<ControlSpec> {
            vec![ControlSpec {
                key: "enabled".to_owned(),
                label: "Enabled".to_owned(),
                description: String::new(),
                category: "Properties".to_owned(),
                kind: ControlKind::Checkbox,
                default: ControlValue::Boolean(false),
                bounds: ControlBounds::default(),
                options: Vec::new(),
            }]
        }

        fn control_value(&self, key: &str) -> Result<ControlValue, ControlError> {
            match key {
                "enabled" => Ok(ControlValue::Boolean(self.enabled)),
                _ => Err(ControlError::UnknownControl {
                    key: key.to_owned(),
                }),
            }
        }

        fn set_control_value(
            &mut self,
            key: &str,
            value: ControlValue,
        ) -> Result<(), ControlError> {
            match (key, value) {
                ("enabled", ControlValue::Boolean(value)) => {
                    self.enabled = value;
                    Ok(())
                },
                _ => Err(ControlError::UnknownControl {
                    key: key.to_owned(),
                }),
            }
        }
    }

    impl Focusable for ControlledStory {
        fn focus_handle(&self, _: &App) -> gpui::FocusHandle {
            self.focus_handle.clone()
        }
    }

    impl Render for ControlledStory {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div().child(self.enabled.to_string())
        }
    }

    impl Story for ControlledStory {
        fn title(_: &App) -> String {
            "Controlled".to_owned()
        }

        fn new_view(_: &mut Window, cx: &mut App) -> Entity<Self> {
            cx.new(|cx| Self {
                focus_handle: cx.focus_handle(),
                enabled: false,
            })
        }
    }

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
                "/tmp/crate",
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
                assert!(gallery.left_sidebar_visible);
                assert!(gallery.right_sidebar_visible);
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
                        _operation: automation
                            .begin_operation()
                            .expect("open operation should start"),
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

                let (response, mut result) = oneshot::channel();
                gallery.handle_automation_command(
                    StorybookAutomationCommand::RunSteps {
                        request_id: 8,
                        request: crate::automation::StoryInteractionRequest {
                            story_key: Some("crate-ButtonStory".to_owned()),
                            controls: BTreeMap::new(),
                            width: None,
                            height: None,
                            viewport: None,
                            steps: vec![crate::automation::StoryInteractionStep::DispatchAction {
                                name: "storybook_test::MissingAction".to_owned(),
                                args: None,
                            }],
                            capture: None,
                        },
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
                assert_eq!(
                    gallery
                        .active_story_snapshot(cx)
                        .expect("invalid batch must not change routes")
                        .key,
                    "crate-TableStory"
                );

                let (response, cancelled) = oneshot::channel();
                drop(cancelled);
                gallery.handle_automation_command(
                    StorybookAutomationCommand::RunSteps {
                        request_id: 9,
                        request: crate::automation::StoryInteractionRequest {
                            story_key: Some("crate-ButtonStory".to_owned()),
                            controls: BTreeMap::new(),
                            width: None,
                            height: None,
                            viewport: None,
                            steps: vec![crate::automation::StoryInteractionStep::FocusNext],
                            capture: None,
                        },
                        response,
                        progress: std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0)),
                        operation: automation
                            .begin_operation()
                            .expect("cancelled interaction operation should start"),
                    },
                    window,
                    cx,
                );
                assert_eq!(
                    gallery
                        .active_story_snapshot(cx)
                        .expect("cancelled batch must not change routes")
                        .key,
                    "crate-TableStory"
                );
                assert!(
                    automation.begin_operation().is_ok(),
                    "cancelled batch should release its guard"
                );

                gallery.stories.clear();
                gallery.active_index = None;
                gallery.workbench_state.update(cx, |state, cx| {
                    state.set_active_story(None, cx);
                });
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
                        operation: automation
                            .begin_operation()
                            .expect("capture operation should start"),
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

    #[test]
    fn sidebar_toggles_keep_responsive_canvas_centered_with_its_resize_gutter() {
        let mut app = TestAppContext::single();
        app.update(gpui_component::init);
        let (_, cx) = app.add_window_view(move |window, cx| {
            let gallery = cx.new(|gallery_cx| {
                let story = story(
                    "crate-ButtonStory",
                    "ButtonStory",
                    "Button",
                    window,
                    gallery_cx,
                );
                Gallery::new(vec![story], None, None, window, gallery_cx)
            });
            crate::story::StoryRoot::new(
                "Storybook",
                gallery,
                StorybookWindowUi::default(),
                window,
                cx,
            )
        });
        let draw = |cx: &mut VisualTestContext| {
            cx.run_until_parked();
            cx.update(|window, cx| {
                _ = window.draw(cx);
            });
            cx.run_until_parked();
            cx.update(|window, cx| {
                _ = window.draw(cx);
            });
        };
        let assert_canvas_centered_with_resize_gutter = |cx: &mut VisualTestContext| {
            let canvas = cx
                .debug_bounds("story-canvas")
                .expect("story canvas should render");
            let stage = cx
                .debug_bounds("story-canvas-stage")
                .expect("story canvas stage should render");
            let main_content = cx
                .debug_bounds("gallery-main-content")
                .expect("main content pane should render");
            assert_eq!(
                stage.left(),
                main_content.left(),
                "responsive stage {stage:?} must not have a left inset in {main_content:?}"
            );
            assert_eq!(
                stage.right(),
                main_content.right(),
                "responsive stage {stage:?} must not have a right inset in {main_content:?}"
            );
            assert_eq!(
                canvas.left() - stage.left(),
                stage.right() - canvas.right(),
                "responsive canvas {canvas:?} must have a symmetric gutter in {stage:?}"
            );
            assert!(
                canvas.left() > stage.left(),
                "responsive canvas {canvas:?} must keep its resize gutter in {stage:?}"
            );
            assert_eq!(
                canvas.center().x,
                main_content.center().x,
                "responsive canvas {canvas:?} must be centered in {main_content:?}"
            );
            if let Some(left_sidebar) = cx.debug_bounds("gallery-left-sidebar") {
                assert!(
                    canvas.left() >= left_sidebar.right(),
                    "canvas {canvas:?} must stay to the right of {left_sidebar:?}"
                );
            }
            if let Some(right_sidebar) = cx.debug_bounds("gallery-right-sidebar") {
                assert!(
                    canvas.right() <= right_sidebar.left(),
                    "canvas {canvas:?} must stay to the left of {right_sidebar:?}"
                );
            }
        };
        let click_toggle = |selector, cx: &mut VisualTestContext| {
            let bounds = cx
                .debug_bounds(selector)
                .expect("sidebar toggle should render");
            cx.simulate_click(bounds.center(), Modifiers::none());
            draw(cx);
        };

        draw(cx);
        assert!(
            cx.debug_bounds("workbench-viewport").is_some(),
            "viewport selector should remain in the workbench header"
        );
        assert_eq!(
            cx.debug_bounds("workbench-background"),
            None,
            "canvas background selector should not render in the workbench"
        );
        let left_toggle_bounds = cx
            .debug_bounds("gallery-toggle-left-sidebar")
            .expect("left sidebar toggle should render in the title bar");
        let right_toggle_bounds = cx
            .debug_bounds("gallery-toggle-right-sidebar")
            .expect("right sidebar toggle should render in the title bar");
        let settings_bounds = cx
            .debug_bounds("storybook-settings")
            .expect("settings should render in the title bar");
        assert!(left_toggle_bounds.right() <= right_toggle_bounds.left());
        assert!(right_toggle_bounds.right() <= settings_bounds.left());

        let left_width = cx
            .debug_bounds("gallery-left-sidebar")
            .expect("left sidebar should render")
            .size
            .width;
        let right_width = cx
            .debug_bounds("gallery-right-sidebar")
            .expect("right sidebar should render")
            .size
            .width;
        assert_canvas_centered_with_resize_gutter(cx);

        click_toggle("gallery-toggle-left-sidebar", cx);
        assert_eq!(cx.debug_bounds("gallery-left-sidebar"), None);
        assert!(cx.debug_bounds("gallery-right-sidebar").is_some());
        assert_canvas_centered_with_resize_gutter(cx);

        click_toggle("gallery-toggle-right-sidebar", cx);
        assert_eq!(cx.debug_bounds("gallery-left-sidebar"), None);
        assert_eq!(cx.debug_bounds("gallery-right-sidebar"), None);
        assert_canvas_centered_with_resize_gutter(cx);

        click_toggle("gallery-toggle-left-sidebar", cx);
        assert!(cx.debug_bounds("gallery-left-sidebar").is_some());
        assert_eq!(cx.debug_bounds("gallery-right-sidebar"), None);
        assert_canvas_centered_with_resize_gutter(cx);

        click_toggle("gallery-toggle-right-sidebar", cx);
        assert!(cx.debug_bounds("gallery-left-sidebar").is_some());
        assert!(cx.debug_bounds("gallery-right-sidebar").is_some());
        assert_canvas_centered_with_resize_gutter(cx);

        let left_bounds = cx
            .debug_bounds("gallery-left-sidebar")
            .expect("left sidebar should render before resizing");
        let resize_start = point(left_bounds.right(), left_bounds.center().y);
        cx.simulate_mouse_move(resize_start, None, Modifiers::none());
        cx.simulate_mouse_down(resize_start, MouseButton::Left, Modifiers::none());
        cx.simulate_mouse_move(
            point(resize_start.x + px(5.), resize_start.y),
            MouseButton::Left,
            Modifiers::none(),
        );
        cx.simulate_mouse_move(
            point(resize_start.x + px(30.), resize_start.y),
            MouseButton::Left,
            Modifiers::none(),
        );
        cx.simulate_mouse_up(
            point(resize_start.x + px(30.), resize_start.y),
            MouseButton::Left,
            Modifiers::none(),
        );
        draw(cx);
        let resized_left_width = cx
            .debug_bounds("gallery-left-sidebar")
            .expect("left sidebar should render after resizing")
            .size
            .width;
        assert!(resized_left_width > left_width);
        assert_canvas_centered_with_resize_gutter(cx);

        let right_bounds = cx
            .debug_bounds("gallery-right-sidebar")
            .expect("right sidebar should render before resizing");
        let resize_start = point(right_bounds.left(), right_bounds.center().y);
        cx.simulate_mouse_move(resize_start, None, Modifiers::none());
        cx.simulate_mouse_down(resize_start, MouseButton::Left, Modifiers::none());
        cx.simulate_mouse_move(
            point(resize_start.x - px(5.), resize_start.y),
            MouseButton::Left,
            Modifiers::none(),
        );
        cx.simulate_mouse_move(
            point(resize_start.x - px(30.), resize_start.y),
            MouseButton::Left,
            Modifiers::none(),
        );
        cx.simulate_mouse_up(
            point(resize_start.x - px(30.), resize_start.y),
            MouseButton::Left,
            Modifiers::none(),
        );
        draw(cx);
        let resized_right_width = cx
            .debug_bounds("gallery-right-sidebar")
            .expect("right sidebar should render after resizing")
            .size
            .width;
        assert!(resized_right_width > right_width);
        assert_canvas_centered_with_resize_gutter(cx);
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

    #[gpui::test]
    fn automation_controls_read_set_and_reset_the_live_entity(cx: &mut App) {
        gpui_component::init(cx);
        let automation = crate::automation::StorybookAutomation::new();
        let automation_for_view = automation.clone();
        let window: gpui::WindowHandle<Gallery> = cx
            .open_window(Default::default(), move |window, cx| {
                let story = StoryContainer::panel::<ControlledStory>(window, cx);
                story.update(cx, |story, _| {
                    story.set_registration_metadata(RegisteredStoryMetadata::new(
                        StoryKey::new("crate-ControlledStory"),
                        StoryName::new("ControlledStory"),
                        None,
                        "crate",
                        "/tmp/crate",
                        "src/controlled.rs",
                        1,
                    ));
                });
                Gallery::view_with_automation(vec![story], None, automation_for_view, window, cx)
            })
            .expect("gallery window should open");

        window
            .update(cx, |gallery, window, cx| {
                let (response, mut result) = oneshot::channel();
                gallery.handle_automation_command(
                    StorybookAutomationCommand::ReadControls { response },
                    window,
                    cx,
                );
                let snapshot = result
                    .try_recv()
                    .expect("read response is sent")
                    .expect("controls are available");
                assert_eq!(snapshot.controls[0].value, ControlValue::Boolean(false));

                let (response, mut result) = oneshot::channel();
                gallery.handle_automation_command(
                    StorybookAutomationCommand::SetControl {
                        key: "enabled".to_owned(),
                        value: ControlValue::Boolean(true),
                        response,
                        _operation: automation
                            .begin_operation()
                            .expect("control operation should start"),
                    },
                    window,
                    cx,
                );
                let snapshot = result
                    .try_recv()
                    .expect("set response is sent")
                    .expect("control update succeeds");
                assert_eq!(snapshot.controls[0].value, ControlValue::Boolean(true));

                let (response, mut result) = oneshot::channel();
                gallery.handle_automation_command(
                    StorybookAutomationCommand::ResetControl {
                        key: None,
                        response,
                        _operation: automation
                            .begin_operation()
                            .expect("reset operation should start"),
                    },
                    window,
                    cx,
                );
                let snapshot = result
                    .try_recv()
                    .expect("reset response is sent")
                    .expect("control reset succeeds");
                assert_eq!(snapshot.controls[0].value, ControlValue::Boolean(false));
            })
            .expect("gallery should update");
    }

    #[gpui::test]
    fn grouped_route_selects_the_exact_workbench_variant(cx: &mut App) {
        gpui_component::init(cx);
        let automation = crate::automation::StorybookAutomation::new();
        let window: gpui::WindowHandle<Gallery> = cx
            .open_window(Default::default(), move |window, cx| {
                let primary = story(
                    "crate-PrimaryButtonStory",
                    "PrimaryButtonStory",
                    "Button",
                    window,
                    cx,
                );
                let danger = story(
                    "crate-DangerButtonStory",
                    "DangerButtonStory",
                    "Button",
                    window,
                    cx,
                );
                let grouped =
                    StoryContainer::list_panel("Button", vec![primary, danger], window, cx);
                Gallery::view_with_automation(vec![grouped], None, automation, window, cx)
            })
            .expect("grouped gallery window should open");

        window
            .update(cx, |gallery, _, cx| {
                gallery
                    .set_active_story_by_key("crate-DangerButtonStory", cx)
                    .expect("member route should select its group");
                let active = gallery
                    .workbench_state
                    .read(cx)
                    .active_story()
                    .expect("active member exists");
                assert_eq!(
                    active.read(cx).story_key_label(),
                    Some("crate-DangerButtonStory")
                );
            })
            .expect("grouped gallery should update");
    }

    #[gpui::test]
    fn separate_windows_keep_control_entities_independent(cx: &mut App) {
        gpui_component::init(cx);
        let open = |cx: &mut App| {
            cx.open_window(Default::default(), |window, cx| {
                let story = StoryContainer::panel::<ControlledStory>(window, cx);
                Gallery::view(vec![story], None, window, cx)
            })
            .expect("gallery window should open")
        };
        let first: gpui::WindowHandle<Gallery> = open(cx);
        let second: gpui::WindowHandle<Gallery> = open(cx);

        first
            .update(cx, |gallery, _, cx| {
                let story = gallery
                    .workbench_state
                    .read(cx)
                    .active_story()
                    .expect("first story is active");
                story
                    .read(cx)
                    .control_target()
                    .expect("first story has controls")
                    .set("enabled", ControlValue::Boolean(true), cx)
                    .expect("first control update succeeds");
            })
            .expect("first gallery should update");

        second
            .update(cx, |gallery, _, cx| {
                let story = gallery
                    .workbench_state
                    .read(cx)
                    .active_story()
                    .expect("second story is active");
                assert_eq!(
                    story
                        .read(cx)
                        .control_target()
                        .expect("second story has controls")
                        .value("enabled", cx),
                    Ok(ControlValue::Boolean(false))
                );
            })
            .expect("second gallery should update");
    }
}
