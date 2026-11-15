use lunelle_aom::{Element, StyleSnapshot};
use lunelle_core::NodeId;
use lunelle_core::{Point, Rect};
use lunelle_css::{
    ComputedStyle, CssRender, CssTextNode, StyleState, taffy_style_state, text_intrinsic_size,
};
use std::collections::BTreeMap;
use taffy::{AvailableSpace, Dimension, Size, TaffyTree, TraversePartialTree};

pub(super) struct CssScene {
    pub(super) _snapshot: StyleSnapshot,
    pub(super) styles: Vec<ComputedStyle>,
    pub(super) text: Vec<CssTextNode>,
    pub(super) bounds: Vec<Rect>,
    pub(super) clips: Vec<Option<Rect>>,
    pub(super) states: Vec<lunelle_css::CssComputedState>,
    pub(super) raw_bounds: Vec<Rect>,
    pub(super) children: Vec<Vec<usize>>,
    pub(super) scroll_offsets: Vec<Point>,
    pub(super) max_scroll: Vec<Point>,
}

impl CssScene {
    pub(super) fn from_aom(
        element: &dyn Element,
        render: &CssRender,
        width: u32,
        height: u32,
        scale_factor: f32,
    ) -> Self {
        Self::from_aom_with_states(
            element,
            render,
            width,
            height,
            scale_factor,
            BTreeMap::new(),
        )
    }

    pub(super) fn from_aom_with_states(
        element: &dyn Element,
        render: &CssRender,
        width: u32,
        height: u32,
        scale_factor: f32,
        interaction: BTreeMap<NodeId, StyleState>,
    ) -> Self {
        let snapshot = element.style_snapshot(1);
        let text = collect_text(element);
        let context = lunelle_css::style_compute_context(
            snapshot.version,
            0,
            0,
            width,
            height,
            scale_factor,
            interaction,
        );
        let styles = render
            .compute_with_context(&snapshot.nodes, &context)
            .styles;
        let states = render.states(&styles);
        let (raw_bounds, children) =
            compute_layout(element, &styles, &states, width as f32, height as f32);
        let mut scene = Self {
            _snapshot: snapshot,
            styles,
            text,
            bounds: Vec::new(),
            clips: Vec::new(),
            scroll_offsets: vec![Point { x: 0.0, y: 0.0 }; states.len()],
            max_scroll: Vec::new(),
            states,
            raw_bounds,
            children,
        };
        scene.apply_overflow();
        scene
    }

    pub(super) fn apply_overflow(&mut self) {
        let layout = lunelle_css::apply_overflow_layout(
            &self.raw_bounds,
            &self.states,
            &self.children,
            &mut self.scroll_offsets,
        );
        self.bounds = layout.bounds;
        self.clips = layout.clips;
        self.max_scroll = layout.max_scroll;
    }

    pub(super) fn apply_styles(
        &mut self,
        element: &dyn Element,
        render: &CssRender,
        styles: Vec<ComputedStyle>,
        width: u32,
        height: u32,
    ) -> bool {
        let layout_changed = self.styles.len() != styles.len()
            || self.styles.iter().zip(&styles).any(|(previous, next)| {
                previous.node != next.node
                    || previous
                        .properties
                        .iter()
                        .chain(next.properties.iter())
                        .any(|(name, _)| {
                            !matches!(
                                name.as_str(),
                                "background-color"
                                    | "color"
                                    | "opacity"
                                    | "transform"
                                    | "transform-origin"
                            ) && previous.properties.get(name) != next.properties.get(name)
                        })
            });
        self.styles = styles;
        self.states = render.states(&self.styles);
        if layout_changed {
            (self.raw_bounds, self.children) = compute_layout(
                element,
                &self.styles,
                &self.states,
                width as f32,
                height as f32,
            );
            self.apply_overflow();
        }
        layout_changed
    }
}

fn compute_layout(
    root: &dyn Element,
    styles: &[ComputedStyle],
    states: &[lunelle_css::CssComputedState],
    width: f32,
    height: f32,
) -> (Vec<Rect>, Vec<Vec<usize>>) {
    fn build(
        element: &dyn Element,
        index: &mut usize,
        styles: &[ComputedStyle],
        states: &[lunelle_css::CssComputedState],
        tree: &mut TaffyTree,
        text_bounds: &mut Vec<Option<(f32, f32)>>,
        width: f32,
        height: f32,
    ) -> taffy::NodeId {
        let current = *index;
        *index += 1;
        let (mut style, _) = taffy_style_state(&states[current]);
        if current == 0 {
            style.size.width = Dimension::length(width);
            style.size.height = Dimension::length(height);
        }
        if let Some(value) = styles[current]
            .properties
            .get("min-height")
            .and_then(|value| value.strip_suffix("vh"))
            .and_then(|value| value.trim().parse::<f32>().ok())
        {
            style.min_size.height = taffy::LengthPercentageAuto::length(height * value / 100.0);
        }
        let intrinsic = element
            .text()
            .map(|text| text_intrinsic_size(&states[current], text));
        if let Some((_, text_height)) = intrinsic {
            if style.size.height == Dimension::auto() {
                style.size.height = Dimension::length(text_height.max(1.0));
            }
        }
        text_bounds.push(intrinsic);
        let children: Vec<_> = element
            .children()
            .iter()
            .map(|child| {
                build(
                    child.as_ref(),
                    index,
                    styles,
                    states,
                    tree,
                    text_bounds,
                    width,
                    height,
                )
            })
            .collect();
        if children.is_empty() {
            tree.new_leaf(style).expect("failed to create Taffy leaf")
        } else {
            tree.new_with_children(style, &children)
                .expect("failed to create Taffy node")
        }
    }

    let mut tree = TaffyTree::new();
    let mut index = 0;
    let mut text_bounds = Vec::with_capacity(states.len());
    let root_node = build(
        root,
        &mut index,
        styles,
        states,
        &mut tree,
        &mut text_bounds,
        width,
        height,
    );
    let available = Size {
        width: AvailableSpace::Definite(width),
        height: AvailableSpace::Definite(height),
    };
    let _ = tree.compute_layout(root_node, available);

    let mut bounds = Vec::with_capacity(states.len());
    let mut children = vec![Vec::new(); states.len()];
    fn collect(
        tree: &TaffyTree,
        node: taffy::NodeId,
        parent_x: f32,
        parent_y: f32,
        bounds: &mut Vec<Rect>,
        children: &mut [Vec<usize>],
    ) {
        let layout = tree.layout(node).expect("missing Taffy layout");
        let x = parent_x + layout.location.x;
        let y = parent_y + layout.location.y;
        let index = bounds.len();
        bounds.push(Rect {
            x,
            y,
            width: layout.size.width,
            height: layout.size.height,
        });
        for child in tree.child_ids(node) {
            children[index].push(bounds.len());
            collect(tree, child, x, y, bounds, children);
        }
    }
    collect(&tree, root_node, 0.0, 0.0, &mut bounds, &mut children);
    if bounds.len() < states.len() {
        bounds.resize(
            states.len(),
            Rect {
                x: 0.0,
                y: 0.0,
                width,
                height,
            },
        );
    }
    let _ = text_bounds;
    (bounds, children)
}

fn collect_text(element: &dyn Element) -> Vec<CssTextNode> {
    fn visit(element: &dyn Element, index: &mut usize, text: &mut Vec<CssTextNode>) {
        if let Some(value) = element.text() {
            text.push(CssTextNode {
                text: value.to_owned(),
                node: *index,
            });
        }
        *index += 1;
        for child in element.children() {
            visit(child.as_ref(), index, text);
        }
    }
    let mut text = Vec::new();
    visit(element, &mut 0, &mut text);
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::windowed::interaction::interaction_states;
    use lunelle_core::Point;
    use lunelle_css::StyleSheet;

    #[test]
    fn overflow_scene_clips_scrolls_and_limits_hover() {
        let root = lunelle_aom::Div::new().with_class("root").child(
            lunelle_aom::Div::new()
                .with_class("container")
                .child(lunelle_aom::Div::new().with_class("content")),
        );
        let sheet = StyleSheet::parse(
            ".root { display: block; } .container { display: block; width: 100px; height: 100px; overflow: auto; } .content { display: block; width: 100px; height: 200px; }",
        );
        let mut scene = CssScene::from_aom(&root, &CssRender::new(sheet), 300, 300, 1.0);
        assert_eq!(scene.clips[2], Some(scene.bounds[1]));
        assert_eq!(scene.max_scroll[1].y, 100.0);
        let outside = Point { x: 20.0, y: 150.0 };
        assert!(
            !interaction_states(&scene, Some(outside), false)
                .contains_key(&scene._snapshot.nodes[2].id)
        );
        assert_eq!(
            lunelle_css::scroll_at_point(
                Point { x: 20.0, y: 20.0 },
                Point { x: 0.0, y: -40.0 },
                1.0,
                &scene.bounds,
                &scene.clips,
                &scene.states,
                &scene.max_scroll,
                &mut scene.scroll_offsets
            ),
            Some(1)
        );
        scene.apply_overflow();
        assert_eq!(scene.bounds[2].y, scene.raw_bounds[2].y - 40.0);
        assert_eq!(scene.clips[2], Some(scene.bounds[1]));
    }

    #[test]
    fn overflow_scene_scrolls_horizontally() {
        let root = lunelle_aom::Div::new().with_class("root").child(
            lunelle_aom::Div::new()
                .with_class("container")
                .child(lunelle_aom::Div::new().with_class("content")),
        );
        let sheet = StyleSheet::parse(
            ".root { display: block; } .container { display: block; width: 100px; height: 100px; overflow-x: auto; } .content { display: block; width: 200px; height: 100px; }",
        );
        let mut scene = CssScene::from_aom(&root, &CssRender::new(sheet), 300, 300, 1.0);
        assert_eq!(scene.max_scroll[1].x, 100.0);
        assert_eq!(
            lunelle_css::scroll_at_point(
                Point { x: 20.0, y: 20.0 },
                Point { x: -40.0, y: 0.0 },
                1.0,
                &scene.bounds,
                &scene.clips,
                &scene.states,
                &scene.max_scroll,
                &mut scene.scroll_offsets
            ),
            Some(1)
        );
        scene.apply_overflow();
        assert_eq!(scene.bounds[2].x, scene.raw_bounds[2].x - 40.0);
    }
}
