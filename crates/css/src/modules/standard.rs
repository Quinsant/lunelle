use lunelle_macros::css_module;

/// Validates values for standard declarations before they enter computed styles.
fn parse_property(name: &str, value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    let valid = match name {
        "display" => matches!(
            value.to_ascii_lowercase().as_str(),
            "block" | "flex" | "grid" | "none" | "inline" | "inline-block"
        ),
        "opacity" => value
            .parse::<f32>()
            .ok()
            .is_some_and(|value| value.is_finite() && (0.0..=1.0).contains(&value)),
        "flex-grow" | "flex-shrink" => value
            .parse::<f32>()
            .ok()
            .is_some_and(|value| value.is_finite() && value >= 0.0),
        "transform" => crate::parse_transform(value).is_some(),
        "transform-origin" => crate::parse_transform_origin(value, 100.0, 100.0).is_some(),
        "transition" => crate::parse_transition(value).is_some(),
        "animation" => crate::parse_animation(value).is_some(),
        "text-decoration-line" => value.split_ascii_whitespace().all(|token| {
            matches!(
                token.to_ascii_lowercase().as_str(),
                "none" | "underline" | "overline" | "line-through"
            )
        }),
        "text-decoration-style" => matches!(
            value.to_ascii_lowercase().as_str(),
            "solid" | "double" | "dotted" | "dashed" | "wavy"
        ),
        "text-decoration-thickness" => {
            value.eq_ignore_ascii_case("auto") || crate::css_length(value).is_some()
        }
        "text-decoration-color" => crate::parse_color(value).is_some(),
        _ => true,
    };
    valid.then(|| value.to_owned())
}

#[css_module(name = "base", parser = parse_property, properties(
    "display",
    "width",
    "height",
    "min-width",
    "max-width",
    "min-height",
    "max-height",
    "position",
    "left",
    "top",
    "right",
    "bottom",
    "overflow",
    "overflow-x",
    "overflow-y",
    "visibility",
    "margin",
    "margin-top",
    "margin-right",
    "margin-bottom",
    "margin-left",
    "padding",
    "padding-top",
    "padding-right",
    "padding-bottom",
    "padding-left",
    "gap",
    "row-gap",
    "column-gap",
    "border",
    "border-width",
    "border-color",
    "border-radius",
    "box-shadow",
    "background",
    "background-color",
    "background-image",
    "opacity",
    "border-style",
    "border-top-width",
    "border-right-width",
    "border-bottom-width",
    "border-left-width",
    "border-top-color",
    "border-right-color",
    "border-bottom-color",
    "border-left-color",
    "border-top-left-radius",
    "border-top-right-radius",
    "border-bottom-right-radius",
    "border-bottom-left-radius"
))]
/// Standard box, sizing, positioning, overflow, and appearance properties.
pub struct CssBase;

#[css_module(name = "flexbox", parser = parse_property, properties(
    "flex-direction",
    "justify-content",
    "align-content",
    "justify-items",
    "justify-self",
    "align-items",
    "align-self",
    "flex-wrap",
    "flex-basis",
    "flex",
    "flex-grow",
    "flex-shrink"
))]
/// Standard flexbox property group.
pub struct CssFlexbox;

#[css_module(name = "grid", parser = parse_property, properties(
    "grid-template-columns",
    "grid-template-rows",
    "grid-template-areas",
    "grid-auto-columns",
    "grid-auto-rows",
    "grid-column-start",
    "grid-row-start",
    "grid-column-span",
    "grid-row-span",
    "grid-column",
    "grid-row",
    "grid-area",
    "grid-auto-flow"
))]
/// Standard grid template, placement, and auto-flow property group.
pub struct CssGrid;

#[css_module(name = "transform", parser = parse_property, properties(
    "transform",
    "transform-origin"
))]
/// Standard transform and transform-origin property group.
pub struct CssTransform;

#[css_module(name = "media", properties())]
/// Module hook for media-dependent style computation.
pub struct CssMedia;

#[css_module(name = "property", parser = parse_property, properties(
    "transition",
    "transition-property",
    "transition-duration",
    "transition-timing-function",
    "transition-delay",
    "animation",
    "animation-name",
    "animation-duration",
    "animation-timing-function",
    "animation-delay",
    "animation-iteration-count",
    "animation-direction",
    "animation-fill-mode",
    "animation-play-state"
))]
/// Transition and animation declaration property group.
pub struct CssProperty;

#[css_module(name = "text", parser = parse_property, properties(
    "text-align",
    "font-size",
    "line-height",
    "white-space",
    "color",
    "font-family",
    "font-weight",
    "font-style",
    "font-stretch",
    "text-decoration",
    "text-decoration-line",
    "text-decoration-color",
    "text-decoration-style",
    "text-decoration-thickness",
    "text-transform"
))]
/// Standard text, font, and color property group.
pub struct CssText;

impl CssMedia {
    /// Returns true when every condition matches; an empty list always matches.
    pub fn matches(
        conditions: &[crate::MediaCondition],
        context: Option<crate::CssMediaContext>,
    ) -> bool {
        conditions.is_empty()
            || context.is_some_and(|context| {
                conditions
                    .iter()
                    .copied()
                    .all(|condition| condition.matches(context))
            })
    }
}
