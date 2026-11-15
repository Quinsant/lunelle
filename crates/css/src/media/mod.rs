use cssparser::{Parser, Token};
use lunelle_core::Size;

/// Viewport or device-scale constraint attached to a style rule.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MediaCondition {
    /// Requires a minimum viewport width in pixels.
    MinWidth(f32),
    /// Requires a maximum viewport width in pixels.
    MaxWidth(f32),
    /// Requires a minimum viewport height in pixels.
    MinHeight(f32),
    /// Requires a maximum viewport height in pixels.
    MaxHeight(f32),
    /// Requires a minimum device scale in dppx.
    MinResolution(f32),
    /// Requires a maximum device scale in dppx.
    MaxResolution(f32),
}

impl MediaCondition {
    /// Tests this condition against the current viewport and scale factor.
    pub fn matches(self, context: CssMediaContext) -> bool {
        match self {
            Self::MinWidth(value) => context.viewport.width >= value,
            Self::MaxWidth(value) => context.viewport.width <= value,
            Self::MinHeight(value) => context.viewport.height >= value,
            Self::MaxHeight(value) => context.viewport.height <= value,
            Self::MinResolution(value) => context.scale_factor >= value,
            Self::MaxResolution(value) => context.scale_factor <= value,
        }
    }
}

/// Parses supported `and`-joined media feature conditions.
pub fn parse_media_conditions(source: &str) -> Option<Vec<MediaCondition>> {
    let mut input = Parser::new(source);
    let mut conditions = Vec::new();

    loop {
        input.skip_whitespace();
        if input.is_exhausted() {
            break;
        }

        let Token::ParenthesisBlock = input.next().ok()? else {
            return None;
        };
        let condition = input
            .parse_nested_block(|nested| {
                Ok::<_, cssparser::ParseError<()>>(parse_media_condition(nested))
            })
            .ok()??;
        conditions.push(condition);

        input.skip_whitespace();
        if input.is_exhausted() {
            break;
        }
        let Token::Ident(operator) = input.next().ok()? else {
            return None;
        };
        if !operator.eq_ignore_ascii_case("and") {
            return None;
        }
    }

    (!conditions.is_empty()).then_some(conditions)
}

fn parse_media_condition(input: &mut Parser<'_>) -> Option<MediaCondition> {
    input.skip_whitespace();
    let Token::Ident(feature) = input.next().ok()? else {
        return None;
    };
    let feature = feature.to_ascii_lowercase();
    input.expect_colon().ok()?;
    input.skip_whitespace();
    let Token::Dimension {
        value: number,
        unit,
        ..
    } = input.next().ok()?
    else {
        return None;
    };
    let number = *number;
    let unit = unit.to_ascii_lowercase();
    if !input.is_exhausted() {
        return None;
    }
    if !number.is_finite() || number < 0.0 {
        return None;
    }
    match feature.as_str() {
        "min-width" if unit == "px" => Some(MediaCondition::MinWidth(number)),
        "max-width" if unit == "px" => Some(MediaCondition::MaxWidth(number)),
        "min-height" if unit == "px" => Some(MediaCondition::MinHeight(number)),
        "max-height" if unit == "px" => Some(MediaCondition::MaxHeight(number)),
        "min-resolution" if number > 0.0 && unit == "dppx" => {
            Some(MediaCondition::MinResolution(number))
        }
        "max-resolution" if number > 0.0 && unit == "dppx" => {
            Some(MediaCondition::MaxResolution(number))
        }
        _ => None,
    }
}

/// Viewport size and device scale used to evaluate media conditions.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CssMediaContext {
    /// Current viewport dimensions.
    pub viewport: Size,
    /// Device scale factor in physical pixels per CSS pixel.
    pub scale_factor: f32,
}
