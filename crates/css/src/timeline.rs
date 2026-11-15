use std::collections::BTreeMap;

use lunelle_core::NodeId;

use crate::utils::{parse_color, style_opacity};
use crate::{
    AnimationController, ComputedStyle, ComputedStyleTree, TransitionProperty, TransitionRun,
    parse_transition,
};

#[derive(Clone)]
struct ColorTransitionRun {
    timeline: TransitionRun,
    from: [f32; 4],
    to: [f32; 4],
}
#[derive(Clone)]
struct ScalarTransitionRun {
    timeline: TransitionRun,
    from: f32,
    to: f32,
}

pub(super) trait TimelineBackend {
    fn update(
        &mut self,
        tree: ComputedStyleTree,
        keyframes: &[crate::Keyframes],
        now_ms: f32,
    ) -> (ComputedStyleTree, bool);
    fn needs_frame(&self, now_ms: f32) -> bool;
    fn retarget_transitions(
        &mut self,
        previous_base: &[ComputedStyle],
        presented: &[ComputedStyle],
        next_base: &[ComputedStyle],
        now_ms: f32,
    );
}

#[derive(Clone, Default)]
pub(super) struct CssTimelineBackend {
    animations: AnimationController,
    color_transitions: BTreeMap<NodeId, ColorTransitionRun>,
    opacity_transitions: BTreeMap<NodeId, ScalarTransitionRun>,
}

impl TimelineBackend for CssTimelineBackend {
    fn update(
        &mut self,
        mut tree: ComputedStyleTree,
        keyframes: &[crate::Keyframes],
        now_ms: f32,
    ) -> (ComputedStyleTree, bool) {
        tree = self.animations.update(tree, keyframes, now_ms);
        let active = apply_color_samples(&mut tree.styles, &mut self.color_transitions, now_ms)
            | apply_opacity_samples(&mut tree.styles, &mut self.opacity_transitions, now_ms);
        (tree, active || self.animations.needs_frame(now_ms))
    }
    fn needs_frame(&self, now_ms: f32) -> bool {
        self.animations.needs_frame(now_ms)
            || !self.color_transitions.is_empty()
            || !self.opacity_transitions.is_empty()
    }
    fn retarget_transitions(
        &mut self,
        previous_base: &[ComputedStyle],
        presented: &[ComputedStyle],
        next_base: &[ComputedStyle],
        now_ms: f32,
    ) {
        let mut colors = BTreeMap::new();
        let mut opacity = BTreeMap::new();
        for ((previous, shown), next) in previous_base.iter().zip(presented).zip(next_base) {
            if previous.properties.get("background-color")
                != next.properties.get("background-color")
            {
                if let Some(run) = color_transition(shown, next, now_ms) {
                    colors.insert(next.node, run);
                }
            } else if let Some(run) = self.color_transitions.get(&next.node) {
                colors.insert(next.node, run.clone());
            }
            if previous.properties.get("opacity") != next.properties.get("opacity") {
                if let Some(run) = opacity_transition(shown, next, now_ms) {
                    opacity.insert(next.node, run);
                }
            } else if let Some(run) = self.opacity_transitions.get(&next.node) {
                opacity.insert(next.node, run.clone());
            }
        }
        self.color_transitions = colors;
        self.opacity_transitions = opacity;
    }
}

fn color_transition(
    presented: &ComputedStyle,
    target: &ComputedStyle,
    now_ms: f32,
) -> Option<ColorTransitionRun> {
    let transition = matching_transition(target, "background-color")?;
    let from = presented
        .properties
        .get("background-color")
        .map_or(Some([0.0, 0.0, 0.0, 0.0]), |value| parse_color(value))?;
    let to = parse_color(target.properties.get("background-color")?)?;
    Some(ColorTransitionRun {
        timeline: TransitionRun::new(0.0, 1.0, now_ms, transition),
        from,
        to,
    })
}
fn opacity_transition(
    presented: &ComputedStyle,
    target: &ComputedStyle,
    now_ms: f32,
) -> Option<ScalarTransitionRun> {
    let transition = matching_transition(target, "opacity")?;
    Some(ScalarTransitionRun {
        timeline: TransitionRun::new(0.0, 1.0, now_ms, transition),
        from: style_opacity(presented),
        to: style_opacity(target),
    })
}
fn matching_transition(target: &ComputedStyle, property: &str) -> Option<crate::Transition> {
    parse_transition(target.properties.get("transition")?)?
        .into_iter()
        .rev()
        .find(|transition| match &transition.property {
            TransitionProperty::All => transition.duration_ms > 0.0,
            TransitionProperty::None => false,
            TransitionProperty::Property(name) => name == property && transition.duration_ms > 0.0,
        })
}
fn apply_color_samples(
    styles: &mut [ComputedStyle],
    runs: &mut BTreeMap<NodeId, ColorTransitionRun>,
    now_ms: f32,
) -> bool {
    let mut active = false;
    runs.retain(|node, run| {
        let Some(style) = styles.iter_mut().find(|style| style.node == *node) else {
            return false;
        };
        let sample = run.timeline.sample(now_ms);
        let color = interpolate_color(run.from, run.to, sample.value);
        style.properties.insert(
            "background-color".into(),
            format!(
                "rgba({:.2},{:.2},{:.2},{:.4})",
                color[0] * 255.0,
                color[1] * 255.0,
                color[2] * 255.0,
                color[3]
            ),
        );
        active |= !sample.finished;
        !sample.finished
    });
    active
}
fn apply_opacity_samples(
    styles: &mut [ComputedStyle],
    runs: &mut BTreeMap<NodeId, ScalarTransitionRun>,
    now_ms: f32,
) -> bool {
    let mut active = false;
    runs.retain(|node, run| {
        let Some(style) = styles.iter_mut().find(|style| style.node == *node) else {
            return false;
        };
        let sample = run.timeline.sample(now_ms);
        let value = run.from + (run.to - run.from) * sample.value;
        style
            .properties
            .insert("opacity".into(), format!("{value:.6}"));
        active |= !sample.finished;
        !sample.finished
    });
    active
}
fn interpolate_color(from: [f32; 4], to: [f32; 4], progress: f32) -> [f32; 4] {
    let progress = progress.clamp(0.0, 1.0);
    let alpha = from[3] + (to[3] - from[3]) * progress;
    let mut color = [0.0; 4];
    color[3] = alpha;
    if alpha > 0.0 {
        for channel in 0..3 {
            let from = from[channel] * from[3];
            let to = to[channel] * to[3];
            color[channel] = (from + (to - from) * progress) / alpha;
        }
    }
    color
}
