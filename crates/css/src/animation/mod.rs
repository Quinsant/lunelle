use std::collections::BTreeMap;

use lunelle_macros::css_keyword_enum;

use crate::{ComputedStyleTree, Keyframes, transitions::TimingFunction};

mod parser;

pub use parser::{parse_animation, resolve_animations};

/// Samples keyframe declarations at normalized progress, using base values for
/// properties without explicit 0% or 100% declarations.
pub fn sample_keyframes(
    keyframes: &Keyframes,
    progress: f32,
    underlying: &BTreeMap<String, String>,
) -> BTreeMap<String, String> {
    if !progress.is_finite() {
        return BTreeMap::new();
    }

    let progress = progress.clamp(0.0, 1.0);
    let mut tracks = BTreeMap::<String, BTreeMap<u32, String>>::new();
    for frame in &keyframes.frames {
        if !frame.offset.is_finite() || !(0.0..=1.0).contains(&frame.offset) {
            continue;
        }
        let offset = (frame.offset * 1_000_000.0).round() as u32;
        for declaration in &frame.declarations {
            tracks
                .entry(declaration.name.clone())
                .or_default()
                .insert(offset, declaration.value.clone());
        }
    }

    tracks
        .into_iter()
        .filter_map(|(property, points)| {
            let mut points: Vec<_> = points.into_iter().collect();
            if points.first().is_none_or(|(offset, _)| *offset != 0) {
                if let Some(value) = underlying.get(&property) {
                    points.insert(0, (0, value.clone()));
                }
            }
            if points.last().is_none_or(|(offset, _)| *offset != 1_000_000) {
                if let Some(value) = underlying.get(&property) {
                    points.push((1_000_000, value.clone()));
                }
            }

            let progress = (progress * 1_000_000.0).round() as u32;
            let upper = points.partition_point(|(offset, _)| *offset < progress);
            if upper == 0 && points[0].0 != 0 {
                return None;
            }
            let (from_offset, from_value) = points.get(upper.saturating_sub(1))?;
            let (to_offset, to_value) = points.get(upper).or_else(|| points.last())?;
            let segment_progress = if from_offset == to_offset {
                0.0
            } else {
                (progress - from_offset) as f32 / (to_offset - from_offset) as f32
            };
            Some((
                property,
                interpolate_keyframe_value(from_value, to_value, segment_progress),
            ))
        })
        .collect()
}

/// Samples active animation runs against named keyframes and merges their
/// properties over the underlying computed values. Later runs take precedence.
pub fn sample_animation_runs(
    runs: &[AnimationRun],
    keyframes: &[Keyframes],
    now_ms: f32,
    underlying: &BTreeMap<String, String>,
) -> BTreeMap<String, String> {
    let mut sampled = BTreeMap::new();
    for run in runs {
        let Some(progress) = run.sample(now_ms).map(|sample| sample.progress) else {
            continue;
        };
        let Some(frames) = keyframes
            .iter()
            .rev()
            .find(|frames| frames.name == run.animation.name)
        else {
            continue;
        };
        sampled.extend(sample_keyframes(frames, progress, underlying));
    }
    sampled
}

/// Interpolates compatible numeric values and switches discrete values midway.
fn interpolate_keyframe_value(from: &str, to: &str, progress: f32) -> String {
    if let (Some((from_value, from_unit)), Some((to_value, to_unit))) =
        (parse_number_with_unit(from), parse_number_with_unit(to))
        && from_unit == to_unit
    {
        let value = from_value + (to_value - from_value) * progress;
        return format!("{value}{from_unit}");
    }
    if progress < 0.5 { from } else { to }.to_owned()
}

/// Separates a finite numeric prefix from its unit suffix.
fn parse_number_with_unit(value: &str) -> Option<(f32, &str)> {
    for index in (1..=value.len()).rev() {
        if value.is_char_boundary(index)
            && let Ok(number) = value[..index].parse::<f32>()
            && number.is_finite()
        {
            return Some((number, &value[index..]));
        }
    }
    value
        .parse::<f32>()
        .ok()
        .filter(|number| number.is_finite())
        .map(|number| (number, ""))
}

#[css_keyword_enum(keywords(
    "normal" = Normal,
    "reverse" = Reverse,
    "alternate" = Alternate,
    "alternate-reverse" = AlternateReverse,
))]
/// Direction in which successive animation iterations traverse keyframes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnimationDirection {
    /// Plays every iteration from the first keyframe to the last.
    Normal,
    /// Plays every iteration from the last keyframe to the first.
    Reverse,
    /// Alternates direction after each iteration.
    Alternate,
    /// Alternates direction, starting from the last keyframe.
    AlternateReverse,
}

#[css_keyword_enum(keywords(
    "none" = None,
    "forwards" = Forwards,
    "backwards" = Backwards,
    "both" = Both,
))]
/// Controls whether an animation retains values outside its active interval.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnimationFillMode {
    /// Applies no keyframe values outside the active interval.
    None,
    /// Retains the last keyframe after completion.
    Forwards,
    /// Applies the first keyframe during the delay.
    Backwards,
    /// Applies both backwards and forwards fill behavior.
    Both,
}

/// Number of animation iterations, including the infinite case.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AnimationIterationCount {
    /// Finite number of iterations.
    Count(f32),
    /// Repeats until the animation is replaced.
    Infinite,
}

#[css_keyword_enum(keywords("running" = Running, "paused" = Paused))]
/// Runtime state of an animation timeline.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnimationPlayState {
    /// The timeline advances with time.
    Running,
    /// The timeline is frozen at its current position.
    Paused,
}

/// Fully resolved animation shorthand for one computed style.
#[derive(Clone, Debug, PartialEq)]
pub struct Animation {
    /// Animation or keyframes name.
    pub name: String,
    /// Active interval duration in milliseconds.
    pub duration_ms: f32,
    /// Easing function applied to keyframe progress.
    pub timing_function: TimingFunction,
    /// Delay before the active interval in milliseconds.
    pub delay_ms: f32,
    /// Number of times the timeline repeats.
    pub iteration_count: AnimationIterationCount,
    /// Direction used for successive iterations.
    pub direction: AnimationDirection,
    /// Behavior outside the active interval.
    pub fill_mode: AnimationFillMode,
    /// Whether the timeline advances or is frozen.
    pub play_state: AnimationPlayState,
}

/// Sampled animation progress and completion status at a given time.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AnimationSample {
    /// Normalized progress within the current iteration.
    pub progress: f32,
    /// Zero-based current iteration number.
    pub iteration: u64,
    /// Whether the finite animation has completed.
    pub finished: bool,
}

/// A timeline for one animation. It computes normalized keyframe progress but
/// leaves property interpolation and renderer scheduling to their owners.
#[derive(Clone, Debug, PartialEq)]
pub struct AnimationRun {
    animation: Animation,
    start_time_ms: f32,
    paused_at_ms: Option<f32>,
    paused_duration_ms: f32,
}

/// Reconciles computed animation declarations with persistent timelines and
/// applies their sampled values to a versioned computed-style tree.
#[derive(Clone, Debug, Default)]
pub struct AnimationController {
    runs: BTreeMap<lunelle_core::NodeId, Vec<Option<AnimationRun>>>,
}

impl AnimationController {
    /// Reconciles timelines and overlays sampled keyframe values onto a tree.
    pub fn update(
        &mut self,
        mut tree: ComputedStyleTree,
        keyframes: &[Keyframes],
        now_ms: f32,
    ) -> ComputedStyleTree {
        let mut active_nodes = std::collections::BTreeSet::new();
        for style in &mut tree.styles {
            active_nodes.insert(style.node);
            let animations = style.animations().unwrap_or_default();
            let runs = self.runs.entry(style.node).or_default();
            reconcile_runs(runs, &animations, now_ms);
            let active_runs: Vec<_> = runs.iter().flatten().cloned().collect();
            let sampled = sample_animation_runs(&active_runs, keyframes, now_ms, &style.properties);
            style.properties.extend(sampled);
        }
        self.runs.retain(|node, _| active_nodes.contains(node));
        tree
    }

    /// Reports whether at least one active run needs another frame.
    pub fn needs_frame(&self, now_ms: f32) -> bool {
        self.runs
            .values()
            .flatten()
            .flatten()
            .any(|run| run.needs_frame(now_ms))
    }

    /// Returns the number of nodes that currently have animation timelines.
    pub fn run_count(&self) -> usize {
        self.runs.values().map(Vec::len).sum()
    }
}

/// Reuses compatible timelines and aligns persistent runs with declarations.
fn reconcile_runs(runs: &mut Vec<Option<AnimationRun>>, animations: &[Animation], now_ms: f32) {
    if !now_ms.is_finite() {
        return;
    }
    runs.resize_with(animations.len(), || None);
    for (index, animation) in animations.iter().enumerate() {
        if animation.name.eq_ignore_ascii_case("none") {
            runs[index] = None;
            continue;
        }
        match runs[index].as_mut() {
            Some(run) if same_animation_except_play_state(&run.animation, animation) => {
                match (run.animation.play_state, animation.play_state) {
                    (AnimationPlayState::Running, AnimationPlayState::Paused) => run.pause(now_ms),
                    (AnimationPlayState::Paused, AnimationPlayState::Running) => run.resume(now_ms),
                    _ => {}
                }
                run.animation.play_state = animation.play_state;
            }
            Some(run) => *run = AnimationRun::new(animation.clone(), now_ms),
            None => runs[index] = Some(AnimationRun::new(animation.clone(), now_ms)),
        }
    }
}

/// Tests whether an animation changed only its paused/running state.
fn same_animation_except_play_state(left: &Animation, right: &Animation) -> bool {
    left.name == right.name
        && left.duration_ms == right.duration_ms
        && left.timing_function == right.timing_function
        && left.delay_ms == right.delay_ms
        && left.iteration_count == right.iteration_count
        && left.direction == right.direction
        && left.fill_mode == right.fill_mode
}

impl AnimationRun {
    /// Creates a timeline beginning at the supplied clock time.
    pub fn new(animation: Animation, start_time_ms: f32) -> Self {
        let paused_at_ms =
            (animation.play_state == AnimationPlayState::Paused).then_some(start_time_ms);
        Self {
            animation,
            start_time_ms,
            paused_at_ms,
            paused_duration_ms: 0.0,
        }
    }

    /// Pauses the timeline at the supplied clock time.
    pub fn pause(&mut self, now_ms: f32) {
        if self.paused_at_ms.is_none() && now_ms.is_finite() {
            self.paused_at_ms = Some(now_ms);
        }
    }

    /// Resumes the timeline and accounts for time spent paused.
    pub fn resume(&mut self, now_ms: f32) {
        if let Some(paused_at_ms) = self.paused_at_ms.take() {
            if now_ms.is_finite() && now_ms >= paused_at_ms {
                self.paused_duration_ms += now_ms - paused_at_ms;
            } else {
                self.paused_at_ms = Some(paused_at_ms);
            }
        }
    }

    /// Samples normalized progress at the supplied clock time.
    pub fn sample(&self, now_ms: f32) -> Option<AnimationSample> {
        let animation = &self.animation;
        if animation.name.eq_ignore_ascii_case("none")
            || !now_ms.is_finite()
            || !self.start_time_ms.is_finite()
            || !self.paused_duration_ms.is_finite()
            || !animation.duration_ms.is_finite()
            || animation.duration_ms < 0.0
            || !animation.delay_ms.is_finite()
        {
            return None;
        }

        let timeline_now = self.paused_at_ms.unwrap_or(now_ms);
        let elapsed =
            timeline_now - self.start_time_ms - self.paused_duration_ms - animation.delay_ms;
        let iteration_count = match animation.iteration_count {
            AnimationIterationCount::Count(count) if count.is_finite() && count >= 0.0 => {
                Some(count)
            }
            AnimationIterationCount::Infinite => None,
            _ => return None,
        };

        if iteration_count == Some(0.0) || animation.duration_ms == 0.0 {
            if elapsed < 0.0
                && matches!(
                    animation.fill_mode,
                    AnimationFillMode::Backwards | AnimationFillMode::Both
                )
            {
                return Some(self.sample_at(0.0, 0, false));
            }
            if elapsed >= 0.0
                && matches!(
                    animation.fill_mode,
                    AnimationFillMode::Forwards | AnimationFillMode::Both
                )
            {
                let count = iteration_count.unwrap_or(1.0);
                return Some(self.sample_at(final_progress(count), last_iteration(count), true));
            }
            return None;
        }

        if elapsed < 0.0 {
            return (animation.fill_mode == AnimationFillMode::Backwards
                || animation.fill_mode == AnimationFillMode::Both)
                .then(|| self.sample_at(0.0, 0, false));
        }

        let active_elapsed = elapsed;
        if let Some(count) = iteration_count {
            let active_duration = animation.duration_ms * count;
            if active_elapsed >= active_duration {
                return (animation.fill_mode == AnimationFillMode::Forwards
                    || animation.fill_mode == AnimationFillMode::Both)
                    .then(|| self.sample_at(final_progress(count), last_iteration(count), true));
            }
        }

        let iteration_position = active_elapsed / animation.duration_ms;
        let iteration = iteration_position.floor() as u64;
        let progress = iteration_position.fract();
        Some(self.sample_at(progress, iteration, false))
    }

    fn needs_frame(&self, now_ms: f32) -> bool {
        if self.animation.play_state != AnimationPlayState::Running
            || !now_ms.is_finite()
            || !self.animation.duration_ms.is_finite()
            || self.animation.duration_ms <= 0.0
            || !self.animation.delay_ms.is_finite()
        {
            return false;
        }
        let elapsed = now_ms - self.start_time_ms - self.paused_duration_ms;
        let Some(iterations) = (match self.animation.iteration_count {
            AnimationIterationCount::Count(count) if count.is_finite() && count >= 0.0 => {
                Some(count)
            }
            AnimationIterationCount::Infinite => None,
            _ => return false,
        }) else {
            return true;
        };
        elapsed < self.animation.delay_ms + self.animation.duration_ms * iterations
    }

    fn sample_at(&self, progress: f32, iteration: u64, finished: bool) -> AnimationSample {
        let reversed = match self.animation.direction {
            AnimationDirection::Normal => false,
            AnimationDirection::Reverse => true,
            AnimationDirection::Alternate => iteration % 2 == 1,
            AnimationDirection::AlternateReverse => iteration % 2 == 0,
        };
        let directed_progress = if reversed { 1.0 - progress } else { progress };
        AnimationSample {
            progress: self.animation.timing_function.sample(directed_progress),
            iteration,
            finished,
        }
    }
}

/// Returns the zero-based final iteration index for a finite count.
fn last_iteration(count: f32) -> u64 {
    count.ceil().max(1.0) as u64 - 1
}

/// Computes terminal progress for an integer or fractional iteration count.
fn final_progress(count: f32) -> f32 {
    let fraction = count.fract();
    if fraction == 0.0 { 1.0 } else { fraction }
}

impl Default for Animation {
    fn default() -> Self {
        Self {
            name: "none".into(),
            duration_ms: 0.0,
            timing_function: TimingFunction::Ease,
            delay_ms: 0.0,
            iteration_count: AnimationIterationCount::Count(1.0),
            direction: AnimationDirection::Normal,
            fill_mode: AnimationFillMode::None,
            play_state: AnimationPlayState::Running,
        }
    }
}
