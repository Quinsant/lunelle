mod gradient;

pub use gradient::{CssGradient, CssShadow, parse_gradient, parse_shadows};

use crate::utils::{css_length, split_top_level_whitespace};
use crate::{ComputedStyle, parse_color};

/// Resolves supported border-radius declarations to four pixel radii.
pub fn border_radii(style: &ComputedStyle, width: f32, height: f32) -> [f32; 4] {
    let shorthand = style
        .properties
        .get("border-radius")
        .map(|value| {
            split_top_level_whitespace(value)
                .into_iter()
                .filter_map(css_length)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let mut radii = [
        shorthand.first().copied().unwrap_or(0.0),
        shorthand
            .get(1)
            .copied()
            .unwrap_or_else(|| shorthand.first().copied().unwrap_or(0.0)),
        shorthand
            .get(2)
            .copied()
            .unwrap_or_else(|| shorthand.first().copied().unwrap_or(0.0)),
        shorthand.get(3).copied().unwrap_or_else(|| {
            shorthand
                .get(1)
                .copied()
                .unwrap_or_else(|| shorthand.first().copied().unwrap_or(0.0))
        }),
    ];
    for (index, corner) in ["top-left", "top-right", "bottom-right", "bottom-left"]
        .iter()
        .enumerate()
    {
        if let Some(radius) = style
            .properties
            .get(&format!("border-{corner}-radius"))
            .and_then(|value| css_length(value))
        {
            radii[index] = radius;
        }
    }
    let scale = 1.0_f32
        .min(width / (radii[0] + radii[1]).max(1.0))
        .min(width / (radii[3] + radii[2]).max(1.0))
        .min(height / (radii[0] + radii[3]).max(1.0))
        .min(height / (radii[1] + radii[2]).max(1.0));
    radii.map(|radius| (radius * scale).max(0.0))
}

/// Reads one edge length from a computed shorthand or longhand property.
pub fn edge_length(style: &ComputedStyle, shorthand: &str) -> f32 {
    let shorthand_value = style
        .properties
        .get(shorthand)
        .and_then(|value| value.split_whitespace().next())
        .and_then(css_length);
    ["top", "right", "bottom", "left"]
        .iter()
        .filter_map(|side| {
            style
                .properties
                .get(&format!("{shorthand}-{side}"))
                .and_then(|value| css_length(value))
        })
        .fold(shorthand_value.unwrap_or(0.0), f32::max)
}

/// Finds a supported color token inside a shorthand declaration.
pub fn shorthand_color(value: &str) -> Option<[f32; 4]> {
    split_top_level_whitespace(value)
        .into_iter()
        .find_map(|value| parse_color(value).or_else(|| gradient::named_color_for_shorthand(value)))
}
