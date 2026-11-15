mod parser;

pub use parser::parse_transition;
pub(crate) use parser::{parse_time, parse_timing_function};

use lunelle_macros::css_keyword_enum;

/// Checks whether selected properties changed between two computed trees.
pub fn styles_changed_in(
    previous: &[crate::ComputedStyle],
    current: &[crate::ComputedStyle],
    properties: &[&str],
) -> bool {
    previous.len() != current.len()
        || previous.iter().zip(current).any(|(previous, current)| {
            previous.node != current.node
                || properties.iter().any(|property| {
                    previous.properties.get(*property) != current.properties.get(*property)
                })
        })
}

/// Property selection used by a CSS transition declaration.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TransitionProperty {
    /// Applies the transition to every supported property.
    All,
    /// Disables transitions.
    None,
    /// Applies the transition to one named property.
    Property(String),
}

/// Easing function used to map transition progress.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TimingFunction {
    /// CSS `ease` timing function.
    Ease,
    /// Linear timing function.
    Linear,
    /// CSS `ease-in` timing function.
    EaseIn,
    /// CSS `ease-out` timing function.
    EaseOut,
    /// CSS `ease-in-out` timing function.
    EaseInOut,
    /// Cubic-bezier control points.
    CubicBezier(f32, f32, f32, f32),
    /// Discrete step count and jump position.
    Steps(u32, StepPosition),
}

#[css_keyword_enum(keywords(
    "start" = JumpStart,
    "jump-start" = JumpStart,
    "end" = JumpEnd,
    "jump-end" = JumpEnd,
    "jump-none" = JumpNone,
    "jump-both" = JumpBoth,
))]
/// Step discontinuity placement for `steps()` easing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StepPosition {
    /// Jumps at the beginning of each step.
    JumpStart,
    /// Jumps at the end of each step.
    JumpEnd,
    /// Omits jumps at both ends.
    JumpNone,
    /// Includes jumps at both ends.
    JumpBoth,
}

/// Resolved transition shorthand for one property.
#[derive(Clone, Debug, PartialEq)]
pub struct Transition {
    /// Property selected by the transition declaration.
    pub property: TransitionProperty,
    /// Transition duration in milliseconds.
    pub duration_ms: f32,
    /// Easing function used for interpolation.
    pub timing_function: TimingFunction,
    /// Delay before interpolation in milliseconds.
    pub delay_ms: f32,
}

/// Value and completion state sampled from a transition timeline.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TransitionSample {
    /// Interpolated value at the sample time.
    pub value: f32,
    /// Whether the transition reached its target.
    pub finished: bool,
}

/// An interruptible numeric transition timeline. Retargeting starts from the value
/// currently being presented, matching CSS transition interruption behavior.
#[derive(Clone, Debug, PartialEq)]
pub struct TransitionRun {
    from: f32,
    to: f32,
    start_time_ms: f32,
    transition: Transition,
}

impl TransitionRun {
    /// Creates a transition from an initial value to a target value.
    pub fn new(from: f32, to: f32, start_time_ms: f32, transition: Transition) -> Self {
        Self {
            from,
            to,
            start_time_ms,
            transition,
        }
    }

    /// Samples the current value, honoring delay, duration, and easing.
    pub fn sample(&self, now_ms: f32) -> TransitionSample {
        let elapsed = now_ms - self.start_time_ms - self.transition.delay_ms;
        if elapsed < 0.0 {
            return TransitionSample {
                value: self.from,
                finished: false,
            };
        }
        if self.transition.duration_ms <= 0.0 || elapsed >= self.transition.duration_ms {
            return TransitionSample {
                value: self.to,
                finished: true,
            };
        }
        let progress = elapsed / self.transition.duration_ms;
        let eased = self.transition.timing_function.sample(progress);
        TransitionSample {
            value: self.from + (self.to - self.from) * eased,
            finished: false,
        }
    }

    /// Changes the target while preserving the currently presented value.
    pub fn retarget(&mut self, now_ms: f32, to: f32, transition: Transition) {
        let current = self.sample(now_ms).value;
        self.from = current;
        self.to = to;
        self.start_time_ms = now_ms;
        self.transition = transition;
    }
}

impl Default for Transition {
    fn default() -> Self {
        Self {
            property: TransitionProperty::All,
            duration_ms: 0.0,
            timing_function: TimingFunction::Ease,
            delay_ms: 0.0,
        }
    }
}

impl TimingFunction {
    /// Maps normalized input progress to timing-function output progress.
    pub fn sample(self, progress: f32) -> f32 {
        let progress = progress.clamp(0.0, 1.0);
        match self {
            Self::Linear => progress,
            Self::Ease => cubic_bezier_sample(progress, 0.25, 0.1, 0.25, 1.0),
            Self::EaseIn => cubic_bezier_sample(progress, 0.42, 0.0, 1.0, 1.0),
            Self::EaseOut => cubic_bezier_sample(progress, 0.0, 0.0, 0.58, 1.0),
            Self::EaseInOut => cubic_bezier_sample(progress, 0.42, 0.0, 0.58, 1.0),
            Self::CubicBezier(x1, y1, x2, y2) => cubic_bezier_sample(progress, x1, y1, x2, y2),
            Self::Steps(count, position) => steps_sample(progress, count, position),
        }
    }
}

/// Evaluates cubic-bezier easing by solving the x curve and sampling y.
fn cubic_bezier_sample(progress: f32, x1: f32, y1: f32, x2: f32, y2: f32) -> f32 {
    let coordinate = |t: f32, p1: f32, p2: f32| {
        3.0 * (1.0 - t).powi(2) * t * p1 + 3.0 * (1.0 - t) * t.powi(2) * p2 + t.powi(3)
    };
    let derivative = |t: f32, p1: f32, p2: f32| {
        3.0 * (1.0 - t).powi(2) * p1
            + 6.0 * (1.0 - t) * t * (p2 - p1)
            + 3.0 * t.powi(2) * (1.0 - p2)
    };

    // Newton iteration is fast for the common case; bisection keeps the result
    // stable near flat tangents and at the ends of the curve.
    let mut t = progress;
    for _ in 0..8 {
        let error = coordinate(t, x1, x2) - progress;
        let slope = derivative(t, x1, x2);
        if error.abs() < 1e-6 || slope.abs() < 1e-6 {
            break;
        }
        t = (t - error / slope).clamp(0.0, 1.0);
    }
    let mut low = 0.0;
    let mut high = 1.0;
    for _ in 0..16 {
        let x = coordinate(t, x1, x2);
        if (x - progress).abs() < 1e-6 {
            break;
        }
        if x < progress {
            low = t;
        } else {
            high = t;
        }
        t = (low + high) * 0.5;
    }
    coordinate(t, y1, y2)
}

/// Maps progress to one of the discrete steps in a steps easing.
fn steps_sample(progress: f32, count: u32, position: StepPosition) -> f32 {
    let count = count.max(1) as f32;
    let scaled = progress * count;
    match position {
        StepPosition::JumpStart => ((scaled.floor() + 1.0) / count).min(1.0),
        StepPosition::JumpEnd => scaled.floor().min(count) / count,
        StepPosition::JumpNone => {
            if progress >= 1.0 {
                1.0
            } else {
                scaled.floor() / (count - 1.0).max(1.0)
            }
        }
        StepPosition::JumpBoth => (scaled.floor() + 1.0) / (count + 1.0),
    }
}
