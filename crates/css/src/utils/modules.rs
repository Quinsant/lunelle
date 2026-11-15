use std::collections::BTreeMap;

use super::{split_top_level_once, split_top_level_whitespace};

/// Parses a finite numeric declaration value.
pub(crate) fn number(value: Option<&String>) -> Option<f32> {
    value?.parse::<f32>().ok().filter(|value| value.is_finite())
}

/// Parses non-negative pixel/rem values used by gap-like properties.
pub(crate) fn css_number(value: &str) -> Option<f32> {
    let value = value.trim();
    value
        .strip_suffix("px")
        .or_else(|| value.strip_suffix("rem"))
        .and_then(|value| value.trim().parse::<f32>().ok())
        .map(|value| {
            if value.is_finite() && value >= 0.0 {
                value
            } else {
                0.0
            }
        })
}

/// Extracts a concrete pixel value from a typed length.
pub(crate) fn length_number(value: crate::Length) -> Option<f32> {
    match value {
        crate::Length::Px(value) if value.is_finite() => Some(value),
        _ => None,
    }
}

/// Resolves a longhand or four-value shorthand for one box-model side.
pub(crate) fn edge_value(
    properties: &BTreeMap<String, String>,
    shorthand: &str,
    side: &str,
) -> Option<crate::Length> {
    properties
        .get(&format!("{shorthand}-{side}"))
        .and_then(|value| length_string(value))
        .or_else(|| {
            let values = properties
                .get(shorthand)
                .map(String::as_str)
                .map(split_top_level_whitespace)?;
            let index = match values.len() {
                1 => 0,
                2 => {
                    if matches!(side, "top" | "bottom") {
                        0
                    } else {
                        1
                    }
                }
                3 => match side {
                    "top" => 0,
                    "right" | "left" => 1,
                    _ => 2,
                },
                4 => match side {
                    "top" => 0,
                    "right" => 1,
                    "bottom" => 2,
                    _ => 3,
                },
                _ => return None,
            };
            length_string(values[index])
        })
}

/// Parses an integer-valued grid line.
pub(crate) fn integer(value: Option<&String>) -> Option<i32> {
    value?.trim().parse::<i32>().ok()
}

/// Parses a positive grid span.
pub(crate) fn span(value: Option<&String>) -> Option<usize> {
    value?
        .trim()
        .parse::<usize>()
        .ok()
        .filter(|value| *value > 0)
}

/// Parses `start / end` or `start / span count` grid placement syntax.
pub(crate) fn placement(value: &str) -> Option<(Option<i32>, Option<i32>, Option<usize>)> {
    let (start, end) = split_top_level_once(value, '/')?;
    let start = integer_string(start.trim());
    let end = end.trim();
    if let Some(span) = end
        .strip_prefix("span ")
        .and_then(|value| value.trim().parse::<usize>().ok())
    {
        return Some((start, None, (span > 0).then_some(span)));
    }
    let end = integer_string(end);
    let span = match (start, end) {
        (Some(start), Some(end)) if end > start => usize::try_from(end - start).ok(),
        _ => None,
    };
    Some((start, end, span))
}

/// Parses an integer from an already trimmed grid token.
pub(crate) fn integer_string(value: &str) -> Option<i32> {
    value.parse::<i32>().ok()
}

/// Expands and parses a grid track list, including bounded `repeat()` syntax.
pub(crate) fn grid_tracks(value: Option<&String>) -> Vec<crate::GridTrack> {
    let Some(value) = value.map(String::as_str).map(str::trim) else {
        return Vec::new();
    };
    if let Some(inner) = value
        .strip_prefix("repeat(")
        .and_then(|value| value.strip_suffix(')'))
    {
        let Some((count, tracks)) = split_top_level_once(inner, ',') else {
            return Vec::new();
        };
        let count = count
            .trim()
            .parse::<usize>()
            .ok()
            .filter(|count| *count > 0)
            .unwrap_or(1)
            .min(32);
        let tracks = grid_tracks(Some(&tracks.trim().to_owned()));
        return (0..count).flat_map(|_| tracks.iter().copied()).collect();
    }
    split_top_level_whitespace(value)
        .into_iter()
        .filter_map(grid_track)
        .collect()
}

/// Parses one grid track such as `auto`, `1fr`, or `minmax(...)`.
pub(crate) fn grid_track(value: &str) -> Option<crate::GridTrack> {
    let value = value.trim();
    if value == "auto" {
        return Some(crate::GridTrack::Auto);
    }
    if let Some(value) = value
        .strip_suffix("fr")
        .and_then(|value| value.trim().parse::<f32>().ok())
    {
        return value
            .is_finite()
            .then_some(crate::GridTrack::Fraction(value));
    }
    if let Some(value) = length_string(value) {
        return Some(crate::GridTrack::Length(value));
    }
    let inner = value
        .strip_prefix("minmax(")
        .and_then(|value| value.strip_suffix(')'))?;
    let (minimum, maximum) = split_top_level_once(inner, ',')?;
    let minimum = length_string(minimum.trim()).unwrap_or(crate::Length::Auto);
    if maximum.trim() == "auto" {
        return Some(crate::GridTrack::MinMaxAuto(minimum));
    }
    if let Some(value) = maximum
        .trim()
        .strip_suffix("fr")
        .and_then(|value| value.trim().parse::<f32>().ok())
    {
        return value
            .is_finite()
            .then_some(crate::GridTrack::MinMaxFraction(minimum, value));
    }
    Some(crate::GridTrack::MinMax(
        minimum,
        length_string(maximum.trim()).unwrap_or(crate::Length::Auto),
    ))
}

/// Parses the length subset accepted by grid track and shorthand conversion.
pub(crate) fn length_string(value: &str) -> Option<crate::Length> {
    if value == "0" {
        return Some(crate::Length::Px(0.0));
    }
    if value == "auto" {
        return Some(crate::Length::Auto);
    }
    value
        .strip_suffix("px")
        .and_then(|value| value.trim().parse().ok())
        .map(crate::Length::Px)
        .or_else(|| {
            value
                .strip_suffix('%')
                .and_then(|value| value.trim().parse().ok())
                .map(crate::Length::Percent)
        })
}

/// Converts the `grid-auto-flow` keyword into its typed representation.
pub(crate) fn grid_auto_flow(value: Option<&String>) -> crate::GridAutoFlow {
    value
        .and_then(|value| crate::GridAutoFlow::from_keyword(value))
        .unwrap_or_default()
}

/// Converts a CSS length declaration into the typed layout length model.
pub(crate) fn length(value: Option<&String>) -> crate::Length {
    let Some(value) = value.map(String::as_str).map(str::trim) else {
        return crate::Length::Auto;
    };
    if value == "auto" {
        return crate::Length::Auto;
    }
    if let Some(value) = value
        .strip_suffix("px")
        .and_then(|value| value.trim().parse().ok())
    {
        return crate::Length::Px(value);
    }
    if let Some(value) = value
        .strip_suffix('%')
        .and_then(|value| value.trim().parse().ok())
    {
        return crate::Length::Percent(value);
    }
    crate::Length::Auto
}

/// Converts a justification keyword, defaulting to flex-start.
pub(crate) fn justify_content(value: Option<&String>) -> crate::JustifyContent {
    value
        .and_then(|value| crate::JustifyContent::from_keyword(value))
        .unwrap_or_default()
}

/// Converts an alignment keyword, defaulting to stretch.
pub(crate) fn align_items(value: Option<&String>) -> crate::AlignItems {
    value
        .and_then(|value| crate::AlignItems::from_keyword(value))
        .unwrap_or_default()
}
