use crate::parse_color;
use crate::utils::{css_length, split_top_level, split_top_level_whitespace};

/// Parsed linear, radial, or conic gradient ready for pixel sampling.
/// Parsed linear, radial, or conic gradient ready for pixel sampling.
#[derive(Clone, Debug)]
pub struct CssGradient {
    /// Gradient direction in radians.
    pub angle: f32,
    /// Gradient colors paired with normalized stop positions.
    pub stops: Vec<([f32; 4], f32)>,
    /// Whether the gradient uses radial distance.
    pub radial: bool,
    /// Whether the gradient repeats outside its stop range.
    pub repeating: bool,
    /// Whether the gradient uses angular conic sampling.
    pub conic: bool,
    /// Normalized gradient center.
    pub center: [f32; 2],
}

impl CssGradient {
    /// Samples the interpolated gradient color at a local pixel coordinate.
    pub fn color_at(&self, local: [f32; 2], width: f32, height: f32) -> [f32; 4] {
        let (sin, cos) = self.angle.sin_cos();
        let extent = (width * sin.abs() + height * cos.abs()).max(1.0);
        let relative = [
            local[0] - (self.center[0] - 0.5) * width,
            local[1] - (self.center[1] - 0.5) * height,
        ];
        let position = if self.conic {
            (relative[1].atan2(relative[0]) / std::f32::consts::TAU + 0.25
                - self.angle / std::f32::consts::TAU)
                .rem_euclid(1.0)
        } else if self.radial {
            relative[0].hypot(relative[1]) / (width.max(height) * 0.5).max(1.0)
        } else {
            (relative[0] * sin - relative[1] * cos) / extent + 0.5
        };
        let position = if self.repeating && !self.conic {
            let start = self.stops[0].1;
            let end = self.stops.last().unwrap().1;
            (position - start).rem_euclid((end - start).max(f32::EPSILON)) + start
        } else {
            position.clamp(0.0, 1.0)
        };
        let upper = self
            .stops
            .iter()
            .position(|stop| stop.1 >= position)
            .unwrap_or(self.stops.len() - 1);
        if upper == 0 {
            return self.stops[0].0;
        }
        let (from_color, from_position) = self.stops[upper - 1];
        let (to_color, to_position) = self.stops[upper];
        let amount = ((position - from_position) / (to_position - from_position).max(f32::EPSILON))
            .clamp(0.0, 1.0);
        std::array::from_fn(|channel| {
            from_color[channel] + (to_color[channel] - from_color[channel]) * amount
        })
    }
}

/// Parsed box-shadow lengths and RGBA color.
#[derive(Clone, Debug)]
pub struct CssShadow {
    pub lengths: [f32; 4],
    pub color: [f32; 4],
}

/// Parses supported comma-separated `box-shadow` layers.
pub fn parse_shadows(value: &str) -> Vec<CssShadow> {
    split_top_level(value, ',')
        .into_iter()
        .filter_map(|layer| {
            let mut lengths = Vec::new();
            let mut color = None;
            for token in split_top_level_whitespace(layer) {
                if token == "inset" {
                    return None;
                }
                if let Some(parsed) = parse_color(token) {
                    color = Some(parsed);
                } else if let Some(length) = css_length(token) {
                    lengths.push(length);
                } else {
                    return None;
                }
            }
            if !(2..=4).contains(&lengths.len()) {
                return None;
            }
            let mut values = [0.0; 4];
            values[..lengths.len()].copy_from_slice(&lengths);
            Some(CssShadow {
                lengths: values,
                color: color.unwrap_or([0.0, 0.0, 0.0, 0.25]),
            })
        })
        .collect()
}

/// Parses supported CSS gradient functions into a renderer-neutral model.
pub fn parse_gradient(value: &str) -> Option<CssGradient> {
    let value = value.trim();
    if let Some(body) = value
        .strip_prefix("repeating-linear-gradient(")
        .and_then(|v| v.strip_suffix(')'))
    {
        return parse_gradient_body(body, false, true);
    }
    if let Some(body) = value
        .strip_prefix("repeating-radial-gradient(")
        .and_then(|v| v.strip_suffix(')'))
    {
        let mut gradient = parse_radial_gradient(body)?;
        gradient.repeating = true;
        return Some(gradient);
    }
    if let Some(body) = value
        .strip_prefix("linear-gradient(")
        .and_then(|v| v.strip_suffix(')'))
    {
        return parse_gradient_body(body, false, false);
    }
    if let Some(body) = value
        .strip_prefix("radial-gradient(")
        .and_then(|v| v.strip_suffix(')'))
    {
        return parse_radial_gradient(body);
    }
    if let Some(body) = value
        .strip_prefix("conic-gradient(")
        .and_then(|v| v.strip_suffix(')'))
    {
        let mut parts = split_top_level(body, ',');
        let mut angle = 0.0;
        let mut center = [0.5, 0.5];
        if let Some(first) = parts.first().copied() {
            if first.starts_with("from ") && first.contains(" at ") {
                let (from, at) = first.split_once(" at ")?;
                angle = from
                    .strip_prefix("from ")?
                    .strip_suffix("deg")?
                    .trim()
                    .parse::<f32>()
                    .ok()?
                    .to_radians();
                center = parse_gradient_center(at)?;
                parts.remove(0);
            } else if let Some(from) = first.strip_prefix("from ") {
                angle = from
                    .strip_suffix("deg")?
                    .trim()
                    .parse::<f32>()
                    .ok()?
                    .to_radians();
                parts.remove(0);
            } else if let Some(at) = first.strip_prefix("at ") {
                center = parse_gradient_center(at)?;
                parts.remove(0);
            }
        }
        return Some(CssGradient {
            angle,
            stops: parse_gradient_stops(parts)?,
            radial: false,
            repeating: false,
            conic: true,
            center,
        });
    }
    None
}

fn parse_gradient_body(body: &str, radial: bool, repeating: bool) -> Option<CssGradient> {
    let mut parts = split_top_level(body, ',');
    if parts.len() < 2 {
        return None;
    }
    let mut angle = std::f32::consts::PI;
    if let Some(direction) = parts.first().copied() {
        let parsed = direction
            .strip_suffix("deg")
            .and_then(|v| v.trim().parse::<f32>().ok())
            .map(f32::to_radians)
            .or_else(|| {
                direction
                    .strip_suffix("turn")
                    .and_then(|v| v.trim().parse::<f32>().ok())
                    .map(|v| v * std::f32::consts::TAU)
            })
            .or(match direction {
                "to top" => Some(0.0),
                "to right" => Some(std::f32::consts::FRAC_PI_2),
                "to bottom" => Some(std::f32::consts::PI),
                "to left" => Some(-std::f32::consts::FRAC_PI_2),
                "to top right" | "to right top" => Some(std::f32::consts::FRAC_PI_4),
                "to bottom right" | "to right bottom" => Some(3.0 * std::f32::consts::FRAC_PI_4),
                "to bottom left" | "to left bottom" => Some(5.0 * std::f32::consts::FRAC_PI_4),
                "to top left" | "to left top" => Some(-3.0 * std::f32::consts::FRAC_PI_4),
                _ => None,
            });
        if let Some(parsed) = parsed {
            angle = parsed;
            parts.remove(0);
        }
    }
    Some(CssGradient {
        angle,
        stops: parse_gradient_stops(parts)?,
        radial,
        repeating,
        conic: false,
        center: [0.5, 0.5],
    })
}

fn parse_radial_gradient(body: &str) -> Option<CssGradient> {
    let mut parts = split_top_level(body, ',');
    let mut center = [0.5, 0.5];
    if parts.first().is_some_and(|v| v.contains(" at ")) {
        let (_, at) = parts[0].split_once(" at ")?;
        center = parse_gradient_center(at)?;
        parts.remove(0);
    } else if parts.first().is_some_and(|v| v.starts_with("at ")) {
        center = parse_gradient_center(parts[0].strip_prefix("at ")?)?;
        parts.remove(0);
    } else if parts.first().is_some_and(|v| {
        matches!(
            *v,
            "circle" | "ellipse" | "farthest-corner" | "closest-corner"
        )
    }) {
        parts.remove(0);
    }
    Some(CssGradient {
        angle: 0.0,
        stops: parse_gradient_stops(parts)?,
        radial: true,
        repeating: false,
        conic: false,
        center,
    })
}

fn parse_gradient_center(value: &str) -> Option<[f32; 2]> {
    let parts = value.split_whitespace().collect::<Vec<_>>();
    let coordinate = |value: &str| match value {
        "left" | "top" => Some(0.0),
        "center" => Some(0.5),
        "right" | "bottom" => Some(1.0),
        _ => value
            .strip_suffix('%')?
            .parse::<f32>()
            .ok()
            .map(|v| v / 100.0),
    };
    let (x, y) = match parts.as_slice() {
        [one] if *one == "left" || *one == "right" => (coordinate(one)?, 0.5),
        [one] if *one == "top" || *one == "bottom" => (0.5, coordinate(one)?),
        [one] if *one == "center" => (0.5, 0.5),
        [x, y] => (coordinate(x)?, coordinate(y)?),
        _ => return None,
    };
    Some([x, y])
}

fn parse_gradient_stops(parts: Vec<&str>) -> Option<Vec<([f32; 4], f32)>> {
    if parts.len() < 2 {
        return None;
    }
    let mut stops = Vec::new();
    for part in parts {
        let tokens = split_top_level_whitespace(part);
        let (index, color) = tokens.iter().enumerate().find_map(|(i, token)| {
            parse_color(token)
                .or_else(|| named_color(token))
                .map(|c| (i, c))
        })?;
        let position = tokens
            .get(index + 1)
            .and_then(|v| v.strip_suffix('%'))
            .and_then(|v| v.parse::<f32>().ok())
            .map(|v| v / 100.0);
        stops.push((color, position.unwrap_or(f32::NAN)));
    }
    if stops[0].1.is_nan() {
        stops[0].1 = 0.0;
    }
    let last = stops.len() - 1;
    if stops[last].1.is_nan() {
        stops[last].1 = 1.0;
    }
    let mut previous = stops[0].1;
    let mut missing_start = None;
    for index in 1..=last {
        if stops[index].1.is_nan() {
            missing_start.get_or_insert(index);
        } else {
            if let Some(start) = missing_start.take() {
                let span = (index - start + 1) as f32;
                for missing in start..index {
                    stops[missing].1 = previous
                        + (stops[index].1 - previous) * (missing - start + 1) as f32 / span;
                }
            }
            previous = previous.max(stops[index].1);
        }
    }
    Some(stops)
}

fn named_color(value: &str) -> Option<[f32; 4]> {
    match value {
        "red" => Some([1.0, 0.0, 0.0, 1.0]),
        "green" => Some([0.0, 0.50196, 0.0, 1.0]),
        "blue" => Some([0.0, 0.0, 1.0, 1.0]),
        "yellow" => Some([1.0, 1.0, 0.0, 1.0]),
        "orange" => Some([1.0, 0.64706, 0.0, 1.0]),
        "pink" => Some([1.0, 0.75294, 0.79608, 1.0]),
        "purple" => Some([0.50196, 0.0, 0.50196, 1.0]),
        "gray" | "grey" => Some([0.50196, 0.50196, 0.50196, 1.0]),
        _ => None,
    }
}

pub(super) fn named_color_for_shorthand(value: &str) -> Option<[f32; 4]> {
    named_color(value)
}
