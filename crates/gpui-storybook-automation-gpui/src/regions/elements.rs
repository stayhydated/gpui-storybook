use super::*;

pub(super) struct CaptureScopeElement {
    pub(super) story_key: Option<String>,
    pub(super) scroll_handle: Option<ScrollHandle>,
    pub(super) child: AnyElement,
}

impl IntoElement for CaptureScopeElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for CaptureScopeElement {
    type RequestLayoutState = LayoutId;
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let regions = window_regions(window, cx);
        let inherited = current_scope();
        let story_key = self
            .story_key
            .clone()
            .or_else(|| inherited.as_ref().and_then(|scope| scope.story_key.clone()));
        let scope = CaptureScope {
            regions,
            route_id: self
                .story_key
                .clone()
                .or_else(|| inherited.as_ref().and_then(|scope| scope.route_id.clone())),
            story_key,
            viewport_bounds: None,
            scroll_handle: self.scroll_handle.clone().or_else(|| {
                inherited
                    .as_ref()
                    .and_then(|scope| scope.scroll_handle.clone())
            }),
        };
        let layout_id = with_scope(scope, || self.child.request_layout(window, cx));
        (layout_id, layout_id)
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let regions = window_regions(window, cx);
        let inherited = current_scope();
        let story_key = self
            .story_key
            .clone()
            .or_else(|| inherited.as_ref().and_then(|scope| scope.story_key.clone()));
        let route_id = self
            .story_key
            .clone()
            .or_else(|| inherited.as_ref().and_then(|scope| scope.route_id.clone()));
        let scope = CaptureScope {
            regions,
            story_key,
            route_id,
            // Ancestor clips are the actual story pane, even when automation
            // requests a story larger than that pane's current layout.
            viewport_bounds: Some(bounds.intersect(&window.content_mask().bounds)),
            scroll_handle: self.scroll_handle.clone().or_else(|| {
                inherited
                    .as_ref()
                    .and_then(|scope| scope.scroll_handle.clone())
            }),
        };

        if let Some(story_key) = self.story_key.clone() {
            scope.regions.reset_story(&story_key);
            record_region(story_key, bounds, &scope, window.viewport_size());
        }

        with_scope(scope, || {
            self.child.prepaint(window, cx);
        });
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        _prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let regions = window_regions(window, cx);
        let inherited = current_scope();
        let scope = CaptureScope {
            regions,
            story_key: self
                .story_key
                .clone()
                .or_else(|| inherited.as_ref().and_then(|scope| scope.story_key.clone())),
            route_id: self
                .story_key
                .clone()
                .or_else(|| inherited.as_ref().and_then(|scope| scope.route_id.clone())),
            viewport_bounds: inherited.as_ref().and_then(|scope| scope.viewport_bounds),
            scroll_handle: self.scroll_handle.clone().or_else(|| {
                inherited
                    .as_ref()
                    .and_then(|scope| scope.scroll_handle.clone())
            }),
        };

        with_scope(scope, || {
            self.child.paint(window, cx);
        });
    }
}

pub(super) struct CaptureSubstoryElement {
    pub(super) route_key: String,
    pub(super) child: AnyElement,
}

impl IntoElement for CaptureSubstoryElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for CaptureSubstoryElement {
    type RequestLayoutState = LayoutId;
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let scope = current_scope().and_then(|scope| {
            let story_key = scope.story_key.clone()?;
            Some(CaptureScope {
                route_id: Some(capture_substory_route_id_with_key(
                    &story_key,
                    &self.route_key,
                )),
                story_key: Some(story_key),
                ..scope
            })
        });
        let layout_id = if let Some(scope) = scope {
            with_scope(scope, || self.child.request_layout(window, cx))
        } else {
            self.child.request_layout(window, cx)
        };
        (layout_id, layout_id)
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        if let Some(scope) = current_scope()
            && let Some(story_key) = scope.story_key.clone()
        {
            let route_id = capture_substory_route_id_with_key(story_key, &self.route_key);
            clear_route_automation_values(&scope.regions, &route_id);
            record_region(route_id.clone(), bounds, &scope, window.viewport_size());
            with_scope(
                CaptureScope {
                    route_id: Some(route_id),
                    ..scope
                },
                || {
                    self.child.prepaint(window, cx);
                },
            );
            return;
        }

        self.child.prepaint(window, cx);
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        _prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let scope = current_scope().and_then(|scope| {
            let story_key = scope.story_key.clone()?;
            Some(CaptureScope {
                route_id: Some(capture_substory_route_id_with_key(
                    story_key,
                    &self.route_key,
                )),
                ..scope
            })
        });
        if let Some(scope) = scope {
            with_scope(scope, || self.child.paint(window, cx));
        } else {
            self.child.paint(window, cx);
        }
    }
}

pub(super) struct InteractionTargetElement {
    pub(super) key: String,
    pub(super) label: String,
    pub(super) child: AnyElement,
}

impl IntoElement for InteractionTargetElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for InteractionTargetElement {
    type RequestLayoutState = LayoutId;
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let layout_id = self.child.request_layout(window, cx);
        (layout_id, layout_id)
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        if let Some(scope) = current_scope()
            && let Some(route_id) = scope.route_id
        {
            record_interaction_target(
                &scope.regions,
                route_id,
                self.key.clone(),
                self.label.clone(),
                bounds,
            );
        }
        self.child.prepaint(window, cx);
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        _prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.child.paint(window, cx);
    }
}

pub(super) struct SemanticValueElement {
    pub(super) key: String,
    pub(super) label: String,
    pub(super) value: Value,
    pub(super) child: AnyElement,
}

impl IntoElement for SemanticValueElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for SemanticValueElement {
    type RequestLayoutState = LayoutId;
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let layout_id = self.child.request_layout(window, cx);
        (layout_id, layout_id)
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        if let Some(scope) = current_scope()
            && let Some(route_id) = scope.route_id
        {
            record_semantic_value(
                &scope.regions,
                route_id,
                self.key.clone(),
                self.label.clone(),
                self.value.clone(),
            );
        }
        self.child.prepaint(window, cx);
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        _prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.child.paint(window, cx);
    }
}
