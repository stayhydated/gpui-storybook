use super::*;

impl Gallery {
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
            active_story_to_render = self.workbench_state.read(cx).active_story();
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
