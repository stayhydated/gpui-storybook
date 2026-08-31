use super::*;

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

#[cfg(test)]
pub(crate) fn current_capture_scroll_handle() -> Option<ScrollHandle> {
    current_scope().and_then(|scope| scope.scroll_handle)
}

/// Scrolls the nearest registered story viewport so `route_id` becomes visible.
///
/// Returns `false` when the route has not registered bounds during the latest
/// frame. Portable and live capture runners should request another frame after
/// this returns `true` before cropping the route image.
pub fn scroll_capture_region_into_view(route_id: &str) -> bool {
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

pub(super) fn current_scope() -> Option<CaptureScope> {
    CAPTURE_REGIONS.with_borrow(|registry| registry.scopes.last().cloned())
}

pub(super) fn with_scope<R>(scope: CaptureScope, f: impl FnOnce() -> R) -> R {
    CAPTURE_REGIONS.with_borrow_mut(|registry| registry.scopes.push(scope));
    let result = f();
    CAPTURE_REGIONS.with_borrow_mut(|registry| {
        registry.scopes.pop();
    });
    result
}

pub(super) fn record_region(route_id: String, bounds: Bounds<Pixels>, scope: &CaptureScope) {
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

pub(super) fn clear_route_automation_values(route_id: &str) {
    CAPTURE_REGIONS.with_borrow_mut(|registry| {
        registry.interaction_targets.remove(route_id);
        registry.duplicate_interaction_targets.remove(route_id);
        registry.semantic_values.remove(route_id);
        registry.duplicate_semantic_values.remove(route_id);
    });
}

pub(super) fn record_semantic_value(route_id: String, key: String, label: String, value: Value) {
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

pub(super) fn record_interaction_target(
    route_id: String,
    key: String,
    label: String,
    bounds: Bounds<Pixels>,
) {
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
