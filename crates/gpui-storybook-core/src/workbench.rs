//! Window-scoped Storybook workbench state and UI.

use crate::{
    automation::{
        StoryControlsSnapshot, StoryScenarioRunSnapshot, StorySnapshot, StorybookAutomationError,
        default_storybook_automation,
    },
    controls::{ControlKind, ControlSpec, ControlTarget, ControlValue},
    presentation::{StoryCanvasBackground, StoryPresentation, StoryViewportPreset},
    story::{StoryContainer, StoryScenario},
    theme_workbench::ThemeDraft,
};
use gpui::{
    Action, AnyElement, App, AppContext as _, ClipboardItem, Context, Entity, EntityId,
    EventEmitter, FocusHandle, Focusable, InteractiveElement as _, IntoElement, KeyBinding,
    ParentElement as _, Pixels, Render, SharedString, Size, Styled as _, Subscription, Window, div,
    prelude::FluentBuilder as _, px, size,
};
use gpui_component::{
    ActiveTheme as _, Disableable as _, Selectable as _, Sizable as _,
    button::{Button, ButtonVariants as _},
    checkbox::Checkbox,
    clipboard::Clipboard,
    color_picker::{ColorPicker, ColorPickerEvent, ColorPickerState},
    dock::{BasePanel, Panel, PanelControl, PanelEvent, PanelInfo, PanelState},
    h_flex,
    input::{Input, InputEvent, InputState, NumberInput},
    link::Link,
    menu::DropdownMenu as _,
    scroll::ScrollableElement as _,
    slider::{Slider, SliderEvent, SliderState},
    tab::{Tab, TabBar},
    v_flex,
};
use serde::{Deserialize, Serialize};
#[cfg(not(target_family = "wasm"))]
use std::path::Path;
use std::{collections::BTreeMap, rc::Rc};

/// Tabs available in the Storybook workbench.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkbenchTab {
    #[default]
    Controls,
    Theme,
    Inspect,
    Actions,
    Scenarios,
    #[cfg(feature = "performance")]
    Performance,
}

impl WorkbenchTab {
    fn index(self) -> usize {
        match self {
            Self::Controls => 0,
            Self::Theme => 1,
            Self::Inspect => 2,
            Self::Actions => 3,
            Self::Scenarios => 4,
            #[cfg(feature = "performance")]
            Self::Performance => 5,
        }
    }

    fn from_index(index: usize) -> Self {
        match index {
            1 => Self::Theme,
            2 => Self::Inspect,
            3 => Self::Actions,
            4 => Self::Scenarios,
            #[cfg(feature = "performance")]
            5 => Self::Performance,
            _ => Self::Controls,
        }
    }
}

/// Per-window active story and variant selection.
pub struct WorkbenchState {
    active_group: Option<Entity<StoryContainer>>,
    active_story: Option<Entity<StoryContainer>>,
    presentation: StoryPresentation,
    responsive_size: Option<Size<Pixels>>,
}

impl WorkbenchState {
    pub fn new(initial_story: Option<Entity<StoryContainer>>) -> Self {
        let mut state = Self {
            active_group: None,
            active_story: None,
            presentation: StoryPresentation::default(),
            responsive_size: None,
        };
        if let Some(story) = initial_story {
            state.active_group = Some(story.clone());
            state.active_story = Some(story);
        }
        state
    }

    /// Select a gallery or dock story, choosing its first variant when grouped.
    pub fn set_active_story(
        &mut self,
        story: Option<Entity<StoryContainer>>,
        cx: &mut Context<Self>,
    ) {
        self.active_group = story.clone();
        self.active_story =
            story.and_then(|story| story.read(cx).list_members.first().cloned().or(Some(story)));
        self.apply_presentation(cx);
        cx.notify();
    }

    /// Select a story group and the exact registered member matching `key`.
    pub fn set_active_story_by_key(
        &mut self,
        story: Entity<StoryContainer>,
        key: &str,
        cx: &mut Context<Self>,
    ) {
        fn find(
            story: &Entity<StoryContainer>,
            key: &str,
            cx: &App,
        ) -> Option<Entity<StoryContainer>> {
            let (matches, members) = {
                let story = story.read(cx);
                (
                    story
                        .story_key_label()
                        .is_some_and(|candidate| candidate == key),
                    story.list_members.clone(),
                )
            };
            if matches {
                return Some(story.clone());
            }
            members.iter().find_map(|member| find(member, key, cx))
        }

        self.active_group = Some(story.clone());
        self.active_story = find(&story, key, cx)
            .or_else(|| story.read(cx).list_members.first().cloned().or(Some(story)));
        self.apply_presentation(cx);
        cx.notify();
    }

    /// Select one member of the active grouped story.
    pub fn set_active_variant(&mut self, story: Entity<StoryContainer>, cx: &mut Context<Self>) {
        let belongs_to_group = self.active_group.as_ref().is_some_and(|group| {
            group
                .read(cx)
                .list_members
                .iter()
                .any(|member| member == &story)
        });
        if belongs_to_group || self.active_group.as_ref() == Some(&story) {
            self.active_story = Some(story);
            self.apply_presentation(cx);
            cx.notify();
        }
    }

    pub fn active_story(&self) -> Option<Entity<StoryContainer>> {
        self.active_story.clone()
    }

    pub fn active_group(&self) -> Option<Entity<StoryContainer>> {
        self.active_group.clone()
    }

    pub fn variants(&self, cx: &App) -> Vec<Entity<StoryContainer>> {
        self.active_group
            .as_ref()
            .map(|group| group.read(cx).list_members.clone())
            .unwrap_or_default()
    }

    pub fn presentation(&self) -> StoryPresentation {
        self.presentation
    }

    #[cfg(test)]
    pub(crate) fn responsive_size(&self) -> Option<Size<Pixels>> {
        self.responsive_size
    }

    pub fn set_viewport(&mut self, viewport: StoryViewportPreset, cx: &mut Context<Self>) {
        if viewport == StoryViewportPreset::Responsive
            && self.presentation.viewport != StoryViewportPreset::Responsive
        {
            self.responsive_size = self
                .presentation
                .viewport
                .dimensions()
                .map(|(width, height)| size(px(width as f32), px(height as f32)));
        }
        self.presentation.viewport = viewport;
        self.apply_presentation(cx);
        cx.notify();
    }

    pub fn set_background(&mut self, background: StoryCanvasBackground, cx: &mut Context<Self>) {
        self.presentation.background = background;
        self.apply_presentation(cx);
        cx.notify();
    }

    pub(crate) fn set_responsive_size(
        &mut self,
        responsive_size: Size<Pixels>,
        cx: &mut Context<Self>,
    ) {
        if self.presentation.viewport != StoryViewportPreset::Responsive {
            return;
        }
        self.responsive_size = Some(responsive_size);
        cx.notify();
    }

    fn apply_presentation(&self, cx: &mut Context<Self>) {
        if let Some(story) = &self.active_story {
            let workbench_state = cx.entity().downgrade();
            story.update(cx, |story, cx| {
                story.set_presentation(self.presentation);
                story.set_responsive_size(self.responsive_size);
                story.set_workbench_state(workbench_state);
                cx.notify();
            });
        }
    }

    pub(crate) fn controls_snapshot(
        &self,
        cx: &App,
    ) -> Result<StoryControlsSnapshot, StorybookAutomationError> {
        let story = self
            .active_story()
            .ok_or(StorybookAutomationError::NoActiveStory)?;
        let snapshot = StorySnapshot::from_container(story.read(cx), cx)
            .ok_or(StorybookAutomationError::NoActiveStory)?;
        let target = story.read(cx).control_target().ok_or_else(|| {
            StorybookAutomationError::ControlsUnavailable {
                key: snapshot.key.clone(),
            }
        })?;
        let controls = target.snapshots(cx).map_err(|error| {
            StorybookAutomationError::ControlOperationFailed {
                message: error.to_string(),
            }
        })?;
        Ok(StoryControlsSnapshot {
            story: snapshot,
            controls,
        })
    }

    pub(crate) fn set_control(
        &mut self,
        key: &str,
        value: ControlValue,
        cx: &mut Context<Self>,
    ) -> Result<StoryControlsSnapshot, StorybookAutomationError> {
        let story = self
            .active_story()
            .ok_or(StorybookAutomationError::NoActiveStory)?;
        let target = story.read(cx).control_target().ok_or_else(|| {
            StorybookAutomationError::ControlsUnavailable {
                key: story
                    .read(cx)
                    .story_key_label()
                    .unwrap_or_default()
                    .to_owned(),
            }
        })?;
        target.set(key, value, cx).map_err(|error| {
            StorybookAutomationError::ControlOperationFailed {
                message: error.to_string(),
            }
        })?;
        cx.notify();
        self.controls_snapshot(cx)
    }

    pub(crate) fn reset_control(
        &mut self,
        key: Option<&str>,
        cx: &mut Context<Self>,
    ) -> Result<StoryControlsSnapshot, StorybookAutomationError> {
        let story = self
            .active_story()
            .ok_or(StorybookAutomationError::NoActiveStory)?;
        let target = story.read(cx).control_target().ok_or_else(|| {
            StorybookAutomationError::ControlsUnavailable {
                key: story
                    .read(cx)
                    .story_key_label()
                    .unwrap_or_default()
                    .to_owned(),
            }
        })?;
        let result = if let Some(key) = key {
            target.reset(key, cx)
        } else {
            target.reset_all(cx)
        };
        result.map_err(|error| StorybookAutomationError::ControlOperationFailed {
            message: error.to_string(),
        })?;
        cx.notify();
        self.controls_snapshot(cx)
    }

    pub(crate) fn apply_controls(
        &mut self,
        controls: &BTreeMap<String, ControlValue>,
        cx: &mut Context<Self>,
    ) -> Result<(), StorybookAutomationError> {
        for (key, value) in controls {
            self.set_control(key, value.clone(), cx)?;
        }
        Ok(())
    }
}

#[derive(Action, Clone, Eq, PartialEq)]
#[action(namespace = storybook_workbench, no_json)]
struct SelectControlOption {
    key: String,
    value: String,
}

#[derive(Action, Clone, Copy, Eq, PartialEq)]
#[action(namespace = storybook_workbench, no_json)]
struct SelectViewport {
    viewport: StoryViewportPreset,
}

enum ControlEditor {
    Text(Entity<InputState>),
    Number {
        state: Entity<InputState>,
        integer: bool,
    },
    Range {
        state: Entity<SliderState>,
        integer: bool,
    },
    Color(Entity<ColorPickerState>),
}

#[cfg(not(target_family = "wasm"))]
fn story_source_url(crate_dir: &str, source_file: &str) -> Option<String> {
    let source_file = Path::new(source_file);
    let source_path = if source_file.is_absolute() {
        source_file.is_file().then(|| source_file.to_path_buf())
    } else {
        Path::new(crate_dir)
            .ancestors()
            .map(|directory| directory.join(source_file))
            .find(|candidate| candidate.is_file())
    }?;

    url::Url::from_file_path(source_path).ok().map(Into::into)
}

#[cfg(target_family = "wasm")]
fn story_source_url(_: &str, _: &str) -> Option<String> {
    None
}

fn format_key_binding(binding: &KeyBinding) -> String {
    binding
        .keystrokes()
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(" ")
}

fn is_story_scoped_action(
    action: &dyn Action,
    action_scope_focus: &FocusHandle,
    storybook_focus: &FocusHandle,
    window: &Window,
) -> bool {
    // Both handles share the Storybook shell path. The story scope handle is an
    // explicit root scope rather than the story's primary (possibly nested)
    // interaction focus, so child-control actions never enter this set.
    window.is_action_available_in(action, action_scope_focus)
        && !window.is_action_available_in(action, storybook_focus)
}

fn story_scoped_actions(
    action_scope_focus: &FocusHandle,
    storybook_focus: &FocusHandle,
    window: &Window,
    cx: &App,
) -> Vec<Box<dyn Action>> {
    let mut actions = cx
        .all_action_names()
        .iter()
        .filter_map(|name| cx.build_action(name, None).ok())
        .filter(|action| {
            is_story_scoped_action(action.as_ref(), action_scope_focus, storybook_focus, window)
        })
        .collect::<Vec<_>>();
    actions.sort_by_key(|action| action.name());
    actions
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
struct StoryWorkbenchPanelState {
    selected_tab: WorkbenchTab,
}

enum ScenarioRunState {
    Running {
        story_key: String,
        scenario: StoryScenario,
    },
    Finished {
        story_key: String,
        scenario: StoryScenario,
        result: Box<Result<StoryScenarioRunSnapshot, StorybookAutomationError>>,
    },
}

/// Right-side developer workbench for controls, themes, inspection, actions,
/// fresh story scenarios, and opt-in performance telemetry.
pub struct StoryWorkbench {
    focus_handle: FocusHandle,
    state: Entity<WorkbenchState>,
    selected_tab: WorkbenchTab,
    editor_story: Option<EntityId>,
    editors: BTreeMap<String, ControlEditor>,
    editor_subscriptions: Vec<Subscription>,
    _state_subscription: Subscription,
    theme_draft: ThemeDraft,
    theme_search: Entity<InputState>,
    theme_editors: BTreeMap<String, Entity<ColorPickerState>>,
    theme_subscriptions: Vec<Subscription>,
    scenario_run: Option<ScenarioRunState>,
    last_error: Option<SharedString>,
}

impl StoryWorkbench {
    pub fn new(
        state: Entity<WorkbenchState>,
        selected_tab: WorkbenchTab,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let state_subscription = cx.observe(&state, |_, _, cx| cx.notify());
        let theme_draft = ThemeDraft::new(cx.theme())
            .expect("the active GPUI Component theme must produce a workbench draft");
        let theme_search =
            cx.new(|cx| InputState::new(window, cx).placeholder("Search theme colors..."));
        let theme_search_subscription =
            cx.subscribe(&theme_search, |_, _, _: &InputEvent, cx| cx.notify());
        let mut this = Self {
            focus_handle: cx.focus_handle(),
            state,
            selected_tab,
            editor_story: None,
            editors: BTreeMap::new(),
            editor_subscriptions: Vec::new(),
            _state_subscription: state_subscription,
            theme_draft,
            theme_search,
            theme_editors: BTreeMap::new(),
            theme_subscriptions: vec![theme_search_subscription],
            scenario_run: None,
            last_error: None,
        };
        this.rebuild_control_editors(window, cx);
        this.rebuild_theme_editors(window, cx);
        this
    }

    #[cfg(feature = "dock")]
    pub(crate) fn selected_tab_from_panel(info: &PanelInfo) -> WorkbenchTab {
        let PanelInfo::Panel(value) = info else {
            return WorkbenchTab::default();
        };
        serde_json::from_value::<StoryWorkbenchPanelState>(value.clone())
            .unwrap_or_default()
            .selected_tab
    }

    fn active_story(&self, cx: &App) -> Option<Entity<StoryContainer>> {
        self.state.read(cx).active_story()
    }

    fn active_target(&self, cx: &App) -> Option<Rc<dyn ControlTarget>> {
        self.active_story(cx)?.read(cx).control_target()
    }

    fn editor_value(value: &ControlValue) -> String {
        match value {
            ControlValue::Boolean(value) => value.to_string(),
            ControlValue::Integer(value) => value.to_string(),
            ControlValue::Float(value) => value.to_string(),
            ControlValue::Text(value) | ControlValue::Choice(value) => value.clone(),
            ControlValue::Color(value) => format!(
                "hsla({:.3}, {:.3}, {:.3}, {:.3})",
                value.h, value.s, value.l, value.a
            ),
            ControlValue::Json(value) => value.to_string(),
        }
    }

    fn rebuild_control_editors(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.editors.clear();
        self.editor_subscriptions.clear();
        self.last_error = None;

        let Some(story) = self.active_story(cx) else {
            self.editor_story = None;
            return;
        };
        self.editor_story = Some(story.entity_id());
        let Some(target) = story.read(cx).control_target() else {
            return;
        };

        for spec in target.specs() {
            let key = spec.key.clone();
            let Ok(value) = target.value(&key, cx) else {
                continue;
            };
            let editor = match spec.kind {
                ControlKind::Text | ControlKind::Custom(_) => {
                    let state = cx.new(|cx| {
                        InputState::new(window, cx).default_value(Self::editor_value(&value))
                    });
                    let target = target.clone();
                    let control_key = key.clone();
                    let json = matches!(value, ControlValue::Json(_));
                    self.editor_subscriptions.push(cx.subscribe_in(
                        &state,
                        window,
                        move |this, state, event: &InputEvent, _, cx| {
                            if !matches!(event, InputEvent::Change) {
                                return;
                            }
                            let text = state.read(cx).value().to_string();
                            let value = if json {
                                serde_json::from_str(&text).map(ControlValue::Json).map_err(
                                    |error| format!("invalid JSON for `{control_key}`: {error}"),
                                )
                            } else {
                                Ok(ControlValue::Text(text))
                            };
                            let result = value.and_then(|value| {
                                target
                                    .set(&control_key, value, cx)
                                    .map_err(|error| error.to_string())
                            });
                            this.last_error = result.err().map(Into::into);
                            cx.notify();
                        },
                    ));
                    ControlEditor::Text(state)
                },
                ControlKind::Number => {
                    let integer = matches!(value, ControlValue::Integer(_));
                    let input_value = Self::editor_value(&value);
                    let bounds = spec.bounds;
                    let state = cx.new(|cx| {
                        let mut input = InputState::new(window, cx).default_value(input_value);
                        if let Some(step) = bounds.step {
                            input = input.step(step);
                        }
                        if let Some(min) = bounds.min {
                            input = input.min(min);
                        }
                        if let Some(max) = bounds.max {
                            input = input.max(max);
                        }
                        input
                    });
                    let target = target.clone();
                    let control_key = key.clone();
                    self.editor_subscriptions.push(cx.subscribe_in(
                        &state,
                        window,
                        move |this, state, event: &InputEvent, _, cx| {
                            if !matches!(event, InputEvent::Change) {
                                return;
                            }
                            let text = state.read(cx).value();
                            let value: Result<ControlValue, String> = if integer {
                                text.parse::<i64>()
                                    .map(ControlValue::Integer)
                                    .map_err(|error| error.to_string())
                            } else {
                                text.parse::<f64>()
                                    .map(ControlValue::Float)
                                    .map_err(|error| error.to_string())
                            };
                            let result = value
                                .map_err(|error| {
                                    format!("invalid number for `{control_key}`: {error}")
                                })
                                .and_then(|value| {
                                    target
                                        .set(&control_key, value, cx)
                                        .map_err(|error| error.to_string())
                                });
                            this.last_error = result.err().map(Into::into);
                            cx.notify();
                        },
                    ));
                    ControlEditor::Number { state, integer }
                },
                ControlKind::Range => {
                    let integer = matches!(value, ControlValue::Integer(_));
                    let numeric = match value {
                        ControlValue::Integer(value) => value as f32,
                        ControlValue::Float(value) => value as f32,
                        _ => continue,
                    };
                    let state = cx.new(|_| {
                        SliderState::new()
                            .min(spec.bounds.min.unwrap_or(0.0) as f32)
                            .max(spec.bounds.max.unwrap_or(100.0) as f32)
                            .step(spec.bounds.step.unwrap_or(1.0) as f32)
                            .default_value(numeric)
                    });
                    let target = target.clone();
                    let control_key = key.clone();
                    self.editor_subscriptions.push(cx.subscribe_in(
                        &state,
                        window,
                        move |this, _, event: &SliderEvent, _, cx| {
                            let SliderEvent::Change(value) = event else {
                                return;
                            };
                            let value = if integer {
                                ControlValue::Integer(value.start().round() as i64)
                            } else {
                                ControlValue::Float(value.start() as f64)
                            };
                            let result = target.set(&control_key, value, cx);
                            this.last_error = result.err().map(|error| error.to_string().into());
                            cx.notify();
                        },
                    ));
                    ControlEditor::Range { state, integer }
                },
                ControlKind::ColorPicker => {
                    let ControlValue::Color(color) = value else {
                        continue;
                    };
                    let state = cx.new(|cx| {
                        ColorPickerState::new(window, cx).default_value(gpui::Hsla::from(color))
                    });
                    let target = target.clone();
                    let control_key = key.clone();
                    self.editor_subscriptions.push(cx.subscribe_in(
                        &state,
                        window,
                        move |this, _, event: &ColorPickerEvent, _, cx| {
                            let ColorPickerEvent::Change(Some(color)) = event else {
                                return;
                            };
                            let result =
                                target.set(&control_key, ControlValue::Color((*color).into()), cx);
                            this.last_error = result.err().map(|error| error.to_string().into());
                            cx.notify();
                        },
                    ));
                    ControlEditor::Color(state)
                },
                ControlKind::Checkbox | ControlKind::Select => continue,
            };
            self.editors.insert(key, editor);
        }
    }

    fn sync_editor_values(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let active_story = self.active_story(cx).map(|story| story.entity_id());
        if active_story != self.editor_story {
            self.rebuild_control_editors(window, cx);
            return;
        }

        let Some(target) = self.active_target(cx) else {
            return;
        };
        for (key, editor) in &self.editors {
            let Ok(value) = target.value(key, cx) else {
                continue;
            };
            match editor {
                ControlEditor::Text(state) | ControlEditor::Number { state, .. } => {
                    let expected = Self::editor_value(&value);
                    if state.read(cx).value().as_ref() != expected {
                        state.update(cx, |state, cx| state.set_value(expected, window, cx));
                    }
                },
                ControlEditor::Range { state, .. } => {
                    let expected = match value {
                        ControlValue::Integer(value) => value as f32,
                        ControlValue::Float(value) => value as f32,
                        _ => continue,
                    };
                    if state.read(cx).value().start() != expected {
                        state.update(cx, |state, cx| state.set_value(expected, window, cx));
                    }
                },
                ControlEditor::Color(state) => {
                    let ControlValue::Color(color) = value else {
                        continue;
                    };
                    let expected = gpui::Hsla::from(color);
                    if state.read(cx).value() != Some(expected) {
                        state.update(cx, |state, cx| state.set_value(expected, window, cx));
                    }
                },
            }
        }
    }

    fn rebuild_theme_editors(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.theme_editors.clear();
        self.theme_subscriptions.truncate(1);

        let rows = match self.theme_draft.rows() {
            Ok(rows) => rows,
            Err(error) => {
                self.last_error = Some(error.to_string().into());
                return;
            },
        };
        for row in rows {
            let name = row.name.clone();
            let state = cx.new(|cx| ColorPickerState::new(window, cx).default_value(row.color));
            self.theme_subscriptions.push(cx.subscribe_in(
                &state,
                window,
                move |this, _, event: &ColorPickerEvent, _, cx| {
                    let ColorPickerEvent::Change(Some(color)) = event else {
                        return;
                    };
                    let result = this.theme_draft.set_color(&name, *color, cx);
                    this.last_error = result.err().map(|error| error.to_string().into());
                    cx.notify();
                },
            ));
            self.theme_editors.insert(row.name, state);
        }
    }

    fn sync_theme_editors(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match self.theme_draft.sync_from_global(cx) {
            Ok(_) => {},
            Err(error) => {
                self.last_error = Some(error.to_string().into());
                return;
            },
        }

        let rows = match self.theme_draft.rows() {
            Ok(rows) => rows,
            Err(error) => {
                self.last_error = Some(error.to_string().into());
                return;
            },
        };
        if rows.len() != self.theme_editors.len() {
            self.rebuild_theme_editors(window, cx);
            return;
        }
        for row in rows {
            let Some(editor) = self.theme_editors.get(&row.name) else {
                self.rebuild_theme_editors(window, cx);
                return;
            };
            if editor.read(cx).value() != Some(row.color) {
                editor.update(cx, |editor, cx| {
                    editor.set_value(row.color, window, cx);
                });
            }
        }
    }

    fn select_option(
        &mut self,
        action: &SelectControlOption,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let result = self
            .active_target(cx)
            .ok_or_else(|| "no active control target".to_owned())
            .and_then(|target| {
                target
                    .set(&action.key, ControlValue::Choice(action.value.clone()), cx)
                    .map_err(|error| error.to_string())
            });
        self.last_error = result.err().map(Into::into);
        cx.notify();
    }

    fn select_viewport(&mut self, action: &SelectViewport, _: &mut Window, cx: &mut Context<Self>) {
        self.state.update(cx, |state, cx| {
            state.set_viewport(action.viewport, cx);
        });
    }

    fn render_control(
        &self,
        spec: &ControlSpec,
        target: Rc<dyn ControlTarget>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let value = target
            .value(&spec.key, cx)
            .unwrap_or_else(|_| spec.default.clone());
        let key = spec.key.clone();
        let editor = match spec.kind {
            ControlKind::Checkbox => {
                let checked = matches!(value, ControlValue::Boolean(true));
                let target = target.clone();
                let control_key = key.clone();
                Checkbox::new(format!("workbench-control-{key}"))
                    .checked(checked)
                    .on_click(move |checked, _, cx| {
                        let _ = target.set(&control_key, ControlValue::Boolean(*checked), cx);
                    })
                    .into_any_element()
            },
            ControlKind::Select => {
                let current = Self::editor_value(&value);
                let options = spec.options.clone();
                let key_for_menu = key.clone();
                Button::new(format!("workbench-select-{key}"))
                    .label(current.clone())
                    .small()
                    .dropdown_menu(move |menu, _, _| {
                        options.iter().fold(menu, |menu, option| {
                            menu.menu_with_check(
                                option.clone(),
                                option == &current,
                                Box::new(SelectControlOption {
                                    key: key_for_menu.clone(),
                                    value: option.clone(),
                                }),
                            )
                        })
                    })
                    .into_any_element()
            },
            ControlKind::Text | ControlKind::Custom(_) => self
                .editors
                .get(&key)
                .and_then(|editor| match editor {
                    ControlEditor::Text(state) => {
                        Some(Input::new(state).small().into_any_element())
                    },
                    _ => None,
                })
                .unwrap_or_else(|| div().child(Self::editor_value(&value)).into_any_element()),
            ControlKind::Number => self
                .editors
                .get(&key)
                .and_then(|editor| match editor {
                    ControlEditor::Number { state, integer } => Some(
                        NumberInput::new(state)
                            .placeholder(if *integer { "0" } else { "0.0" })
                            .small()
                            .into_any_element(),
                    ),
                    _ => None,
                })
                .unwrap_or_else(|| div().child(Self::editor_value(&value)).into_any_element()),
            ControlKind::Range => self
                .editors
                .get(&key)
                .and_then(|editor| match editor {
                    ControlEditor::Range { state, integer } => Some(
                        h_flex()
                            .gap_2()
                            .child(Slider::new(state).flex_1())
                            .child(if *integer {
                                format!("{:.0}", state.read(cx).value().start())
                            } else {
                                format!("{:.2}", state.read(cx).value().start())
                            })
                            .into_any_element(),
                    ),
                    _ => None,
                })
                .unwrap_or_else(|| div().child(Self::editor_value(&value)).into_any_element()),
            ControlKind::ColorPicker => self
                .editors
                .get(&key)
                .and_then(|editor| match editor {
                    ControlEditor::Color(state) => {
                        Some(ColorPicker::new(state).small().into_any_element())
                    },
                    _ => None,
                })
                .unwrap_or_else(|| div().child(Self::editor_value(&value)).into_any_element()),
        };

        let target_for_reset = target;
        let key_for_reset = key.clone();
        v_flex()
            .id(format!("workbench-control-row-{key}"))
            .gap_2()
            .py_3()
            .border_b_1()
            .border_color(cx.theme().border)
            .child(
                h_flex()
                    .justify_between()
                    .gap_2()
                    .child(
                        v_flex()
                            .gap_0p5()
                            .child(div().text_sm().child(spec.label.clone()))
                            .when(!spec.description.is_empty(), |this| {
                                this.child(
                                    div()
                                        .text_xs()
                                        .text_color(cx.theme().muted_foreground)
                                        .child(spec.description.clone()),
                                )
                            }),
                    )
                    .child(
                        Button::new(format!("reset-control-{key}"))
                            .label("Reset")
                            .xsmall()
                            .ghost()
                            .on_click(cx.listener(move |this, _, _, cx| {
                                let result = target_for_reset.reset(&key_for_reset, cx);
                                this.last_error =
                                    result.err().map(|error| error.to_string().into());
                                cx.notify();
                            })),
                    ),
            )
            .child(editor)
            .into_any_element()
    }

    fn render_controls(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let Some(target) = self.active_target(cx) else {
            return v_flex()
                .p_4()
                .text_color(cx.theme().muted_foreground)
                .child("This story has no controls.")
                .into_any_element();
        };
        let specs = target.specs().to_vec();
        let target_for_reset = target.clone();

        v_flex()
            .id("workbench-controls")
            .p_4()
            .gap_1()
            .child(
                h_flex().justify_between().child("Story controls").child(
                    Button::new("reset-all-controls")
                        .label("Reset all")
                        .xsmall()
                        .on_click(cx.listener(move |this, _, _, cx| {
                            let result = target_for_reset.reset_all(cx);
                            this.last_error = result.err().map(|error| error.to_string().into());
                            cx.notify();
                        })),
                ),
            )
            .children(
                specs
                    .iter()
                    .map(|spec| self.render_control(spec, target.clone(), window, cx)),
            )
            .into_any_element()
    }

    fn render_theme(&self, cx: &mut Context<Self>) -> AnyElement {
        let query = self.theme_search.read(cx).value().to_lowercase();
        let rows = self.theme_draft.rows().unwrap_or_default();
        let base_theme_name = self.theme_draft.base_theme_name().to_owned();

        let header = v_flex()
            .debug_selector(|| "workbench-theme-sticky-header".to_owned())
            .flex_shrink_0()
            .p_4()
            .pb_3()
            .gap_3()
            .bg(cx.theme().background)
            .child(
                v_flex().gap_1().child("Session theme draft").child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(base_theme_name),
                ),
            )
            .child(Input::new(&self.theme_search).small().cleanable(true))
            .child(
                h_flex()
                    .gap_2()
                    .child(
                        Button::new("theme-export")
                            .label("Copy export")
                            .xsmall()
                            .on_click(cx.listener(|this, _, _, cx| {
                                match this.theme_draft.export_json() {
                                    Ok(json) => {
                                        cx.write_to_clipboard(ClipboardItem::new_string(json));
                                        this.last_error = None;
                                    },
                                    Err(error) => {
                                        this.last_error = Some(error.to_string().into());
                                    },
                                }
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("theme-import")
                            .label("Import clipboard")
                            .xsmall()
                            .on_click(cx.listener(|this, _, _, cx| {
                                let result = cx
                                    .read_from_clipboard()
                                    .and_then(|clipboard| clipboard.text())
                                    .ok_or_else(|| "clipboard does not contain text".to_owned())
                                    .and_then(|json| {
                                        this.theme_draft
                                            .import_json(&json, cx)
                                            .map_err(|error| error.to_string())
                                    });
                                this.last_error = result.err().map(Into::into);
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("theme-reset-all")
                            .label("Reset all")
                            .xsmall()
                            .on_click(cx.listener(|this, _, _, cx| {
                                let result = this.theme_draft.reset_all(cx);
                                this.last_error =
                                    result.err().map(|error| error.to_string().into());
                                cx.notify();
                            })),
                    ),
            );
        let items = rows
            .into_iter()
            .filter(|row| query.is_empty() || row.name.to_lowercase().contains(&query))
            .enumerate()
            .filter_map(|(index, row)| {
                let editor = self.theme_editors.get(&row.name)?.clone();
                let name = row.name.clone();
                Some(
                    h_flex()
                        .id(format!("theme-color-row-{}", row.name))
                        .when(index == 0, |this| {
                            this.debug_selector(|| "workbench-theme-first-item".to_owned())
                        })
                        .justify_between()
                        .gap_2()
                        .py_2()
                        .border_b_1()
                        .border_color(cx.theme().border)
                        .child(div().text_xs().child(row.name))
                        .child(
                            h_flex()
                                .gap_1()
                                .child(ColorPicker::new(&editor).small())
                                .child(
                                    Button::new(format!("theme-reset-{name}"))
                                        .label("Reset")
                                        .xsmall()
                                        .ghost()
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            let result = this.theme_draft.reset_color(&name, cx);
                                            this.last_error =
                                                result.err().map(|error| error.to_string().into());
                                            cx.notify();
                                        })),
                                ),
                        ),
                )
            });

        v_flex()
            .id("workbench-theme")
            .size_full()
            .min_h_0()
            .overflow_hidden()
            .child(header)
            .child(
                div()
                    .debug_selector(|| "workbench-theme-items".to_owned())
                    .flex_1()
                    .min_h_0()
                    .overflow_hidden()
                    .child(
                        v_flex()
                            .size_full()
                            .overflow_y_scrollbar()
                            .px_4()
                            .pb_4()
                            .gap_3()
                            .children(items),
                    ),
            )
            .into_any_element()
    }

    fn render_inspect(&self, cx: &mut Context<Self>) -> AnyElement {
        let story = self.active_story(cx);
        let (key, source, source_url) = story
            .as_ref()
            .map(|story| {
                let story = story.read(cx);
                let key = story.story_key_label().unwrap_or("unregistered").to_owned();
                let source_file = story.source_file_label().unwrap_or("unknown source");
                let source = format!(
                    "{}:{}",
                    source_file,
                    story.source_line().unwrap_or_default()
                );
                let source_url = story
                    .registration_metadata()
                    .and_then(|metadata| story_source_url(metadata.crate_dir(), source_file));
                (key, source, source_url)
            })
            .unwrap_or_else(|| ("No active story".to_owned(), String::new(), None));

        let source = source_url
            .map(|url| {
                Link::new("open-story-source")
                    .href(url)
                    .child(source.clone())
                    .into_any_element()
            })
            .unwrap_or_else(|| div().child(source).into_any_element());

        let content = v_flex().p_4().gap_3();
        #[cfg(feature = "inspector")]
        let content = content.child(
            Button::new("open-gpui-inspector")
                .label("Open GPUI Inspector")
                .on_click(|_, window, cx| window.toggle_inspector(cx)),
        );

        content
            .child(
                v_flex()
                    .gap_1()
                    .child(
                        h_flex().justify_between().gap_2().child("Story key").child(
                            Clipboard::new("copy-story-key")
                                .value(key.clone())
                                .tooltip("Copy story key"),
                        ),
                    )
                    .child(key),
            )
            .child(v_flex().gap_1().child("Source").child(source))
            .into_any_element()
    }

    fn run_scenario(
        &mut self,
        story_key: String,
        scenario: StoryScenario,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(automation) = default_storybook_automation(cx) else {
            self.scenario_run = Some(ScenarioRunState::Finished {
                story_key,
                scenario,
                result: Box::new(Err(StorybookAutomationError::NoLiveHost)),
            });
            cx.notify();
            return;
        };

        let scenario_key = scenario.key.clone();
        self.scenario_run = Some(ScenarioRunState::Running {
            story_key: story_key.clone(),
            scenario,
        });
        cx.notify();

        cx.spawn_in(window, async move |this, cx| {
            let result = automation
                .run_scenario(Some(story_key.clone()), scenario_key)
                .await;
            _ = this.update_in(cx, |this, window, cx| {
                let scenario = match &this.scenario_run {
                    Some(ScenarioRunState::Running { scenario, .. }) => scenario.clone(),
                    Some(ScenarioRunState::Finished { scenario, .. }) => scenario.clone(),
                    None => return,
                };
                this.scenario_run = Some(ScenarioRunState::Finished {
                    story_key,
                    scenario,
                    result: Box::new(result),
                });
                this.rebuild_control_editors(window, cx);
                cx.notify();
            });
        })
        .detach();
    }

    fn scenario_progress(error: &StorybookAutomationError) -> usize {
        match error {
            StorybookAutomationError::HostDisconnected {
                steps_dispatched, ..
            }
            | StorybookAutomationError::InteractionFailed {
                steps_dispatched, ..
            } => *steps_dispatched,
            _ => 0,
        }
    }

    fn render_scenarios(&self, _window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let Some(story) = self.active_story(cx) else {
            return v_flex()
                .p_4()
                .text_color(cx.theme().muted_foreground)
                .child("Select a story to inspect its scenarios.")
                .into_any_element();
        };
        let story = story.read(cx);
        let story_key = story.story_key_label().unwrap_or_default().to_owned();
        let scenarios = story.scenarios().to_vec();

        if scenarios.is_empty() {
            return v_flex()
                .p_4()
                .text_color(cx.theme().muted_foreground)
                .child("This story has no declared scenarios.")
                .into_any_element();
        }

        let any_scenario_running =
            matches!(&self.scenario_run, Some(ScenarioRunState::Running { .. }));

        let scenario_rows = scenarios.into_iter().map(|scenario| {
            let run = self.scenario_run.as_ref().filter(|run| match run {
                ScenarioRunState::Running {
                    story_key: run_story_key,
                    scenario: run_scenario,
                }
                | ScenarioRunState::Finished {
                    story_key: run_story_key,
                    scenario: run_scenario,
                    ..
                } => run_story_key == &story_key && run_scenario.key == scenario.key,
            });
            let running = matches!(run, Some(ScenarioRunState::Running { .. }));
            let scenario_for_run = scenario.clone();
            let story_key_for_run = story_key.clone();

            let step_rows = scenario.steps.iter().enumerate().map(|(index, step)| {
                let status = match run {
                    Some(ScenarioRunState::Running { .. }) => {
                        if index == 0 {
                            "Running"
                        } else {
                            "Queued"
                        }
                    },
                    Some(ScenarioRunState::Finished { result, .. }) => match result.as_ref() {
                        Ok(result) if index < result.interaction.steps_dispatched => "Passed",
                        Ok(_) => "Not run",
                        Err(error) => {
                            let completed = Self::scenario_progress(error);
                            if index < completed {
                                "Passed"
                            } else if index == completed {
                                "Failed"
                            } else {
                                "Not run"
                            }
                        },
                    },
                    None => "Ready",
                };

                h_flex()
                    .justify_between()
                    .gap_2()
                    .child(
                        div()
                            .text_xs()
                            .child(format!("{}. {}", index + 1, step.name)),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(status),
                    )
            });

            v_flex()
                .id(format!("workbench-scenario-{}", scenario.key))
                .gap_2()
                .py_3()
                .border_b_1()
                .border_color(cx.theme().border)
                .child(
                    h_flex()
                        .justify_between()
                        .gap_2()
                        .child(
                            v_flex()
                                .gap_0p5()
                                .child(div().text_sm().child(scenario.title.clone()))
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(cx.theme().muted_foreground)
                                        .child(scenario.key.clone()),
                                ),
                        )
                        .child(
                            Button::new(format!("run-scenario-{}", scenario.key))
                                .label(if running { "Running…" } else { "Run fresh" })
                                .xsmall()
                                .disabled(any_scenario_running)
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.run_scenario(
                                        story_key_for_run.clone(),
                                        scenario_for_run.clone(),
                                        window,
                                        cx,
                                    );
                                })),
                        ),
                )
                .when(!scenario.description.is_empty(), |this| {
                    this.child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(scenario.description.clone()),
                    )
                })
                .child(v_flex().gap_1().children(step_rows))
                .when_some(run, |this, run| match run {
                    ScenarioRunState::Running { .. } => this.child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child("Recreated the story and started a fresh run."),
                    ),
                    ScenarioRunState::Finished { result, .. } => match result.as_ref() {
                        Ok(result) => this.child(div().text_xs().child(format!(
                            "Passed · {} postconditions · {}",
                            result.interaction.postconditions.len(),
                            result
                                .interaction
                                .capture
                                .as_ref()
                                .map(|capture| capture.path.display().to_string())
                                .unwrap_or_else(|| "no capture".to_owned())
                        ))),
                        Err(error) => this.child(
                            div()
                                .text_xs()
                                .text_color(cx.theme().danger)
                                .child(error.to_string()),
                        ),
                    },
                })
        });

        v_flex()
            .id("workbench-scenarios")
            .p_4()
            .gap_2()
            .child("Story scenarios")
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child("Every run recreates the story before controls, steps, postconditions, and capture."),
            )
            .children(scenario_rows)
            .into_any_element()
    }

    fn render_actions(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let Some(story) = self.state.read(cx).active_story() else {
            return v_flex()
                .id("workbench-actions")
                .p_4()
                .gap_2()
                .child("Story actions")
                .child(
                    div()
                        .text_color(cx.theme().muted_foreground)
                        .child("Select a story to inspect its actions."),
                )
                .into_any_element();
        };
        let (story_key, action_scope_focus) = {
            let story = story.read(cx);
            (
                story
                    .story_key_label()
                    .unwrap_or("unregistered story")
                    .to_owned(),
                story.action_scope_focus_handle(),
            )
        };
        let Some(action_scope_focus) = action_scope_focus else {
            return v_flex()
                .id("workbench-actions")
                .p_4()
                .gap_2()
                .child("Story actions")
                .child(
                    div()
                        .text_color(cx.theme().muted_foreground)
                        .child("This story does not expose an action scope."),
                )
                .into_any_element();
        };
        let documentation = cx.action_documentation();
        let mut schema_generator = schemars::generate::SchemaSettings::draft2020_12()
            .with(|settings| settings.inline_subschemas = true)
            .into_generator();
        let actions = story_scoped_actions(&action_scope_focus, &self.focus_handle, window, cx);

        let action_rows = actions.into_iter().map(|action| {
            let name = action.name();
            let documentation = documentation.get(name).copied();
            let argument_schema = cx
                .action_schema_by_name(name, &mut schema_generator)
                .flatten()
                .and_then(|schema| serde_json::to_string(&schema).ok());
            let bindings = window
                .bindings_for_action_in(action.as_ref(), &action_scope_focus)
                .into_iter()
                .map(|binding| format_key_binding(&binding))
                .collect::<Vec<_>>();
            let dispatch_focus = action_scope_focus.clone();

            v_flex()
                .id(format!("workbench-action-{name}"))
                .gap_1()
                .py_3()
                .border_b_1()
                .border_color(cx.theme().border)
                .child(
                    h_flex()
                        .justify_between()
                        .gap_2()
                        .child(div().text_sm().child(name))
                        .child(
                            Button::new(format!("dispatch-action-{name}"))
                                .label("Dispatch")
                                .xsmall()
                                .on_click(move |_, window, cx| {
                                    dispatch_focus.dispatch_action(action.as_ref(), window, cx);
                                }),
                        ),
                )
                .when_some(documentation, |this, documentation| {
                    this.child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(documentation),
                    )
                })
                .when_some(argument_schema, |this, schema| {
                    this.child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(format!("Arguments: {schema}")),
                    )
                })
                .when(!bindings.is_empty(), |this| {
                    this.child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(format!("Bindings: {}", bindings.join(", "))),
                    )
                })
        });

        v_flex()
            .id("workbench-actions")
            .p_4()
            .gap_2()
            .child("Story actions")
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(format!("Scope: {story_key}")),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(
                        "Only actions exposed at the selected story's explicit root scope are shown. Nested controls and Storybook shell actions are excluded.",
                    ),
            )
            .when(action_rows.len() == 0, |this| {
                this.child(
                    div()
                        .text_color(cx.theme().muted_foreground)
                        .child("The selected story exposes no default-buildable actions."),
                )
            })
            .children(action_rows)
            .into_any_element()
    }

    #[cfg(feature = "performance")]
    fn render_performance(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        struct Metric {
            label: &'static str,
            samples: u64,
            p50_ms: f64,
            p95_ms: f64,
            p99_ms: f64,
            max_ms: f64,
        }

        macro_rules! duration_metric {
            ($label:literal, $histogram:expr) => {{
                let histogram = &$histogram;
                Metric {
                    label: $label,
                    samples: histogram.len(),
                    p50_ms: histogram.value_at_quantile(0.50) as f64 / 1_000_000.0,
                    p95_ms: histogram.value_at_quantile(0.95) as f64 / 1_000_000.0,
                    p99_ms: histogram.value_at_quantile(0.99) as f64 / 1_000_000.0,
                    max_ms: histogram.max() as f64 / 1_000_000.0,
                }
            }};
        }

        let frames = window.frame_duration_snapshot();
        let input = window.input_latency_snapshot();
        let metrics = [
            duration_metric!("Draw duration", frames.draw_duration_histogram),
            duration_metric!("Dirty to present", frames.dirty_to_present_histogram),
            duration_metric!("Present interval", frames.present_interval_histogram),
            duration_metric!("Input to frame", input.latency_histogram),
        ];
        let overlay_mode = format!("{:?}", window.debug_frame_overlay_mode());

        v_flex()
            .id("workbench-performance")
            .p_4()
            .gap_3()
            .child(
                h_flex()
                    .justify_between()
                    .gap_2()
                    .child("Window performance")
                    .child(
                        h_flex()
                            .gap_1()
                            .child(
                                Button::new("performance-refresh")
                                    .label("Refresh")
                                    .xsmall()
                                    .on_click(|_, window, _| {
                                        window.refresh();
                                    }),
                            )
                            .child(
                                Button::new("performance-cycle-overlay")
                                    .label(format!("Overlay: {overlay_mode}"))
                                    .xsmall()
                                    .on_click(|_, window, _| {
                                        window.cycle_debug_frame_overlay_mode();
                                    }),
                            ),
                    ),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(format!(
                        "Input events dropped during draw: {}",
                        input.mid_draw_events_dropped
                    )),
            )
            .children(metrics.into_iter().map(|metric| {
                v_flex()
                    .gap_1()
                    .py_3()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(
                        h_flex()
                            .justify_between()
                            .child(metric.label)
                            .child(format!("{} samples", metric.samples)),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(format!(
                                "p50 {:.2} ms  ·  p95 {:.2} ms  ·  p99 {:.2} ms  ·  max {:.2} ms",
                                metric.p50_ms, metric.p95_ms, metric.p99_ms, metric.max_ms
                            )),
                    )
            }))
            .into_any_element()
    }
}

impl EventEmitter<PanelEvent> for StoryWorkbench {}

impl Focusable for StoryWorkbench {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl BasePanel for StoryWorkbench {
    fn panel_name(&self) -> &'static str {
        "StoryWorkbench"
    }

    fn closable(&self, _: &App) -> bool {
        false
    }

    fn zoomable(&self, _: &App) -> bool {
        false
    }

    fn dump(&self, _: &App) -> PanelState {
        PanelState {
            panel_name: self.panel_name().to_owned(),
            children: Vec::new(),
            info: PanelInfo::panel(
                serde_json::to_value(StoryWorkbenchPanelState {
                    selected_tab: self.selected_tab,
                })
                .expect("workbench panel state serializes"),
            ),
        }
    }
}

impl Panel for StoryWorkbench {
    fn title(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        "Workbench"
    }

    fn zoom_control(&self, _: &App) -> Option<PanelControl> {
        None
    }

    fn inner_padding(&self, _: &App) -> bool {
        false
    }
}

impl Render for StoryWorkbench {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.sync_editor_values(window, cx);
        self.sync_theme_editors(window, cx);

        let active_story = self.active_story(cx);
        let header = active_story
            .as_ref()
            .map(|story| {
                let story = story.read(cx);
                let title = story.display_title(cx);
                let description = story.display_description(cx);
                if description.is_empty() {
                    title
                } else {
                    format!("{title} — {description}")
                }
            })
            .unwrap_or_else(|| "No active story".to_owned());
        let variants = self.state.read(cx).variants(cx);
        let presentation = self.state.read(cx).presentation();
        let active_story_id = active_story.map(|story| story.entity_id());

        v_flex()
            .id("story-workbench")
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::select_option))
            .on_action(cx.listener(Self::select_viewport))
            .size_full()
            .overflow_hidden()
            .child(
                v_flex()
                    .p_3()
                    .gap_2()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(div().text_sm().child(header))
                    .when(!variants.is_empty(), |this| {
                        this.child(
                            h_flex()
                                .gap_1()
                                .flex_wrap()
                                .children(variants.into_iter().map(|variant| {
                                    let title = variant.read(cx).display_description(cx);
                                    let selected = active_story_id == Some(variant.entity_id());
                                    let state = self.state.clone();
                                    Button::new(format!(
                                        "workbench-variant-{}",
                                        variant.entity_id()
                                    ))
                                    .label(if title.is_empty() {
                                        variant.read(cx).display_title(cx)
                                    } else {
                                        title
                                    })
                                    .xsmall()
                                    .selected(selected)
                                    .on_click(
                                        move |_, _, cx| {
                                            state.update(cx, |state, cx| {
                                                state.set_active_variant(variant.clone(), cx);
                                            });
                                        },
                                    )
                                })),
                        )
                    })
                    .child(
                        h_flex().gap_1().flex_wrap().child(
                            Button::new("workbench-viewport")
                                .debug_selector(|| "workbench-viewport".to_owned())
                                .label(presentation.viewport.label())
                                .xsmall()
                                .dropdown_menu(move |menu, _, _| {
                                    StoryViewportPreset::ALL.into_iter().fold(
                                        menu,
                                        |menu, viewport| {
                                            menu.menu_with_check(
                                                viewport.label(),
                                                viewport == presentation.viewport,
                                                Box::new(SelectViewport { viewport }),
                                            )
                                        },
                                    )
                                }),
                        ),
                    ),
            )
            .child(
                TabBar::new("workbench-tabs")
                    .selected_index(self.selected_tab.index())
                    .on_click(cx.listener(|this, index, _, cx| {
                        this.selected_tab = WorkbenchTab::from_index(*index);
                        cx.emit(PanelEvent::LayoutChanged);
                        cx.notify();
                    }))
                    .child(Tab::new().label("Controls"))
                    .child(Tab::new().label("Theme"))
                    .child(Tab::new().label("Inspect"))
                    .child(Tab::new().label("Actions"))
                    .child(Tab::new().label("Scenarios"))
                    .when(cfg!(feature = "performance"), |this| {
                        #[cfg(feature = "performance")]
                        let this = this.child(Tab::new().label("Perf"));
                        this
                    }),
            )
            .when_some(self.last_error.clone(), |this, error| {
                this.child(
                    div()
                        .px_4()
                        .py_2()
                        .text_xs()
                        .text_color(cx.theme().danger)
                        .child(error),
                )
            })
            .child(match self.selected_tab {
                WorkbenchTab::Controls => div()
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scrollbar()
                    .child(self.render_controls(window, cx))
                    .into_any_element(),
                WorkbenchTab::Theme => div()
                    .flex_1()
                    .min_h_0()
                    .overflow_hidden()
                    .child(self.render_theme(cx))
                    .into_any_element(),
                WorkbenchTab::Inspect => div()
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scrollbar()
                    .child(self.render_inspect(cx))
                    .into_any_element(),
                WorkbenchTab::Actions => div()
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scrollbar()
                    .child(self.render_actions(window, cx))
                    .into_any_element(),
                WorkbenchTab::Scenarios => div()
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scrollbar()
                    .child(self.render_scenarios(window, cx))
                    .into_any_element(),
                #[cfg(feature = "performance")]
                WorkbenchTab::Performance => div()
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scrollbar()
                    .child(self.render_performance(window, cx))
                    .into_any_element(),
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{ScrollDelta, ScrollWheelEvent, TestAppContext, VisualTestContext, point};

    #[derive(Action, Clone, Default, Eq, PartialEq)]
    #[action(namespace = storybook_action_scope_test)]
    struct ShellAction;

    #[derive(Action, Clone, Default, Eq, PartialEq)]
    #[action(namespace = storybook_action_scope_test)]
    struct StoryAction;

    #[derive(Action, Clone, Default, Eq, PartialEq)]
    #[action(namespace = storybook_action_scope_test)]
    struct NestedInputAction;

    struct ActionScopeFixture {
        shell_focus: FocusHandle,
        story_focus: FocusHandle,
        nested_input_focus: FocusHandle,
    }

    impl ActionScopeFixture {
        fn new(cx: &mut Context<Self>) -> Self {
            Self {
                shell_focus: cx.focus_handle(),
                story_focus: cx.focus_handle(),
                nested_input_focus: cx.focus_handle(),
            }
        }

        fn ignore_shell_action(&mut self, _: &ShellAction, _: &mut Window, _: &mut Context<Self>) {}

        fn ignore_story_action(&mut self, _: &StoryAction, _: &mut Window, _: &mut Context<Self>) {}

        fn ignore_nested_input_action(
            &mut self,
            _: &NestedInputAction,
            _: &mut Window,
            _: &mut Context<Self>,
        ) {
        }
    }

    impl Render for ActionScopeFixture {
        fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            div()
                .on_action(cx.listener(Self::ignore_shell_action))
                .child(div().track_focus(&self.shell_focus))
                .child(
                    div()
                        .track_focus(&self.story_focus)
                        .on_action(cx.listener(Self::ignore_story_action))
                        .child(
                            div()
                                .track_focus(&self.nested_input_focus)
                                .on_action(cx.listener(Self::ignore_nested_input_action)),
                        ),
                )
        }
    }

    struct ScopedStory {
        interaction_focus: FocusHandle,
        action_scope_focus: FocusHandle,
    }

    impl crate::controls::StoryControls for ScopedStory {}

    impl Focusable for ScopedStory {
        fn focus_handle(&self, _: &App) -> FocusHandle {
            self.interaction_focus.clone()
        }
    }

    impl crate::story::Story for ScopedStory {
        fn title(_: &App) -> String {
            "Scoped story".to_owned()
        }

        fn new_view(_: &mut Window, cx: &mut App) -> Entity<Self> {
            cx.new(|cx| Self {
                interaction_focus: cx.focus_handle(),
                action_scope_focus: cx.focus_handle(),
            })
        }

        fn action_scope_focus_handle(&self, _: &App) -> Option<FocusHandle> {
            Some(self.action_scope_focus.clone())
        }
    }

    impl ScopedStory {
        fn ignore_story_action(&mut self, _: &StoryAction, _: &mut Window, _: &mut Context<Self>) {}

        fn ignore_nested_input_action(
            &mut self,
            _: &NestedInputAction,
            _: &mut Window,
            _: &mut Context<Self>,
        ) {
        }
    }

    impl Render for ScopedStory {
        fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            div()
                .track_focus(&self.action_scope_focus)
                .on_action(cx.listener(Self::ignore_story_action))
                .child(
                    div()
                        .track_focus(&self.interaction_focus)
                        .on_action(cx.listener(Self::ignore_nested_input_action)),
                )
        }
    }

    struct StoryContainerActionFixture {
        shell_focus: FocusHandle,
        story: Entity<StoryContainer>,
    }

    impl StoryContainerActionFixture {
        fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
            Self {
                shell_focus: cx.focus_handle(),
                story: StoryContainer::panel::<ScopedStory>(window, cx),
            }
        }

        fn ignore_shell_action(&mut self, _: &ShellAction, _: &mut Window, _: &mut Context<Self>) {}
    }

    impl Render for StoryContainerActionFixture {
        fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            div()
                .on_action(cx.listener(Self::ignore_shell_action))
                .child(div().track_focus(&self.shell_focus))
                .child(self.story.clone())
        }
    }

    #[test]
    fn workbench_tabs_have_stable_persisted_indices() {
        for (tab, index) in [
            (WorkbenchTab::Controls, 0),
            (WorkbenchTab::Theme, 1),
            (WorkbenchTab::Inspect, 2),
            (WorkbenchTab::Actions, 3),
            (WorkbenchTab::Scenarios, 4),
            #[cfg(feature = "performance")]
            (WorkbenchTab::Performance, 5),
        ] {
            assert_eq!(tab.index(), index);
            assert_eq!(WorkbenchTab::from_index(index), tab);
        }
    }

    #[test]
    fn action_debugger_formats_multi_stroke_bindings() {
        let binding = KeyBinding::new(
            "ctrl-k enter",
            SelectViewport {
                viewport: StoryViewportPreset::Mobile,
            },
            None,
        );
        assert_eq!(format_key_binding(&binding), "ctrl-K enter");
    }

    #[gpui::test]
    fn action_debugger_keeps_only_selected_story_actions(cx: &mut TestAppContext) {
        let window = cx.update(|cx| {
            cx.open_window(Default::default(), |_, cx| cx.new(ActionScopeFixture::new))
                .expect("action scope test window")
        });
        let mut visual_cx = VisualTestContext::from_window(window.into(), cx);
        let fixture = window
            .root(&mut visual_cx)
            .expect("action scope fixture should be the window root");
        visual_cx.update(|window, cx| {
            _ = window.draw(cx);
        });
        let (shell_focus, story_focus) = fixture.read_with(&visual_cx, |fixture, _| {
            (fixture.shell_focus.clone(), fixture.story_focus.clone())
        });

        visual_cx.update(|window, _| {
            assert!(!is_story_scoped_action(
                &ShellAction,
                &story_focus,
                &shell_focus,
                window,
            ));
            assert!(is_story_scoped_action(
                &StoryAction,
                &story_focus,
                &shell_focus,
                window,
            ));
            assert!(!is_story_scoped_action(
                &NestedInputAction,
                &story_focus,
                &shell_focus,
                window,
            ));
        });
        let action_names = visual_cx.update(|window, cx| {
            story_scoped_actions(&story_focus, &shell_focus, window, cx)
                .into_iter()
                .map(|action| action.name())
                .filter(|name| {
                    [
                        ShellAction.name(),
                        StoryAction.name(),
                        NestedInputAction.name(),
                    ]
                    .contains(name)
                })
                .collect::<Vec<_>>()
        });
        assert_eq!(action_names, vec![StoryAction.name()]);
    }

    #[gpui::test]
    fn action_debugger_uses_the_story_root_instead_of_its_nested_interaction_focus(
        cx: &mut TestAppContext,
    ) {
        let window = cx.update(|cx| {
            gpui_component::init(cx);
            cx.open_window(Default::default(), |window, cx| {
                cx.new(|cx| StoryContainerActionFixture::new(window, cx))
            })
            .expect("story container action fixture window")
        });
        let mut visual_cx = VisualTestContext::from_window(window.into(), cx);
        let fixture = window
            .root(&mut visual_cx)
            .expect("story container action fixture should be the window root");
        visual_cx.update(|window, cx| {
            _ = window.draw(cx);
        });
        let (shell_focus, story) = fixture.read_with(&visual_cx, |fixture, _| {
            (fixture.shell_focus.clone(), fixture.story.clone())
        });
        let (interaction_focus, action_scope_focus) = visual_cx.update(|_, cx| {
            (
                story.focus_handle(cx),
                story
                    .read(cx)
                    .action_scope_focus_handle()
                    .expect("scoped story should expose an action focus handle"),
            )
        });

        visual_cx.update(|window, _| {
            assert!(window.is_action_available_in(&NestedInputAction, &interaction_focus));
            assert!(!window.is_action_available_in(&NestedInputAction, &action_scope_focus));
            assert!(is_story_scoped_action(
                &StoryAction,
                &action_scope_focus,
                &shell_focus,
                window,
            ));
            assert!(!is_story_scoped_action(
                &ShellAction,
                &action_scope_focus,
                &shell_focus,
                window,
            ));
        });

        let action_names = visual_cx.update(|window, cx| {
            story_scoped_actions(&action_scope_focus, &shell_focus, window, cx)
                .into_iter()
                .map(|action| action.name())
                .filter(|name| {
                    [
                        ShellAction.name(),
                        StoryAction.name(),
                        NestedInputAction.name(),
                    ]
                    .contains(name)
                })
                .collect::<Vec<_>>()
        });
        assert_eq!(action_names, vec![StoryAction.name()]);
    }

    #[test]
    fn scenario_progress_reports_only_completed_steps() {
        assert_eq!(
            StoryWorkbench::scenario_progress(&StorybookAutomationError::InteractionFailed {
                request_id: 7,
                steps_dispatched: 2,
                message: "postcondition failed".to_owned(),
            }),
            2
        );
        assert_eq!(
            StoryWorkbench::scenario_progress(&StorybookAutomationError::AutomationBusy),
            0
        );
    }

    #[cfg(feature = "dock")]
    #[test]
    fn persisted_panel_state_restores_the_selected_tab() {
        let info = PanelInfo::panel(
            serde_json::to_value(StoryWorkbenchPanelState {
                selected_tab: WorkbenchTab::Theme,
            })
            .expect("panel state serializes"),
        );
        assert_eq!(
            StoryWorkbench::selected_tab_from_panel(&info),
            WorkbenchTab::Theme
        );
    }

    #[test]
    fn theme_header_stays_fixed_while_color_items_scroll() {
        let mut app = TestAppContext::single();
        app.update(gpui_component::init);
        let window = app.open_window(size(px(400.), px(600.)), |window, cx| {
            let state = cx.new(|_| WorkbenchState::new(None));
            StoryWorkbench::new(state, WorkbenchTab::Theme, window, cx)
        });
        let mut visual_cx = VisualTestContext::from_window(*window, &app);
        let cx = &mut visual_cx;
        let draw = |cx: &mut VisualTestContext| {
            cx.run_until_parked();
            cx.update(|window, cx| {
                _ = window.draw(cx);
            });
        };

        draw(cx);
        let header_before = cx
            .debug_bounds("workbench-theme-sticky-header")
            .expect("theme header should render");
        let items = cx
            .debug_bounds("workbench-theme-items")
            .expect("theme items should render");
        let first_item_before = cx
            .debug_bounds("workbench-theme-first-item")
            .expect("theme color items should render");

        cx.simulate_event(ScrollWheelEvent {
            position: items.center(),
            delta: ScrollDelta::Pixels(point(px(0.), px(-120.))),
            ..Default::default()
        });
        draw(cx);

        let header_after = cx
            .debug_bounds("workbench-theme-sticky-header")
            .expect("theme header should remain rendered");
        let first_item_after = cx
            .debug_bounds("workbench-theme-first-item")
            .expect("theme color items should remain rendered");
        assert_eq!(header_after.origin, header_before.origin);
        assert!(
            first_item_after.origin.y < first_item_before.origin.y,
            "theme items should move after scrolling: before={first_item_before:?}, after={first_item_after:?}, viewport={items:?}"
        );
    }

    #[gpui::test]
    fn window_scoped_states_keep_preview_independent(cx: &mut App) {
        let first = cx.new(|_| WorkbenchState::new(None));
        let second = cx.new(|_| WorkbenchState::new(None));

        first.update(cx, |state, cx| {
            state.set_viewport(StoryViewportPreset::Mobile, cx);
            state.set_background(StoryCanvasBackground::Dark, cx);
        });

        assert_eq!(
            first.read(cx).presentation(),
            StoryPresentation {
                viewport: StoryViewportPreset::Mobile,
                background: StoryCanvasBackground::Dark,
            }
        );
        assert_eq!(first.read(cx).responsive_size(), None);
        assert_eq!(second.read(cx).presentation(), StoryPresentation::default());
    }

    #[gpui::test]
    fn responsive_viewport_inherits_the_previous_fixed_preset(cx: &mut App) {
        let state = cx.new(|_| WorkbenchState::new(None));

        state.update(cx, |state, cx| {
            state.set_viewport(StoryViewportPreset::Mobile, cx);
            state.set_viewport(StoryViewportPreset::Responsive, cx);
        });
        assert_eq!(
            state.read(cx).responsive_size(),
            Some(size(px(390.), px(844.)))
        );

        state.update(cx, |state, cx| {
            state.set_viewport(StoryViewportPreset::Desktop, cx);
            state.set_viewport(StoryViewportPreset::Responsive, cx);
        });
        assert_eq!(
            state.read(cx).responsive_size(),
            Some(size(px(1440.), px(900.)))
        );
    }

    #[test]
    fn story_source_url_resolves_workspace_relative_files_from_the_crate_directory() {
        let workspace = tempfile::tempdir().expect("temporary workspace");
        let crate_dir = workspace.path().join("examples/story");
        let source_file = workspace.path().join("examples/story/src/lib.rs");
        std::fs::create_dir_all(source_file.parent().expect("source parent"))
            .expect("create source directory");
        std::fs::File::create(&source_file).expect("create source file");

        assert_eq!(
            story_source_url(
                crate_dir.to_str().expect("UTF-8 crate directory"),
                "examples/story/src/lib.rs",
            ),
            Some(
                url::Url::from_file_path(source_file)
                    .expect("source file URL")
                    .into(),
            )
        );
    }
}
