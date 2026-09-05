use super::*;
use gpui_kit::component::button::Button;
use gpui_kit::{ScrollHandle, div, point, px, size};

fn clear_registry() {
    CAPTURE_REGIONS.with_borrow_mut(|registry| *registry = CaptureRegionRegistry::default());
}

#[test]
fn resetting_one_story_drops_its_stale_routes_and_keeps_other_stories() {
    clear_registry();
    let bounds = Bounds {
        origin: point(px(0.), px(0.)),
        size: size(px(100.), px(50.)),
    };
    let story_a_scope = CaptureScope {
        story_key: Some("story-a".to_owned()),
        route_id: Some("story-a".to_owned()),
        viewport_bounds: Some(bounds),
        scroll_handle: None,
    };
    let story_b_scope = CaptureScope {
        story_key: Some("story-b".to_owned()),
        route_id: Some("story-b".to_owned()),
        viewport_bounds: Some(bounds),
        scroll_handle: None,
    };
    record_region("story-a".to_owned(), bounds, &story_a_scope);
    record_region("story-a/old".to_owned(), bounds, &story_a_scope);
    record_semantic_value(
        "story-a/old".to_owned(),
        "status".to_owned(),
        "Status".to_owned(),
        serde_json::json!("stale"),
    );
    record_region("story-b".to_owned(), bounds, &story_b_scope);
    record_semantic_value(
        "story-b".to_owned(),
        "status".to_owned(),
        "Status".to_owned(),
        serde_json::json!("current"),
    );

    reset_capture_regions_for_story("story-a");

    assert!(capture_region_bounds("story-a").is_none());
    assert!(capture_region_bounds("story-a/old").is_none());
    assert!(matches!(
        semantic_values("story-a/old"),
        Err(SemanticValueLookupError::RouteNotRendered)
    ));
    assert!(capture_region_bounds("story-b").is_some());
    assert_eq!(
        semantic_values("story-b")
            .expect("unrelated story values remain")
            .len(),
        1
    );
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
        size: gpui_kit::size(px(100.), px(50.)),
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
            size: gpui_kit::size(px(100.), px(100.)),
        }),
        scroll_handle: Some(handle.clone()),
    };
    record_region(
        "story-key/section".to_string(),
        Bounds {
            origin: point(px(50.), px(70.)),
            size: gpui_kit::size(px(40.), px(20.)),
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

    let story_without_scroll = capture_story_view_with_scroll("story", None, div()).into_element();
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
        size: gpui_kit::size(px(200.), px(100.)),
    };
    record_region("story-key".to_string(), route_bounds, &scope);
    record_interaction_target(
        "story-key".to_string(),
        "second".to_string(),
        "Second".to_string(),
        Bounds {
            origin: point(px(40.), px(50.)),
            size: gpui_kit::size(px(60.), px(20.)),
        },
    );
    record_interaction_target(
        "story-key".to_string(),
        "first".to_string(),
        "First".to_string(),
        Bounds {
            origin: point(px(20.), px(30.)),
            size: gpui_kit::size(px(30.), px(10.)),
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
        size: gpui_kit::size(px(10.), px(10.)),
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

#[cfg(feature = "capture")]
#[test]
fn route_image_crop_scales_logical_bounds_to_physical_pixels() {
    clear_registry();
    let scope = CaptureScope {
        story_key: Some("story-key".to_owned()),
        route_id: Some("story-key/details".to_owned()),
        viewport_bounds: None,
        scroll_handle: None,
    };
    record_region(
        "story-key/details".to_owned(),
        Bounds {
            origin: point(px(10.), px(5.)),
            size: gpui_kit::size(px(20.), px(10.)),
        },
        &scope,
    );
    let image = image::RgbaImage::from_pixel(200, 100, image::Rgba([1, 2, 3, 255]));

    let cropped = crop_capture_region_image(
        "story-key/details",
        image,
        gpui_kit::size(px(100.), px(50.)),
    )
    .expect("registered route should crop");

    assert_eq!(cropped.dimensions(), (40, 20));
}

#[cfg(feature = "capture")]
#[test]
fn route_image_crop_rejects_an_unrendered_route() {
    clear_registry();
    let error = crop_capture_region_image(
        "story-key/missing",
        image::RgbaImage::new(10, 10),
        gpui_kit::size(px(10.), px(10.)),
    )
    .expect_err("missing route should fail");

    assert_eq!(
        error,
        CaptureRegionImageError::RouteNotRendered {
            route_id: "story-key/missing".to_owned(),
        }
    );
}
