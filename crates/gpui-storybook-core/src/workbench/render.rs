use super::*;

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

        let presentation = self.state.read(cx).presentation();

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
                    .when(!self.variant_options.is_empty(), |this| {
                        this.child(
                            v_flex()
                                .gap_1()
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(cx.theme().muted_foreground)
                                        .child("Variant"),
                                )
                                .child(
                                    div()
                                        .debug_selector(|| "workbench-variant-select".to_owned())
                                        .child(Select::new(&self.variant_select).xsmall().w_full()),
                                ),
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
                    .overflow_hidden()
                    .child(self.render_actions(window, cx))
                    .into_any_element(),
                WorkbenchTab::Scenarios => div()
                    .flex_1()
                    .min_h_0()
                    .overflow_hidden()
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
