use gpui::{
    AnyElement, App, Bounds, Element, ElementId, GlobalElementId, InspectorElementId,
    InteractiveElement, IntoElement, LayoutId, Pixels, ScrollHandle, SharedString, Window, point,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet},
};

/// Logical bounds of a semantic interaction target relative to its story route.
#[derive(Clone, Copy, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StoryInteractionTargetBounds {
    /// Horizontal offset from the route origin in logical pixels.
    pub x: f32,
    /// Vertical offset from the route origin in logical pixels.
    pub y: f32,
    /// Target width in logical pixels.
    pub width: f32,
    /// Target height in logical pixels.
    pub height: f32,
}

/// One stable semantic interaction target rendered by a story route.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StoryInteractionTargetSnapshot {
    /// Stable target key supplied by the story author.
    pub key: String,
    /// Human-readable target label supplied by the story author.
    pub label: String,
    /// Target bounds in logical pixels relative to the active route.
    pub bounds: StoryInteractionTargetBounds,
}

/// One stable machine-readable value rendered by a story route.
#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StorySemanticValueSnapshot {
    /// Stable value key supplied by the story author.
    pub key: String,
    /// Human-readable value label supplied by the story author.
    pub label: String,
    /// Current JSON value captured from application state during rendering.
    pub value: Value,
}

#[derive(Clone)]
pub(crate) struct CaptureRegionBounds {
    pub bounds: Bounds<Pixels>,
    pub viewport_bounds: Bounds<Pixels>,
    pub scroll_handle: Option<ScrollHandle>,
}

#[derive(Clone)]
struct CaptureScope {
    story_key: Option<String>,
    route_id: Option<String>,
    viewport_bounds: Option<Bounds<Pixels>>,
    scroll_handle: Option<ScrollHandle>,
}

#[derive(Clone)]
struct InteractionTargetRecord {
    label: String,
    bounds: Bounds<Pixels>,
}

#[derive(Clone)]
struct SemanticValueRecord {
    label: String,
    value: Value,
}

#[derive(Default)]
struct CaptureRegionRegistry {
    scopes: Vec<CaptureScope>,
    regions: BTreeMap<String, CaptureRegionBounds>,
    interaction_targets: BTreeMap<String, BTreeMap<String, InteractionTargetRecord>>,
    duplicate_interaction_targets: BTreeMap<String, BTreeSet<String>>,
    semantic_values: BTreeMap<String, BTreeMap<String, SemanticValueRecord>>,
    duplicate_semantic_values: BTreeMap<String, BTreeSet<String>>,
}

thread_local! {
    static CAPTURE_REGIONS: RefCell<CaptureRegionRegistry> = RefCell::default();
}

/// Wrap a story viewport so screenshot capture can crop to that viewport.
///
/// Applications normally get this through [`StoryContainer`](crate::story::StoryContainer).
pub fn capture_story_view(
    story_key: impl Into<String>,
    scroll_handle: ScrollHandle,
    child: impl IntoElement,
) -> impl IntoElement {
    capture_story_view_with_scroll(story_key, Some(scroll_handle), child)
}

pub(crate) fn capture_story_view_with_scroll(
    story_key: impl Into<String>,
    scroll_handle: Option<ScrollHandle>,
    child: impl IntoElement,
) -> impl IntoElement {
    CaptureScopeElement {
        story_key: Some(story_key.into()),
        scroll_handle,
        child: child.into_any_element(),
    }
}

/// Fluent Storybook automation metadata for GPUI elements.
///
/// Import this trait as `_`, then call [`storybook_target`](Self::storybook_target)
/// or [`storybook_value`](Self::storybook_value) on an interactive element with
/// a stable GPUI ID. Storybook uses the displayed [`ElementId`] as the
/// route-local key and derives a human-readable label from it:
///
/// ```no_run
/// use gpui::{InteractiveElement as _, div};
/// use gpui_storybook_core::capture_region::StorybookElementExt as _;
///
/// let button = div().id("execute-request").storybook_target();
/// let response = div()
///     .id("response")
///     .storybook_value(serde_json::json!({ "status": "success" }));
/// ```
///
/// Use [`storybook_target_as`](Self::storybook_target_as) or
/// [`storybook_value_as`](Self::storybook_value_as) when the rendered type does
/// not expose [`InteractiveElement`], or when the stable key and display label
/// need to differ from its GPUI ID.
pub trait StorybookElementExt: IntoElement {
    /// Mark this element as a stable semantic target for Storybook automation.
    ///
    /// The element's GPUI ID becomes the route-local key. Separators in that ID
    /// become spaces in the inferred label. The target is exposed by MCP only
    /// when generic interaction tools are explicitly enabled.
    #[track_caller]
    fn storybook_target(mut self) -> impl IntoElement
    where
        Self: InteractiveElement,
    {
        let (key, label) = implicit_automation_identity(&mut self);
        self.storybook_target_as(key, label)
    }

    /// Mark this element as a semantic target with an explicit key and label.
    ///
    /// Keys must be unique within a story or substory route.
    fn storybook_target_as(
        self,
        key: impl Into<String>,
        label: impl Into<String>,
    ) -> impl IntoElement {
        InteractionTargetElement {
            key: key.into(),
            label: label.into(),
            child: self.into_any_element(),
        }
    }

    /// Mark this element as the source of a stable machine-readable value.
    ///
    /// The element's GPUI ID becomes the route-local key. Values are refreshed
    /// from application state during prepaint and exposed by Storybook
    /// automation for the active story or substory route.
    #[track_caller]
    fn storybook_value(mut self, value: Value) -> impl IntoElement
    where
        Self: InteractiveElement,
    {
        let (key, label) = implicit_automation_identity(&mut self);
        self.storybook_value_as(key, label, value)
    }

    /// Mark this element as a machine-readable value with an explicit key and label.
    ///
    /// Keys must be unique within a story or substory route.
    fn storybook_value_as(
        self,
        key: impl Into<String>,
        label: impl Into<String>,
        value: Value,
    ) -> impl IntoElement {
        SemanticValueElement {
            key: key.into(),
            label: label.into(),
            value,
            child: self.into_any_element(),
        }
    }
}

impl<T: IntoElement> StorybookElementExt for T {}

#[track_caller]
fn implicit_automation_identity(element: &mut impl InteractiveElement) -> (String, String) {
    let id = element
        .interactivity()
        .element_id
        .as_ref()
        .expect("implicit Storybook automation metadata requires a GPUI element ID; assign an ID or use the explicit `_as` method")
        .to_string();
    assert!(
        !id.is_empty(),
        "implicit Storybook automation metadata requires a non-empty GPUI element ID; assign a non-empty ID or use the explicit `_as` method"
    );
    let label = automation_label(&id);
    (id, label)
}

fn automation_label(key: &str) -> String {
    let mut label = String::with_capacity(key.len());
    let mut previous_was_separator = false;

    for ch in key.chars() {
        if ch.is_alphanumeric() {
            if previous_was_separator && !label.is_empty() {
                label.push(' ');
            }
            label.push(ch);
            previous_was_separator = false;
        } else {
            previous_was_separator = true;
        }
    }

    if let Some(first) = label.chars().next() {
        let uppercase = first.to_uppercase().to_string();
        label.replace_range(..first.len_utf8(), &uppercase);
    }

    label
}

pub(crate) fn capture_scroll_scope(
    scroll_handle: ScrollHandle,
    child: impl IntoElement,
) -> impl IntoElement {
    CaptureScopeElement {
        story_key: None,
        scroll_handle: Some(scroll_handle),
        child: child.into_any_element(),
    }
}

/// Wrap a section inside a story so it can be captured as `story-key/section-slug`.
///
/// The standard styled [`section`](crate::story::section) helper and
/// [`StorySectionBase::capture`](crate::story::StorySectionBase::capture) use
/// this automatically.
pub fn capture_substory(
    title: impl Into<SharedString>,
    child: impl IntoElement,
) -> impl IntoElement {
    let title = title.into();

    CaptureSubstoryElement {
        route_key: capture_route_slug(title),
        child: child.into_any_element(),
    }
}

/// Wrap a section inside a story with an explicit stable capture key.
///
/// This is useful when the visible section title can change independently from
/// automation and capture routes.
pub fn capture_substory_with_key(
    key: impl AsRef<str>,
    child: impl IntoElement,
) -> impl IntoElement {
    CaptureSubstoryElement {
        route_key: capture_route_slug(key),
        child: child.into_any_element(),
    }
}

/// Build the capture route id for a story section title.
pub fn capture_substory_route_id(story_key: impl AsRef<str>, title: impl AsRef<str>) -> String {
    capture_substory_route_id_with_key(story_key, capture_route_slug(title))
}

/// Build the capture route id for an explicit sub-story key.
pub fn capture_substory_route_id_with_key(
    story_key: impl AsRef<str>,
    key: impl AsRef<str>,
) -> String {
    format!(
        "{}/{}",
        story_key.as_ref(),
        capture_route_slug(key.as_ref())
    )
}

/// Convert a section title into the slug used by sub-story capture routes.
pub fn capture_route_slug(title: impl AsRef<str>) -> String {
    let mut slug = String::new();
    let mut needs_separator = false;

    for ch in title.as_ref().chars() {
        if ch.is_ascii_alphanumeric() {
            if needs_separator && !slug.is_empty() {
                slug.push('-');
            }
            slug.push(ch.to_ascii_lowercase());
            needs_separator = false;
        } else {
            needs_separator = true;
        }
    }

    if slug.is_empty() {
        "section".to_string()
    } else {
        slug
    }
}

pub(crate) fn capture_route_story_key(route_id: &str) -> &str {
    route_id
        .split_once('/')
        .map_or(route_id, |(story_key, _)| story_key)
}

pub(crate) fn capture_region_bounds(route_id: &str) -> Option<CaptureRegionBounds> {
    CAPTURE_REGIONS.with_borrow(|registry| registry.regions.get(route_id).cloned())
}

#[derive(Debug)]
pub(crate) enum InteractionTargetLookupError {
    RouteNotRendered,
    DuplicateKey(String),
}

#[derive(Debug)]
pub(crate) enum SemanticValueLookupError {
    RouteNotRendered,
    DuplicateKey(String),
}

pub(crate) fn interaction_targets(
    route_id: &str,
) -> Result<Vec<StoryInteractionTargetSnapshot>, InteractionTargetLookupError> {
    CAPTURE_REGIONS.with_borrow(|registry| {
        let Some(region) = registry.regions.get(route_id) else {
            return Err(InteractionTargetLookupError::RouteNotRendered);
        };
        if let Some(key) = registry
            .duplicate_interaction_targets
            .get(route_id)
            .and_then(|keys| keys.first())
        {
            return Err(InteractionTargetLookupError::DuplicateKey(key.clone()));
        }

        let route_origin = region.bounds.origin;
        Ok(registry
            .interaction_targets
            .get(route_id)
            .into_iter()
            .flat_map(|targets| targets.iter())
            .map(|(key, target)| StoryInteractionTargetSnapshot {
                key: key.clone(),
                label: target.label.clone(),
                bounds: StoryInteractionTargetBounds {
                    x: f32::from(target.bounds.origin.x - route_origin.x),
                    y: f32::from(target.bounds.origin.y - route_origin.y),
                    width: f32::from(target.bounds.size.width),
                    height: f32::from(target.bounds.size.height),
                },
            })
            .collect())
    })
}

pub(crate) fn semantic_values(
    route_id: &str,
) -> Result<Vec<StorySemanticValueSnapshot>, SemanticValueLookupError> {
    CAPTURE_REGIONS.with_borrow(|registry| {
        if !registry.regions.contains_key(route_id) {
            return Err(SemanticValueLookupError::RouteNotRendered);
        }
        if let Some(key) = registry
            .duplicate_semantic_values
            .get(route_id)
            .and_then(|keys| keys.first())
        {
            return Err(SemanticValueLookupError::DuplicateKey(key.clone()));
        }

        Ok(registry
            .semantic_values
            .get(route_id)
            .into_iter()
            .flat_map(|values| values.iter())
            .map(|(key, value)| StorySemanticValueSnapshot {
                key: key.clone(),
                label: value.label.clone(),
                value: value.value.clone(),
            })
            .collect())
    })
}

pub(crate) fn current_capture_scroll_handle() -> Option<ScrollHandle> {
    current_scope().and_then(|scope| scope.scroll_handle)
}

pub(crate) fn scroll_capture_region_into_view(route_id: &str) -> bool {
    let Some(region) = capture_region_bounds(route_id) else {
        return false;
    };
    let Some(scroll_handle) = region.scroll_handle else {
        return true;
    };

    let offset = scroll_handle.offset();
    let viewport = region.viewport_bounds;
    let bounds = region.bounds;

    scroll_handle.set_offset(point(
        offset.x + viewport.origin.x - bounds.origin.x,
        offset.y + viewport.origin.y - bounds.origin.y,
    ));

    true
}

fn current_scope() -> Option<CaptureScope> {
    CAPTURE_REGIONS.with_borrow(|registry| registry.scopes.last().cloned())
}

fn with_scope<R>(scope: CaptureScope, f: impl FnOnce() -> R) -> R {
    CAPTURE_REGIONS.with_borrow_mut(|registry| registry.scopes.push(scope));
    let result = f();
    CAPTURE_REGIONS.with_borrow_mut(|registry| {
        registry.scopes.pop();
    });
    result
}

fn record_region(route_id: String, bounds: Bounds<Pixels>, scope: &CaptureScope) {
    let viewport_bounds = scope.viewport_bounds.unwrap_or(bounds);

    CAPTURE_REGIONS.with_borrow_mut(|registry| {
        registry.regions.insert(
            route_id,
            CaptureRegionBounds {
                bounds,
                viewport_bounds,
                scroll_handle: scope.scroll_handle.clone(),
            },
        );
    });
}

fn clear_route_automation_values(route_id: &str) {
    CAPTURE_REGIONS.with_borrow_mut(|registry| {
        registry.interaction_targets.remove(route_id);
        registry.duplicate_interaction_targets.remove(route_id);
        registry.semantic_values.remove(route_id);
        registry.duplicate_semantic_values.remove(route_id);
    });
}

fn record_semantic_value(route_id: String, key: String, label: String, value: Value) {
    CAPTURE_REGIONS.with_borrow_mut(|registry| {
        let duplicate = match registry
            .semantic_values
            .entry(route_id.clone())
            .or_default()
            .entry(key.clone())
        {
            std::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert(SemanticValueRecord { label, value });
                false
            },
            std::collections::btree_map::Entry::Occupied(_) => true,
        };
        if duplicate {
            registry
                .duplicate_semantic_values
                .entry(route_id)
                .or_default()
                .insert(key);
        }
    });
}

fn record_interaction_target(route_id: String, key: String, label: String, bounds: Bounds<Pixels>) {
    CAPTURE_REGIONS.with_borrow_mut(|registry| {
        let duplicate = match registry
            .interaction_targets
            .entry(route_id.clone())
            .or_default()
            .entry(key.clone())
        {
            std::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert(InteractionTargetRecord { label, bounds });
                false
            },
            std::collections::btree_map::Entry::Occupied(_) => true,
        };
        if duplicate {
            registry
                .duplicate_interaction_targets
                .entry(route_id)
                .or_default()
                .insert(key);
        }
    });
}

struct CaptureScopeElement {
    story_key: Option<String>,
    scroll_handle: Option<ScrollHandle>,
    child: AnyElement,
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
        let inherited = current_scope();
        let story_key = self
            .story_key
            .clone()
            .or_else(|| inherited.as_ref().and_then(|scope| scope.story_key.clone()));
        let scope = CaptureScope {
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
            story_key,
            route_id,
            viewport_bounds: Some(bounds),
            scroll_handle: self.scroll_handle.clone().or_else(|| {
                inherited
                    .as_ref()
                    .and_then(|scope| scope.scroll_handle.clone())
            }),
        };

        if let Some(story_key) = self.story_key.clone() {
            clear_route_automation_values(&story_key);
            record_region(story_key, bounds, &scope);
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
        let inherited = current_scope();
        let scope = CaptureScope {
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

struct CaptureSubstoryElement {
    route_key: String,
    child: AnyElement,
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
            clear_route_automation_values(&route_id);
            record_region(route_id.clone(), bounds, &scope);
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

struct InteractionTargetElement {
    key: String,
    label: String,
    child: AnyElement,
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
        if let Some(route_id) = current_scope().and_then(|scope| scope.route_id) {
            record_interaction_target(route_id, self.key.clone(), self.label.clone(), bounds);
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

struct SemanticValueElement {
    key: String,
    label: String,
    value: Value,
    child: AnyElement,
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
        if let Some(route_id) = current_scope().and_then(|scope| scope.route_id) {
            record_semantic_value(
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

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{ScrollHandle, div, point, px};
    use gpui_component::button::Button;

    fn clear_registry() {
        CAPTURE_REGIONS.with_borrow_mut(|registry| *registry = CaptureRegionRegistry::default());
    }

    #[test]
    fn substory_route_id_slugs_titles() {
        assert_eq!(
            capture_substory_route_id("story-key", "Button with Icon"),
            "story-key/button-with-icon"
        );
    }

    #[test]
    fn substory_route_id_accepts_explicit_stable_keys() {
        assert_eq!(
            capture_substory_route_id_with_key("story-key", "button-with-icon"),
            "story-key/button-with-icon"
        );
    }

    #[test]
    fn route_slugs_normalize_separators_and_blank_titles() {
        assert_eq!(
            capture_route_slug("  Button___With ICON!  "),
            "button-with-icon"
        );
        assert_eq!(capture_route_slug("123 Ready"), "123-ready");
        assert_eq!(capture_route_slug("💧"), "section");
        assert_eq!(capture_route_story_key("story-key/section"), "story-key");
        assert_eq!(capture_route_story_key("story-key"), "story-key");
    }

    #[test]
    fn scopes_restore_previous_state_and_record_region_bounds() {
        clear_registry();
        let outer = CaptureScope {
            story_key: Some("story-key".to_string()),
            route_id: Some("story-key".to_string()),
            viewport_bounds: None,
            scroll_handle: None,
        };
        let bounds = Bounds {
            origin: point(px(10.), px(20.)),
            size: gpui::size(px(100.), px(50.)),
        };

        assert!(current_scope().is_none());
        with_scope(outer, || {
            assert_eq!(
                current_scope().and_then(|scope| scope.story_key),
                Some("story-key".to_string())
            );
            let scope = current_scope().expect("scope should be active");
            record_region("story-key".to_string(), bounds, &scope);
        });
        assert!(current_scope().is_none());

        let recorded = capture_region_bounds("story-key").expect("region should be recorded");
        assert_eq!(recorded.bounds, bounds);
        assert_eq!(recorded.viewport_bounds, bounds);
        assert!(recorded.scroll_handle.is_none());
        assert!(scroll_capture_region_into_view("story-key"));
        assert!(!scroll_capture_region_into_view("missing"));
    }

    #[test]
    fn scrolling_recorded_region_aligns_it_with_viewport() {
        clear_registry();
        let handle = ScrollHandle::new();
        handle.set_offset(point(px(5.), px(10.)));
        let scope = CaptureScope {
            story_key: Some("story-key".to_string()),
            route_id: Some("story-key".to_string()),
            viewport_bounds: Some(Bounds {
                origin: point(px(20.), px(30.)),
                size: gpui::size(px(100.), px(100.)),
            }),
            scroll_handle: Some(handle.clone()),
        };
        record_region(
            "story-key/section".to_string(),
            Bounds {
                origin: point(px(50.), px(70.)),
                size: gpui::size(px(40.), px(20.)),
            },
            &scope,
        );

        assert!(scroll_capture_region_into_view("story-key/section"));
        assert_eq!(handle.offset(), point(px(-25.), px(-30.)));

        with_scope(scope, || {
            let active = current_capture_scroll_handle().expect("scroll handle should be exposed");
            assert_eq!(active.offset(), point(px(-25.), px(-30.)));
        });
        assert!(current_capture_scroll_handle().is_none());
    }

    #[test]
    fn capture_wrappers_are_anonymous_elements() {
        let scroll = ScrollHandle::new();
        let story = capture_story_view("story", scroll.clone(), div()).into_element();
        assert!(story.id().is_none());
        assert!(story.source_location().is_none());

        let story_without_scroll =
            capture_story_view_with_scroll("story", None, div()).into_element();
        assert!(story_without_scroll.id().is_none());

        let scroll_scope = capture_scroll_scope(scroll, div()).into_element();
        assert!(scroll_scope.source_location().is_none());

        let substory = capture_substory("With Icon", div()).into_element();
        assert!(substory.id().is_none());
        assert!(substory.source_location().is_none());

        let keyed_substory = capture_substory_with_key("stable-key", div()).into_element();
        assert!(keyed_substory.id().is_none());

        let target = div()
            .storybook_target_as("execute", "Execute")
            .into_element();
        assert!(target.id().is_none());
        assert!(target.source_location().is_none());

        let value = div()
            .storybook_value_as("response", "Response", serde_json::json!(42))
            .into_element();
        assert!(value.id().is_none());
        assert!(value.source_location().is_none());
    }

    #[test]
    fn automation_identity_uses_element_id_and_humanizes_its_label() {
        let mut element = div().id("execute-request");
        let mut button = Button::new("submit-order").label("Submit");

        assert_eq!(
            implicit_automation_identity(&mut element),
            ("execute-request".to_owned(), "Execute request".to_owned())
        );
        assert_eq!(
            implicit_automation_identity(&mut button),
            ("submit-order".to_owned(), "Submit order".to_owned())
        );
        assert_eq!(
            automation_label("fixture_state.value"),
            "Fixture state value"
        );
        assert_eq!(automation_label("MCP2-response"), "MCP2 response");
    }

    #[test]
    #[should_panic(expected = "implicit Storybook automation metadata requires a GPUI element ID")]
    fn implicit_automation_identity_requires_an_element_id() {
        let _ = div().storybook_target();
    }

    #[test]
    fn interaction_targets_are_relative_to_route_and_sorted_by_key() {
        clear_registry();
        let scope = CaptureScope {
            story_key: Some("story-key".to_string()),
            route_id: Some("story-key".to_string()),
            viewport_bounds: None,
            scroll_handle: None,
        };
        let route_bounds = Bounds {
            origin: point(px(10.), px(20.)),
            size: gpui::size(px(200.), px(100.)),
        };
        record_region("story-key".to_string(), route_bounds, &scope);
        record_interaction_target(
            "story-key".to_string(),
            "second".to_string(),
            "Second".to_string(),
            Bounds {
                origin: point(px(40.), px(50.)),
                size: gpui::size(px(60.), px(20.)),
            },
        );
        record_interaction_target(
            "story-key".to_string(),
            "first".to_string(),
            "First".to_string(),
            Bounds {
                origin: point(px(20.), px(30.)),
                size: gpui::size(px(30.), px(10.)),
            },
        );

        let targets = interaction_targets("story-key").expect("targets should resolve");
        assert_eq!(targets[0].key, "first");
        assert_eq!(targets[0].bounds.x, 10.0);
        assert_eq!(targets[0].bounds.y, 10.0);
        assert_eq!(targets[1].key, "second");
    }

    #[test]
    fn duplicate_interaction_target_keys_are_rejected() {
        clear_registry();
        let scope = CaptureScope {
            story_key: Some("story-key".to_string()),
            route_id: Some("story-key".to_string()),
            viewport_bounds: None,
            scroll_handle: None,
        };
        let bounds = Bounds {
            origin: point(px(0.), px(0.)),
            size: gpui::size(px(10.), px(10.)),
        };
        record_region("story-key".to_string(), bounds, &scope);
        record_interaction_target(
            "story-key".to_string(),
            "duplicate".to_string(),
            "First".to_string(),
            bounds,
        );
        record_interaction_target(
            "story-key".to_string(),
            "duplicate".to_string(),
            "Second".to_string(),
            bounds,
        );

        assert!(matches!(
            interaction_targets("story-key"),
            Err(InteractionTargetLookupError::DuplicateKey(key)) if key == "duplicate"
        ));
    }

    #[test]
    fn semantic_values_are_structured_and_sorted_by_key() {
        clear_registry();
        let scope = CaptureScope {
            story_key: Some("story-key".to_string()),
            route_id: Some("story-key".to_string()),
            viewport_bounds: None,
            scroll_handle: None,
        };
        record_region("story-key".to_string(), Bounds::default(), &scope);
        record_semantic_value(
            "story-key".to_string(),
            "status".to_string(),
            "Status".to_string(),
            serde_json::json!({ "ready": true }),
        );
        record_semantic_value(
            "story-key".to_string(),
            "response".to_string(),
            "Response".to_string(),
            serde_json::json!({ "position": 12.5 }),
        );

        let values = semantic_values("story-key").expect("values should resolve");
        assert_eq!(values[0].key, "response");
        assert_eq!(values[0].value, serde_json::json!({ "position": 12.5 }));
        assert_eq!(values[1].key, "status");
    }

    #[test]
    fn duplicate_semantic_value_keys_are_rejected() {
        clear_registry();
        let scope = CaptureScope {
            story_key: Some("story-key".to_string()),
            route_id: Some("story-key".to_string()),
            viewport_bounds: None,
            scroll_handle: None,
        };
        record_region("story-key".to_string(), Bounds::default(), &scope);
        record_semantic_value(
            "story-key".to_string(),
            "response".to_string(),
            "First".to_string(),
            serde_json::json!(1),
        );
        record_semantic_value(
            "story-key".to_string(),
            "response".to_string(),
            "Second".to_string(),
            serde_json::json!(2),
        );

        assert!(matches!(
            semantic_values("story-key"),
            Err(SemanticValueLookupError::DuplicateKey(key)) if key == "response"
        ));
    }
}
