use super::*;

impl Render for StoryContainer {
    fn render(&mut self, _: &mut Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let canvas_scroll_handle = self.scroll_handle.clone();
        let story_scroll_handle = self.story_scroll_handle.clone();
        let story_key = self.story_key_label().map(str::to_owned);
        let presentation = self.presentation;
        let is_responsive =
            presentation.viewport == crate::presentation::StoryViewportPreset::Responsive;
        let viewport_size = self.viewport_size();
        let automation_size = self.automation_size;
        let background = match presentation.background {
            StoryCanvasBackground::Theme => cx.theme().background,
            StoryCanvasBackground::Light => hsla(0.0, 0.0, 0.98, 1.0),
            StoryCanvasBackground::Dark => hsla(0.0, 0.0, 0.08, 1.0),
            StoryCanvasBackground::Transparent => hsla(0.0, 0.0, 0.0, 0.0),
        };
        let border_color = cx.theme().border;
        let frame = || {
            div()
                .absolute()
                .top_0()
                .left_0()
                .size_full()
                .border_1()
                .border_color(border_color)
                .debug_selector(|| "story-canvas-border".to_owned())
        };
        let resize_handles = if is_responsive {
            vec![
                self.render_canvas_resize_handle(StoryCanvasResizeAxis::Horizontal, cx)
                    .into_any_element(),
                self.render_canvas_resize_handle(StoryCanvasResizeAxis::Vertical, cx)
                    .into_any_element(),
                self.render_canvas_resize_handle(StoryCanvasResizeAxis::Both, cx)
                    .into_any_element(),
            ]
        } else {
            Vec::new()
        };
        let story_for_canvas_bounds = cx.entity();
        let canvas = div()
            .relative()
            .flex_none()
            .bg(background)
            .debug_selector(|| "story-canvas".to_owned())
            .map(|this| match viewport_size {
                Some(size) => this.w(size.width).h(size.height),
                None => this.size_full(),
            })
            .when_some(self.story.clone(), |this, story| {
                this.child(
                    div()
                        .relative()
                        .size_full()
                        .child(
                            div()
                                .id("story-content-scroll-region")
                                .debug_selector(|| "story-content-scroll-region".to_owned())
                                .size_full()
                                .overflow_hidden()
                                .track_scroll(&story_scroll_handle)
                                .child(
                                    div()
                                        .flex_none()
                                        .w_auto()
                                        .h_auto()
                                        .min_w_full()
                                        .min_h_full()
                                        .p_4()
                                        .child(story),
                                ),
                        )
                        .child(
                            ScrollableMask::new(Axis::Vertical, &story_scroll_handle)
                                .id("story-content-scroll-region"),
                        )
                        .child(
                            ScrollableMask::new(Axis::Horizontal, &story_scroll_handle)
                                .id("story-content-scroll-region"),
                        )
                        .scrollbar(&story_scroll_handle, ScrollbarAxis::Both),
                )
            })
            .child(frame())
            .children(resize_handles)
            .on_prepaint(move |bounds, _, cx| {
                story_for_canvas_bounds.update(cx, |story, _| {
                    story.canvas_bounds = Some(bounds);
                });
            });
        let story_for_stage_bounds = cx.entity();
        let canvas_stage = div()
            .relative()
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .debug_selector(|| "story-canvas-stage".to_owned())
            .when(is_responsive, |this| this.p(STORY_CANVAS_RESIZE_GUTTER))
            .map(|this| match viewport_size {
                Some(size) if is_responsive => this
                    .min_w_full()
                    .min_h_full()
                    .w(size.width + STORY_CANVAS_RESIZE_GUTTER * 2.)
                    .h(size.height + STORY_CANVAS_RESIZE_GUTTER * 2.),
                Some(size) => this.min_w_full().min_h_full().w(size.width).h(size.height),
                None => this.size_full(),
            })
            .child(canvas)
            .on_prepaint(move |bounds, _, cx| {
                story_for_stage_bounds.update(cx, |story, _| {
                    story.canvas_stage_bounds = Some(bounds);
                });
            });
        let content = v_flex()
            .id("story-container")
            .debug_selector(|| "story-container-scroll-region".to_owned())
            .when(automation_size.is_none(), |this| this.size_full())
            .when_some(automation_size, |this, size| {
                this.flex_none().w(size.width).h(size.height)
            })
            .track_scroll(&canvas_scroll_handle)
            .overflow_scroll()
            .restrict_scroll_to_axis()
            .track_focus(&self.focus_handle)
            .child(canvas_stage)
            .scrollbar(&canvas_scroll_handle, ScrollbarAxis::Both);
        #[cfg(feature = "inspector")]
        let content = crate::story_inspector::inspectable_story(
            crate::story_inspector::StoryInspectorState::from_container(self, cx),
            content,
        );

        if let Some(story_key) = story_key {
            capture_story_view(story_key, story_scroll_handle, content).into_any_element()
        } else {
            capture_scroll_scope(story_scroll_handle, content).into_any_element()
        }
    }
}
