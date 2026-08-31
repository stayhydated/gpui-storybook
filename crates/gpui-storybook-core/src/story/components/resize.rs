use super::*;

impl StoryContainer {
    /// Applies viewport and canvas-background presentation to this story.
    ///
    /// Portable runners use this before the first draw so every matrix case
    /// renders the requested presentation rather than merely labeling it.
    pub fn set_presentation(&mut self, presentation: StoryPresentation) {
        self.presentation = presentation;
    }

    pub(crate) fn set_responsive_size(&mut self, responsive_size: Option<Size<Pixels>>) {
        self.responsive_size = responsive_size;
    }

    pub(crate) fn set_automation_size(&mut self, size: Option<gpui::Size<gpui::Pixels>>) {
        self.automation_size = size;
    }

    pub fn presentation(&self) -> StoryPresentation {
        self.presentation
    }

    pub(crate) fn set_workbench_state(
        &mut self,
        state: gpui::WeakEntity<crate::workbench::WorkbenchState>,
    ) {
        self.workbench_state = Some(state);
    }

    pub(super) fn viewport_size(&self) -> Option<Size<Pixels>> {
        self.presentation
            .viewport
            .dimensions()
            .map(|(width, height)| size(px(width as f32), px(height as f32)))
            .or(self.responsive_size)
    }

    fn begin_canvas_resize(&mut self, start_position: Point<Pixels>) {
        let (Some(canvas_bounds), Some(stage_bounds)) =
            (self.canvas_bounds, self.canvas_stage_bounds)
        else {
            return;
        };
        let gutter = STORY_CANVAS_RESIZE_GUTTER * 2.;
        let available_stage_size = size(
            (stage_bounds.size.width - gutter).max(px(0.)),
            (stage_bounds.size.height - gutter).max(px(0.)),
        );
        self.canvas_resize_drag = Some(StoryCanvasResizeDrag {
            start_position,
            start_size: canvas_bounds.size,
            horizontal_scale: if canvas_bounds.size.width < available_stage_size.width {
                2.
            } else {
                1.
            },
            vertical_scale: if canvas_bounds.size.height < available_stage_size.height {
                2.
            } else {
                1.
            },
        });
    }

    fn resize_canvas(
        &mut self,
        axis: StoryCanvasResizeAxis,
        position: Point<Pixels>,
        cx: &mut gpui::Context<Self>,
    ) {
        if self.presentation.viewport != crate::presentation::StoryViewportPreset::Responsive {
            return;
        }
        let Some(drag) = self.canvas_resize_drag else {
            return;
        };
        let delta = position - drag.start_position;
        let width = match axis {
            StoryCanvasResizeAxis::Horizontal | StoryCanvasResizeAxis::Both => {
                (drag.start_size.width + delta.x * drag.horizontal_scale)
                    .max(STORY_CANVAS_MIN_SIZE.width)
            },
            StoryCanvasResizeAxis::Vertical => drag.start_size.width,
        };
        let height = match axis {
            StoryCanvasResizeAxis::Vertical | StoryCanvasResizeAxis::Both => {
                (drag.start_size.height + delta.y * drag.vertical_scale)
                    .max(STORY_CANVAS_MIN_SIZE.height)
            },
            StoryCanvasResizeAxis::Horizontal => drag.start_size.height,
        };
        let responsive_size = size(width, height);
        self.responsive_size = Some(responsive_size);
        if let Some(workbench_state) = self
            .workbench_state
            .as_ref()
            .and_then(gpui::WeakEntity::upgrade)
        {
            workbench_state.update(cx, |state, cx| {
                state.set_responsive_size(responsive_size, cx);
            });
        }
        cx.notify();
    }

    pub(super) fn render_canvas_resize_handle(
        &self,
        axis: StoryCanvasResizeAxis,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        let entity_id = cx.entity_id();
        let story_for_mouse_down = cx.entity();
        let (id, selector) = match axis {
            StoryCanvasResizeAxis::Horizontal => {
                ("story-canvas-resize-width", "story-canvas-resize-width")
            },
            StoryCanvasResizeAxis::Vertical => {
                ("story-canvas-resize-height", "story-canvas-resize-height")
            },
            StoryCanvasResizeAxis::Both => {
                ("story-canvas-resize-corner", "story-canvas-resize-corner")
            },
        };

        div()
            .id(id)
            .absolute()
            .debug_selector(move || selector.to_owned())
            .map(|this| match axis {
                StoryCanvasResizeAxis::Horizontal => this
                    .top_0()
                    .right(px(-4.))
                    .h_full()
                    .w(px(9.))
                    .cursor_ew_resize(),
                StoryCanvasResizeAxis::Vertical => this
                    .left_0()
                    .bottom(px(-4.))
                    .w_full()
                    .h(px(9.))
                    .cursor_ns_resize(),
                StoryCanvasResizeAxis::Both => this
                    .right(px(-5.))
                    .bottom(px(-5.))
                    .size(px(12.))
                    .cursor_nwse_resize(),
            })
            .on_mouse_down(gpui::MouseButton::Left, move |event, _, cx| {
                cx.stop_propagation();
                story_for_mouse_down.update(cx, |story, _| {
                    story.begin_canvas_resize(event.position);
                });
            })
            .on_drag(
                DragStoryCanvasResize { entity_id, axis },
                move |drag, _, _, cx| {
                    cx.stop_propagation();
                    cx.new(|_| *drag)
                },
            )
            .on_drag_move(cx.listener(
                move |this, event: &DragMoveEvent<DragStoryCanvasResize>, _, cx| {
                    let drag = event.drag(cx);
                    if drag.entity_id == entity_id && drag.axis == axis {
                        this.resize_canvas(axis, event.event.position, cx);
                    }
                },
            ))
    }
}
