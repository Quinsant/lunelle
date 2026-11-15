use lunelle_core::{Point, Rect};

use crate::CssComputedState;

/// Post-layout geometry produced by CSS overflow processing.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CssOverflowLayout {
    /// Bounds translated by ancestor scroll offsets.
    pub bounds: Vec<Rect>,
    /// Inherited clipping rectangles for each node.
    pub clips: Vec<Option<Rect>>,
    /// Maximum horizontal and vertical scroll offsets for each node.
    pub max_scroll: Vec<Point>,
}

/// Applies CSS clipping and vertical scrolling to a preorder layout tree.
///
/// `children` contains direct child indices for every node. The indices must
/// refer to the same preorder arrays as `raw_bounds` and `states`.
pub fn apply_overflow_layout(
    raw_bounds: &[Rect],
    states: &[CssComputedState],
    children: &[Vec<usize>],
    scroll_offsets: &mut [Point],
) -> CssOverflowLayout {
    assert_eq!(raw_bounds.len(), states.len());
    assert_eq!(raw_bounds.len(), children.len());
    assert_eq!(raw_bounds.len(), scroll_offsets.len());

    let mut result = CssOverflowLayout {
        bounds: raw_bounds.to_vec(),
        clips: vec![None; raw_bounds.len()],
        max_scroll: vec![Point { x: 0.0, y: 0.0 }; raw_bounds.len()],
    };
    visit(
        0,
        raw_bounds,
        states,
        children,
        scroll_offsets,
        Point { x: 0.0, y: 0.0 },
        None,
        &mut result,
    );
    result
}

fn visit(
    index: usize,
    raw_bounds: &[Rect],
    states: &[CssComputedState],
    children: &[Vec<usize>],
    scroll_offsets: &mut [Point],
    parent_offset: Point,
    parent_clip: Option<Rect>,
    result: &mut CssOverflowLayout,
) {
    let mut bounds = raw_bounds[index];
    bounds.x -= parent_offset.x;
    bounds.y -= parent_offset.y;
    result.bounds[index] = bounds;
    result.clips[index] = parent_clip;

    let (_, flags) = super::taffy::taffy_style_state(&states[index]);
    let content_right = children[index]
        .iter()
        .map(|child| subtree_right(*child, raw_bounds, children))
        .fold(raw_bounds[index].x + raw_bounds[index].width, f32::max);
    let content_bottom = children[index]
        .iter()
        .map(|child| subtree_bottom(*child, raw_bounds, children))
        .fold(raw_bounds[index].y + raw_bounds[index].height, f32::max);
    let maximum = Point {
        x: if flags.scrollable {
            (content_right - (raw_bounds[index].x + raw_bounds[index].width)).max(0.0)
        } else {
            0.0
        },
        y: if flags.scrollable {
            (content_bottom - (raw_bounds[index].y + raw_bounds[index].height)).max(0.0)
        } else {
            0.0
        },
    };
    result.max_scroll[index] = maximum;
    scroll_offsets[index].x = scroll_offsets[index].x.clamp(0.0, maximum.x);
    scroll_offsets[index].y = scroll_offsets[index].y.clamp(0.0, maximum.y);

    let child_clip = if flags.clips_children {
        intersect_rect(parent_clip, bounds)
    } else {
        parent_clip
    };
    let child_offset = Point {
        x: parent_offset.x
            + if flags.scrollable {
                scroll_offsets[index].x
            } else {
                0.0
            },
        y: parent_offset.y
            + if flags.scrollable {
                scroll_offsets[index].y
            } else {
                0.0
            },
    };
    for child in &children[index] {
        visit(
            *child,
            raw_bounds,
            states,
            children,
            scroll_offsets,
            child_offset,
            child_clip,
            result,
        );
    }
}

fn subtree_bottom(index: usize, bounds: &[Rect], children: &[Vec<usize>]) -> f32 {
    children[index]
        .iter()
        .map(|child| subtree_bottom(*child, bounds, children))
        .fold(bounds[index].y + bounds[index].height, f32::max)
}

fn subtree_right(index: usize, bounds: &[Rect], children: &[Vec<usize>]) -> f32 {
    children[index]
        .iter()
        .map(|child| subtree_right(*child, bounds, children))
        .fold(bounds[index].x + bounds[index].width, f32::max)
}

/// Applies a two-axis wheel delta to the deepest scrollable node under a point.
pub fn scroll_at_point(
    point: Point,
    delta: Point,
    scale: f32,
    bounds: &[Rect],
    clips: &[Option<Rect>],
    states: &[CssComputedState],
    max_scroll: &[Point],
    scroll_offsets: &mut [Point],
) -> Option<usize> {
    let x = point.x * scale;
    let y = point.y * scale;
    let index = bounds
        .iter()
        .enumerate()
        .rev()
        .find_map(|(index, bounds)| {
            (super::taffy::taffy_style_state(&states[index]).1.scrollable
                && (max_scroll[index].x > 0.0 || max_scroll[index].y > 0.0)
                && contains(*bounds, x, y)
                && clips[index].is_none_or(|clip| contains(clip, x, y)))
            .then_some(index)
        })?;
    let previous = scroll_offsets[index];
    scroll_offsets[index].x = (previous.x - delta.x * scale).clamp(0.0, max_scroll[index].x);
    scroll_offsets[index].y = (previous.y - delta.y * scale).clamp(0.0, max_scroll[index].y);
    (scroll_offsets[index] != previous).then_some(index)
}

fn contains(rect: Rect, x: f32, y: f32) -> bool {
    x >= rect.x && y >= rect.y && x < rect.x + rect.width && y < rect.y + rect.height
}

fn intersect_rect(first: Option<Rect>, second: Rect) -> Option<Rect> {
    let Some(first) = first else {
        return Some(second);
    };
    let x = first.x.max(second.x);
    let y = first.y.max(second.y);
    let right = (first.x + first.width).min(second.x + second.width);
    let bottom = (first.y + first.height).min(second.y + second.height);
    Some(Rect {
        x,
        y,
        width: (right - x).max(0.0),
        height: (bottom - y).max(0.0),
    })
}
