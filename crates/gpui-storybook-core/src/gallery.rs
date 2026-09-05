use crate::{
    automation::{
        SharedStorybookAutomation, StoryCurrentSnapshot, StoryScreenshotRequest, StorySnapshot,
        StorybookAutomationCommand, StorybookAutomationCommandReceiver, StorybookAutomationError,
        default_storybook_automation, schedule_story_capture, set_capture_target_size,
        story_snapshots_from_containers, validate_capture_target_size,
    },
    capture_region::capture_route_story_key,
    story::StoryContainer,
    title_bar::sidebar_toggle_button,
    workbench::{StoryWorkbench, WorkbenchState, WorkbenchTab},
};
use gpui_kit::component::{
    ActiveTheme as _, ElementExt as _, Side, h_flex,
    input::{Input, InputEvent, InputState},
    resizable::{h_resizable, resizable_panel},
    sidebar::{Sidebar, SidebarGroup, SidebarMenu, SidebarMenuItem},
    v_flex,
};
use gpui_kit::prelude::{
    Context, FluentBuilder as _, InteractiveElement as _, IntoElement, ParentElement as _, Render,
    Styled as _,
};
use gpui_kit::{
    AnyElement, App, AppContext as _, ClickEvent, Entity, Pixels, SharedString, Subscription,
    Window, div, px, relative,
};
use std::{
    borrow::Borrow,
    collections::{BTreeMap, BTreeSet},
};

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
        Self::new_with_host(
            initial_stories,
            init_story_name,
            automation,
            true,
            window,
            cx,
        )
    }

    pub(crate) fn new_without_automation_host(
        initial_stories: Vec<Entity<StoryContainer>>,
        init_story_name: Option<&str>,
        automation: Option<SharedStorybookAutomation>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        Self::new_with_host(
            initial_stories,
            init_story_name,
            automation,
            false,
            window,
            cx,
        )
    }

    fn new_with_host(
        initial_stories: Vec<Entity<StoryContainer>>,
        init_story_name: Option<&str>,
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
        let search_input =
            cx.new(|cx_input| InputState::new(window, cx_input).placeholder("Search..."));
        let workbench_state =
            cx.new(|cx| WorkbenchState::new_with_automation(None, automation.clone(), cx));
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
                if let Some(automation) = &this.automation
                    && let Some(story) = state.read(cx).active_story()
                    && let Some(key) = story.read(cx).story_key_label()
                {
                    let _ = automation.confirm_current_story(key);
                }
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
        if let Some(command_receiver) = command_receiver {
            this.attach_automation_host(command_receiver, window, cx);
        }

        this
    }
}

mod automation;
mod capture;
mod layout;
mod selection;
mod view;

#[cfg(test)]
mod tests;
