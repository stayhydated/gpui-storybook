use gpui::{
    AnyElement, App, Bounds, Element, ElementId, GlobalElementId, InspectorElementId, IntoElement,
    LayoutId, Pixels, ScrollHandle, SharedString, Window, point,
};
use std::{cell::RefCell, collections::BTreeMap};

#[derive(Clone)]
pub(crate) struct CaptureRegionBounds {
    pub bounds: Bounds<Pixels>,
    pub viewport_bounds: Bounds<Pixels>,
    pub scroll_handle: Option<ScrollHandle>,
}

#[derive(Clone)]
struct CaptureScope {
    story_key: Option<String>,
    viewport_bounds: Option<Bounds<Pixels>>,
    scroll_handle: Option<ScrollHandle>,
}

#[derive(Default)]
struct CaptureRegionRegistry {
    scopes: Vec<CaptureScope>,
    regions: BTreeMap<String, CaptureRegionBounds>,
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
        let scope = CaptureScope {
            story_key: self.story_key.clone(),
            viewport_bounds: None,
            scroll_handle: self.scroll_handle.clone(),
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
        let scope = CaptureScope {
            story_key: self.story_key.clone(),
            viewport_bounds: Some(bounds),
            scroll_handle: self.scroll_handle.clone(),
        };

        if let Some(story_key) = self.story_key.clone() {
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
        let scope = CaptureScope {
            story_key: self.story_key.clone(),
            viewport_bounds: current_scope().and_then(|scope| scope.viewport_bounds),
            scroll_handle: self.scroll_handle.clone(),
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
            && let Some(story_key) = scope.story_key.clone()
        {
            record_region(
                capture_substory_route_id_with_key(story_key, &self.route_key),
                bounds,
                &scope,
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
    }
}
