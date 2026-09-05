use super::*;

impl Gallery {
    pub(super) fn set_left_sidebar_width(&mut self, width: Pixels) {
        if self.left_sidebar_width == width {
            return;
        }
        self.left_sidebar_width = width;
    }

    pub(super) fn set_right_sidebar_width(&mut self, width: Pixels) {
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

    pub(super) fn render_story_sidebar(
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
        let show_group_labels = self
            .stories
            .iter()
            .map(|story| story.read(cx).sidebar_group())
            .collect::<BTreeSet<_>>()
            .len()
            > 1;

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

            SidebarGroup::new(group.filter(|_| show_group_labels).unwrap_or_default())
                .child(SidebarMenu::new().children(menu_items))
        });

        Sidebar::new("sidebar-gallery")
            .side(gpui_kit::component::Side::Left)
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
}
