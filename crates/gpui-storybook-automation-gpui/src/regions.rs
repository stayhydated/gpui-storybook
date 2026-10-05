use gpui::{
    AnyElement, App, Bounds, Element, ElementId, GlobalElementId, InspectorElementId,
    InteractiveElement, IntoElement, LayoutId, Pixels, ScrollHandle, SharedString, Window, point,
};
use serde::Serialize;
use serde_json::Value;
use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet, HashMap},
    rc::Rc,
};

pub use gpui_storybook_automation::{
    StoryInteractionTargetBounds, StoryInteractionTargetSnapshot, StorySemanticValueSnapshot,
};

#[derive(Clone)]
pub struct CaptureRegionBounds {
    pub bounds: Bounds<Pixels>,
    pub viewport_bounds: Bounds<Pixels>,
    pub window_size: gpui::Size<Pixels>,
    pub scroll_handle: Option<ScrollHandle>,
}

#[derive(Clone)]
struct CaptureScope {
    regions: RenderedRegions,
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
    regions: BTreeMap<String, CaptureRegionBounds>,
    interaction_targets: BTreeMap<String, BTreeMap<String, InteractionTargetRecord>>,
    duplicate_interaction_targets: BTreeMap<String, BTreeSet<String>>,
    semantic_values: BTreeMap<String, BTreeMap<String, SemanticValueRecord>>,
    duplicate_semantic_values: BTreeMap<String, BTreeSet<String>>,
}

/// Rendered state retained by one app/window identity.
#[derive(Clone, Default)]
pub struct RenderedRegions(Rc<RefCell<CaptureRegionRegistry>>);

#[derive(Default)]
struct WindowRegions(RefCell<HashMap<gpui::WindowId, RenderedRegions>>);
impl gpui::Global for WindowRegions {}

/// Gets the registry for a window, initializing its app-owned storage.
pub fn window_regions(window: &Window, cx: &mut App) -> RenderedRegions {
    if cx.try_global::<WindowRegions>().is_none() {
        cx.set_global(WindowRegions::default());
    }
    cx.global::<WindowRegions>()
        .0
        .borrow_mut()
        .entry(window.window_handle().window_id())
        .or_default()
        .clone()
}

fn existing_regions(window: &Window, cx: &App) -> Option<RenderedRegions> {
    cx.try_global::<WindowRegions>()?
        .0
        .borrow()
        .get(&window.window_handle().window_id())
        .cloned()
}

/// Invalidates geometry and semantic registrations before replacing a surface.
/// A replacement root publishes its own fresh registry on its first prepaint.
pub fn invalidate_window_regions(window: &Window, cx: &mut App) {
    if let Some(windows) = cx.try_global::<WindowRegions>()
        && let Some(regions) = windows
            .0
            .borrow_mut()
            .remove(&window.window_handle().window_id())
    {
        *regions.0.borrow_mut() = CaptureRegionRegistry::default();
    }
}

thread_local! {
    static CAPTURE_SCOPES: RefCell<Vec<CaptureScope>> = RefCell::default();
}

/// Wrap a story viewport so screenshot capture can crop to that viewport.
///
/// Wrap the application root that owns the registered route.
pub fn capture_story_view(
    story_key: impl Into<String>,
    scroll_handle: ScrollHandle,
    child: impl IntoElement,
) -> impl IntoElement {
    capture_story_view_with_scroll(story_key, Some(scroll_handle), child)
}

pub fn capture_story_view_with_scroll(
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
/// use gpui_storybook_automation_gpui::regions::StorybookElementExt as _;
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
    /// automation for the active story or substory route. Any Serde-serializable
    /// value can be passed directly; serialization failures are authoring errors
    /// and panic at this call site.
    #[track_caller]
    fn storybook_value(mut self, value: impl Serialize) -> impl IntoElement
    where
        Self: InteractiveElement,
    {
        let (key, label) = implicit_automation_identity(&mut self);
        self.storybook_value_as(key, label, value)
    }

    /// Mark this element as a machine-readable value with an explicit key and label.
    ///
    /// Keys must be unique within a story or substory route.
    #[track_caller]
    fn storybook_value_as(
        self,
        key: impl Into<String>,
        label: impl Into<String>,
        value: impl Serialize,
    ) -> impl IntoElement {
        SemanticValueElement {
            key: key.into(),
            label: label.into(),
            value: serde_json::to_value(value)
                .expect("Storybook semantic values must serialize as JSON"),
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

pub fn capture_scroll_scope(
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
/// The parent root supplies the story identity and owning window.
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

pub fn capture_route_story_key(route_id: &str) -> &str {
    route_id
        .split_once('/')
        .map_or(route_id, |(story_key, _)| story_key)
}

/// Reads geometry registered by this window's latest root prepaint.
pub fn capture_region_bounds(
    route_id: &str,
    window: &Window,
    cx: &App,
) -> Option<CaptureRegionBounds> {
    existing_regions(window, cx)?.bounds(route_id)
}

impl RenderedRegions {
    pub fn bounds(&self, route_id: &str) -> Option<CaptureRegionBounds> {
        self.0.borrow().regions.get(route_id).cloned()
    }

    /// Removes one root and all of its subroutes before a fresh prepaint.
    pub fn reset_story(&self, story_key: &str) {
        let mut registry = self.0.borrow_mut();
        registry
            .regions
            .retain(|route, _| capture_route_story_key(route) != story_key);
        registry
            .interaction_targets
            .retain(|route, _| capture_route_story_key(route) != story_key);
        registry
            .duplicate_interaction_targets
            .retain(|route, _| capture_route_story_key(route) != story_key);
        registry
            .semantic_values
            .retain(|route, _| capture_route_story_key(route) != story_key);
        registry
            .duplicate_semantic_values
            .retain(|route, _| capture_route_story_key(route) != story_key);
    }
}

mod automation;
mod crop;
mod elements;

#[cfg(test)]
pub use automation::current_capture_scroll_handle;
pub use automation::scroll_capture_region_into_view;
pub use automation::{
    InteractionTargetLookupError, SemanticValueLookupError, interaction_targets, semantic_values,
};
use automation::{
    clear_route_automation_values, current_scope, record_interaction_target, record_region,
    record_semantic_value, with_scope,
};
pub use crop::CaptureRegionImageError;
#[cfg(feature = "capture")]
pub use crop::crop_capture_region_image;
use elements::{
    CaptureScopeElement, CaptureSubstoryElement, InteractionTargetElement, SemanticValueElement,
};

#[cfg(test)]
mod tests;
