use super::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct StoryVariantOption {
    pub(super) id: EntityId,
    pub(super) label: SharedString,
}

impl SearchableListItem for StoryVariantOption {
    type Value = EntityId;

    fn title(&self) -> SharedString {
        self.label.clone()
    }

    fn value(&self) -> &Self::Value {
        &self.id
    }
}

#[derive(Action, Clone, Eq, PartialEq)]
#[action(namespace = storybook_workbench, no_json)]
pub(super) struct SelectControlOption {
    pub(super) key: String,
    pub(super) value: String,
}

#[derive(Action, Clone, Copy, Eq, PartialEq)]
#[action(namespace = storybook_workbench, no_json)]
pub(super) struct SelectViewport {
    pub(super) viewport: StoryViewportPreset,
}

pub(super) enum ControlEditor {
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
pub(super) fn story_source_url(crate_dir: &str, source_file: &str) -> Option<String> {
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
pub(super) fn story_source_url(_: &str, _: &str) -> Option<String> {
    None
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub(super) struct StoryWorkbenchPanelState {
    pub(super) selected_tab: WorkbenchTab,
}

pub(super) enum ScenarioRunState {
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

impl StoryWorkbench {
    pub fn new(
        state: Entity<WorkbenchState>,
        selected_tab: WorkbenchTab,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let variant_select = cx.new(|cx| SelectState::new(Vec::new(), None, window, cx));
        let state_for_select = state.clone();
        let variant_subscription = cx.subscribe_in(
            &variant_select,
            window,
            move |_, _, event: &SelectEvent<Vec<StoryVariantOption>>, _, cx| {
                let SelectEvent::Confirm(Some(id)) = event else {
                    return;
                };
                let variant = state_for_select
                    .read(cx)
                    .variants(cx)
                    .into_iter()
                    .find(|variant| variant.entity_id() == *id);
                if let Some(variant) = variant {
                    state_for_select.update(cx, |state, cx| {
                        state.set_active_variant(variant, cx);
                    });
                }
            },
        );
        let state_subscription = cx.observe_in(&state, window, |this, state, window, cx| {
            this.sync_variant_select(&state, window, cx);
            cx.notify();
        });
        let theme_draft = ThemeDraft::new(cx.theme())
            .expect("the active GPUI Component theme must produce a workbench draft");
        let theme_search =
            cx.new(|cx| InputState::new(window, cx).placeholder("Search theme colors..."));
        let theme_search_subscription =
            cx.subscribe(&theme_search, |_, _, _: &InputEvent, cx| cx.notify());
        let mut this = Self {
            focus_handle: cx.focus_handle(),
            state,
            variant_select,
            variant_options: Vec::new(),
            selected_tab,
            editor_story: None,
            editors: BTreeMap::new(),
            editor_subscriptions: Vec::new(),
            story_subscription: None,
            _variant_subscription: variant_subscription,
            _state_subscription: state_subscription,
            theme_draft,
            theme_search,
            theme_editors: BTreeMap::new(),
            theme_subscriptions: vec![theme_search_subscription],
            scenario_run: None,
            last_error: None,
        };
        let state = this.state.clone();
        this.sync_variant_select(&state, window, cx);
        this.rebuild_control_editors(window, cx);
        this.rebuild_theme_editors(window, cx);
        this
    }

    pub(crate) fn selected_tab_from_panel(info: &PanelInfo) -> WorkbenchTab {
        let PanelInfo::Panel(value) = info else {
            return WorkbenchTab::default();
        };
        serde_json::from_value::<StoryWorkbenchPanelState>(value.clone())
            .unwrap_or_default()
            .selected_tab
    }

    pub(super) fn active_story(&self, cx: &App) -> Option<Entity<StoryContainer>> {
        self.state.read(cx).active_story()
    }

    pub(super) fn active_target(&self, cx: &App) -> Option<Rc<dyn ControlTarget>> {
        self.active_story(cx)?.read(cx).control_target()
    }

    pub(super) fn sync_variant_select(
        &mut self,
        state: &Entity<WorkbenchState>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let state = state.read(cx);
        let options = state
            .variants(cx)
            .into_iter()
            .map(|variant| StoryVariantOption {
                id: variant.entity_id(),
                label: variant.read(cx).variant_label(cx).into(),
            })
            .collect::<Vec<_>>();
        let selected = state.active_story().map(|story| story.entity_id());
        let current_selected = self.variant_select.read(cx).selected_value().cloned();

        let options_changed = self.variant_options != options;
        if options_changed {
            self.variant_options = options.clone();
            self.variant_select.update(cx, |select, cx| {
                select.set_items(options, window, cx);
            });
        }

        if options_changed || current_selected != selected {
            self.variant_select.update(cx, |select, cx| {
                if let Some(selected) = selected {
                    select.set_selected_value(&selected, window, cx);
                } else {
                    select.set_selected_index(None, window, cx);
                }
            });
        }
    }

    pub(super) fn editor_value(value: &ControlValue) -> String {
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

    pub(super) fn rebuild_control_editors(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.editors.clear();
        self.editor_subscriptions.clear();
        self.last_error = None;

        let Some(story) = self.active_story(cx) else {
            self.editor_story = None;
            self.story_subscription = None;
            return;
        };
        let story_id = story.entity_id();
        if self.editor_story.map(|(entity_id, _)| entity_id) != Some(story_id) {
            self.story_subscription = Some(cx.subscribe_in(
                &story,
                window,
                |this, _, event: &ContainerEvent, window, cx| {
                    if matches!(event, ContainerEvent::Recreated { .. }) {
                        this.rebuild_control_editors(window, cx);
                        cx.notify();
                    }
                },
            ));
        }
        self.editor_story = Some((story_id, story.read(cx).recreation_generation()));
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
                        ColorPickerState::new(window, cx).default_value(gpui_kit::Hsla::from(color))
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

    pub(super) fn sync_editor_values(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let active_story = self.active_story(cx).map(|story| {
            let story_data = story.read(cx);
            (story.entity_id(), story_data.recreation_generation())
        });
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
                    let expected = gpui_kit::Hsla::from(color);
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

    pub(super) fn sync_theme_editors(&mut self, window: &mut Window, cx: &mut Context<Self>) {
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

    pub(super) fn select_option(
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

    pub(super) fn select_viewport(
        &mut self,
        action: &SelectViewport,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.state.update(cx, |state, cx| {
            state.set_viewport(action.viewport, cx);
        });
    }
}
