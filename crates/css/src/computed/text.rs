use crate::utils::css_length;
use crate::{ComputedStyle, parse_color, style_opacity};
use lunelle_macros::css_keyword_enum;

#[css_keyword_enum(keywords(
    "left" = Left,
    "start" = Left,
    "center" = Center,
    "right" = Right,
    "end" = Right,
    "justify" = Justified,
))]
/// Text alignment after CSS keyword normalization.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TextAlignment {
    #[default]
    /// No explicit alignment was specified.
    Default,
    /// Aligns text to the left.
    Left,
    /// Centers text.
    Center,
    /// Aligns text to the right.
    Right,
    /// Justifies text between both edges.
    Justified,
}

/// Resolved font width used by the text renderer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FontStretch {
    /// Uses the font's normal width.
    Normal,
    /// Uses a narrower font face when available.
    Condensed,
    /// Uses a wider font face when available.
    Expanded,
}

/// Rendering style for text decoration lines.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TextDecorationStyle {
    #[default]
    Solid,
    Double,
    Dotted,
    Dashed,
    Wavy,
}

/// Thickness of text decoration lines.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TextDecorationThickness {
    Auto,
    Px(f32),
}

impl Default for TextDecorationThickness {
    fn default() -> Self {
        Self::Auto
    }
}

/// Text decoration lines and paint properties requested by a computed style.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TextDecoration {
    /// Draws a line below the text.
    pub underline: bool,
    /// Draws a line above the text.
    pub overline: bool,
    /// Draws a line through the text.
    pub line_through: bool,
    /// Resolved decoration color.
    pub color: [u8; 4],
    /// Decoration stroke pattern.
    pub style: TextDecorationStyle,
    /// Resolved decoration thickness.
    pub thickness: TextDecorationThickness,
}

/// Text casing transformation applied before shaping glyphs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TextTransform {
    #[default]
    None,
    Uppercase,
    Lowercase,
    Capitalize,
}

/// Resolved text color, metrics, style, weight, and alignment.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextPresentation {
    /// Effective RGBA text color.
    pub color: [u8; 4],
    /// Font size in pixels.
    pub font_size: f32,
    /// Resolved line height in pixels.
    pub line_height: f32,
    /// Whether the font is italic or oblique.
    pub italic: bool,
    /// Optional numeric font weight.
    pub weight: Option<u16>,
    /// Resolved font width.
    pub stretch: FontStretch,
    /// Requested text decoration lines.
    pub decoration: TextDecoration,
    /// Casing transformation applied before text shaping.
    pub transform: TextTransform,
    /// Resolved horizontal alignment.
    pub alignment: TextAlignment,
}

/// Converts serialized text properties into renderer-facing values.
pub fn text_presentation(style: &ComputedStyle) -> TextPresentation {
    let color = style
        .properties
        .get("color")
        .and_then(|value| parse_color(value))
        .map_or([17, 24, 39, 255], |[red, green, blue, alpha]| {
            [
                (red * 255.0) as u8,
                (green * 255.0) as u8,
                (blue * 255.0) as u8,
                (alpha * 255.0) as u8,
            ]
        });
    let opacity = (style_opacity(style) * 255.0).round() as u8;
    let font_size = style
        .properties
        .get("font-size")
        .and_then(|value| css_length(value))
        .unwrap_or(16.0);
    let line_height = style
        .properties
        .get("line-height")
        .and_then(|value| css_length(value))
        .unwrap_or(font_size * 1.4);
    let italic = matches!(
        style.properties.get("font-style").map(String::as_str),
        Some("italic" | "oblique")
    );
    let weight = style
        .properties
        .get("font-weight")
        .and_then(|value| value.parse::<u16>().ok())
        .or_else(|| {
            matches!(
                style.properties.get("font-weight").map(String::as_str),
                Some("bold" | "bolder")
            )
            .then_some(700)
        });
    let alignment = style
        .properties
        .get("text-align")
        .and_then(|value| TextAlignment::from_keyword(value))
        .unwrap_or_default();
    let stretch = match style.properties.get("font-stretch").map(String::as_str) {
        Some("ultra-condensed" | "extra-condensed" | "condensed" | "semi-condensed") => {
            FontStretch::Condensed
        }
        Some("semi-expanded" | "expanded" | "extra-expanded" | "ultra-expanded") => {
            FontStretch::Expanded
        }
        Some(value)
            if value
                .strip_suffix('%')
                .and_then(|value| value.parse::<f32>().ok())
                .is_some_and(|value| value < 100.0) =>
        {
            FontStretch::Condensed
        }
        Some(value)
            if value
                .strip_suffix('%')
                .and_then(|value| value.parse::<f32>().ok())
                .is_some_and(|value| value > 100.0) =>
        {
            FontStretch::Expanded
        }
        _ => FontStretch::Normal,
    };
    let decoration_value = style
        .properties
        .get("text-decoration-line")
        .or_else(|| style.properties.get("text-decoration"));
    let decoration = decoration_value
        .map(|value| {
            let mut decoration = TextDecoration::default();
            for token in value.split_ascii_whitespace() {
                match token {
                    "underline" => decoration.underline = true,
                    "overline" => decoration.overline = true,
                    "line-through" => decoration.line_through = true,
                    _ => {}
                }
            }
            decoration
        })
        .unwrap_or_default();
    let decoration_color = style
        .properties
        .get("text-decoration-color")
        .and_then(|value| parse_color(value))
        .unwrap_or_else(|| {
            [
                color[0] as f32 / 255.0,
                color[1] as f32 / 255.0,
                color[2] as f32 / 255.0,
                color[3] as f32 / 255.0,
            ]
        });
    let decoration_style = match style
        .properties
        .get("text-decoration-style")
        .map(String::as_str)
    {
        Some("double") => TextDecorationStyle::Double,
        Some("dotted") => TextDecorationStyle::Dotted,
        Some("dashed") => TextDecorationStyle::Dashed,
        Some("wavy") => TextDecorationStyle::Wavy,
        _ => TextDecorationStyle::Solid,
    };
    let decoration_thickness = style
        .properties
        .get("text-decoration-thickness")
        .and_then(|value| css_length(value))
        .map(TextDecorationThickness::Px)
        .unwrap_or(TextDecorationThickness::Auto);
    let decoration = TextDecoration {
        color: [
            (decoration_color[0] * 255.0) as u8,
            (decoration_color[1] * 255.0) as u8,
            (decoration_color[2] * 255.0) as u8,
            (decoration_color[3] * 255.0 * style_opacity(style)) as u8,
        ],
        style: decoration_style,
        thickness: decoration_thickness,
        ..decoration
    };
    let transform = match style.properties.get("text-transform").map(String::as_str) {
        Some("uppercase") => TextTransform::Uppercase,
        Some("lowercase") => TextTransform::Lowercase,
        Some("capitalize") => TextTransform::Capitalize,
        _ => TextTransform::None,
    };
    TextPresentation {
        color: [color[0], color[1], color[2], opacity],
        font_size,
        line_height,
        italic,
        weight,
        stretch,
        decoration,
        transform,
        alignment,
    }
}

/// Estimates the intrinsic text box used by layout when no explicit size exists.
pub fn text_intrinsic_size(state: &crate::CssComputedState, text: &str) -> (f32, f32) {
    let width = text.chars().count() as f32 * state.text.font_size * 0.72;
    (
        width.max(state.text.font_size),
        state.text.font_size * 1.4 + 16.0,
    )
}
