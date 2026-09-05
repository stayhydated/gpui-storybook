//! GPUI Inspector metadata for Storybook story roots.

use crate::story::StoryContainer;
use gpui_kit::component::{ActiveTheme as _, v_flex};
use gpui_kit::{
    AnyElement, App, Bounds, Element, ElementId, GlobalElementId, InspectorElementId, IntoElement,
    LayoutId, ParentElement as _, Pixels, Styled as _, Window,
};
use std::panic::Location;

/// Storybook-specific metadata displayed when a story root is inspected.
#[derive(Clone, Debug)]
pub struct StoryInspectorState {
    pub key: String,
    pub title: String,
    pub source: String,
    pub controls: Vec<String>,
}

impl StoryInspectorState {
    pub fn from_container(story: &StoryContainer, cx: &App) -> Self {
        Self {
            key: story.story_key_label().unwrap_or("unregistered").to_owned(),
            title: story.display_title(cx),
            source: format!(
                "{}:{}",
                story.source_file_label().unwrap_or("unknown source"),
                story.source_line().unwrap_or_default()
            ),
            controls: story
                .control_target()
                .map(|target| target.specs().iter().map(|spec| spec.key.clone()).collect())
                .unwrap_or_default(),
        }
    }
}

pub fn init(cx: &mut App) {
    cx.register_inspector_element(|_, state: &StoryInspectorState, _, cx| {
        v_flex()
            .p_3()
            .gap_2()
            .border_t_1()
            .border_color(cx.theme().border)
            .child("Storybook")
            .child(format!("Key: {}", state.key))
            .child(format!("Title: {}", state.title))
            .child(format!("Source: {}", state.source))
            .child(if state.controls.is_empty() {
                "Controls: none".to_owned()
            } else {
                format!("Controls: {}", state.controls.join(", "))
            })
    });
}

/// Wrap a story root with custom metadata for GPUI Inspector.
#[track_caller]
pub fn inspectable_story(
    state: StoryInspectorState,
    child: impl IntoElement,
) -> InspectableStoryElement {
    InspectableStoryElement {
        id: format!("storybook-inspectable-{}", state.key).into(),
        source_location: Location::caller(),
        state,
        child: child.into_any_element(),
    }
}

pub struct InspectableStoryElement {
    id: ElementId,
    source_location: &'static Location<'static>,
    state: StoryInspectorState,
    child: AnyElement,
}

impl Element for InspectableStoryElement {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        Some(self.id.clone())
    }

    fn source_location(&self) -> Option<&'static Location<'static>> {
        Some(self.source_location)
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        window.with_inspector_state(
            inspector_id,
            cx,
            |state: &mut Option<StoryInspectorState>, _| {
                *state = Some(self.state.clone());
            },
        );
        (self.child.request_layout(window, cx), ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        self.child.prepaint(window, cx);
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        _: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.child.paint(window, cx);
    }
}

impl IntoElement for InspectableStoryElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}
