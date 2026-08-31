use super::*;

impl StoryWorkbench {
    pub(super) fn render_theme(&self, cx: &mut Context<Self>) -> AnyElement {
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
}
