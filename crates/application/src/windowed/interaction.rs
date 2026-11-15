//! Clip-aware hit testing for CSS interaction states in physical coordinates.

use super::aom_css::CssScene;
use lunelle_core::{NodeId, Point, Rect};
use lunelle_css::StyleState;
use std::collections::BTreeMap;

pub(super) fn interaction_states(
    scene: &CssScene,
    point: Option<Point>,
    pressed: bool,
) -> BTreeMap<NodeId, StyleState> {
    let mut states = BTreeMap::new();
    for (index, bound) in scene.bounds.iter().enumerate() {
        let hovered = point.is_some_and(|point| {
            contains(*bound, point) && scene.clips[index].is_none_or(|clip| contains(clip, point))
        });
        if hovered {
            states.insert(
                scene._snapshot.nodes[index].id,
                StyleState {
                    hovered,
                    active: pressed,
                    ..StyleState::default()
                },
            );
        }
    }
    states
}

pub(super) fn interaction_states_changed(
    scene: &CssScene,
    point: Option<Point>,
    pressed: bool,
    previous: &BTreeMap<NodeId, StyleState>,
) -> bool {
    if previous.len() > scene.bounds.len() {
        return true;
    }
    let mut current_len = 0;
    for (index, bound) in scene.bounds.iter().enumerate() {
        let hovered = point.is_some_and(|point| {
            contains(*bound, point) && scene.clips[index].is_none_or(|clip| contains(clip, point))
        });
        if !hovered {
            continue;
        }
        current_len += 1;
        let node_id = scene._snapshot.nodes[index].id;
        let expected = StyleState {
            hovered: true,
            active: pressed,
            ..StyleState::default()
        };
        if previous.get(&node_id) != Some(&expected) {
            return true;
        }
    }
    current_len != previous.len()
}

fn contains(rect: Rect, point: Point) -> bool {
    point.x >= rect.x
        && point.x < rect.x + rect.width
        && point.y >= rect.y
        && point.y < rect.y + rect.height
}
