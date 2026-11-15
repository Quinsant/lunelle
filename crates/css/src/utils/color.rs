use crate::ComputedStyle;

/// Parses the supported named, hex, and `rgba()` color forms into normalized RGBA.
pub fn parse_color(value: &str) -> Option<[f32; 4]> {
    let value = value.trim();
    match value {
        "transparent" => Some([0.0, 0.0, 0.0, 0.0]),
        "white" => Some([1.0, 1.0, 1.0, 1.0]),
        "black" => Some([0.0, 0.0, 0.0, 1.0]),
        _ if value.starts_with("rgb(") || value.starts_with("rgba(") => parse_rgb(value),
        _ if value.starts_with('#') && value.len() == 7 => {
            let red = u8::from_str_radix(&value[1..3], 16).ok()?;
            let green = u8::from_str_radix(&value[3..5], 16).ok()?;
            let blue = u8::from_str_radix(&value[5..7], 16).ok()?;
            Some([
                red as f32 / 255.0,
                green as f32 / 255.0,
                blue as f32 / 255.0,
                1.0,
            ])
        }
        _ => None,
    }
}

fn parse_rgb(value: &str) -> Option<[f32; 4]> {
    let body = value
        .strip_prefix("rgba(")
        .or_else(|| value.strip_prefix("rgb("))?
        .strip_suffix(')')?;
    let normalized = body.replace(',', " ").replace('/', " /");
    let mut channels = normalized.split_whitespace();
    let parse_channel = |value: &str| {
        value
            .strip_suffix('%')
            .and_then(|value| value.parse::<f32>().ok())
            .map(|value| value / 100.0)
            .or_else(|| value.parse::<f32>().ok().map(|value| value / 255.0))
    };
    let parse_alpha = |value: &str| {
        value
            .strip_suffix('%')
            .and_then(|value| value.parse::<f32>().ok())
            .map(|value| value / 100.0)
            .or_else(|| value.parse::<f32>().ok())
    };
    let red = parse_channel(channels.next()?)?;
    let green = parse_channel(channels.next()?)?;
    let blue = parse_channel(channels.next()?)?;
    let alpha = match channels.next() {
        None => 1.0,
        Some("/") => channels.next().and_then(parse_alpha).unwrap_or(1.0),
        Some(value) => parse_alpha(value).unwrap_or(1.0),
    };
    Some([
        red.clamp(0.0, 1.0),
        green.clamp(0.0, 1.0),
        blue.clamp(0.0, 1.0),
        alpha.clamp(0.0, 1.0),
    ])
}

/// Reads computed opacity, defaulting to fully opaque and clamping to `[0, 1]`.
pub fn style_opacity(style: &ComputedStyle) -> f32 {
    style
        .properties
        .get("opacity")
        .and_then(|value| value.parse::<f32>().ok())
        .filter(|value| value.is_finite())
        .unwrap_or(1.0)
        .clamp(0.0, 1.0)
}
