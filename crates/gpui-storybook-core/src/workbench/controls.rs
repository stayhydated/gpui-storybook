use super::*;

impl StoryWorkbench {
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

    pub(super) fn render_controls(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if self.active_story(cx).is_none() {
            return v_flex()
                .p_4()
                .text_color(cx.theme().muted_foreground)
                .child("Select a story")
                .into_any_element();
        }
        let Some(target) = self.active_target(cx) else {
            return v_flex()
                .p_4()
                .text_color(cx.theme().muted_foreground)
                .child("No controls")
                .into_any_element();
        };
        let specs = target.specs().to_vec();
        let target_for_reset = target.clone();

        v_flex()
            .id("workbench-controls")
            .p_4()
            .gap_1()
            .child(
                h_flex().justify_end().child(
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
}
