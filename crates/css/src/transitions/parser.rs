use cssparser::{ParseError, Parser, Token};
use lunelle_macros::css_comma_list;

use super::{StepPosition, TimingFunction, Transition, TransitionProperty};

pub fn parse_transition(source: &str) -> Option<Vec<Transition>> {
    css_comma_list!(source, parse_transition_item)
}

fn parse_transition_item(parser: &mut Parser<'_>) -> Result<Transition, ParseError<()>> {
    let mut transition = Transition::default();
    let mut time_count = 0;
    let mut property_seen = false;
    let mut timing_seen = false;
    while !parser.is_exhausted() {
        parser.skip_whitespace();
        if parser.is_exhausted() {
            break;
        }
        if let Ok(time) = parser.try_parse(parse_time) {
            match time_count {
                0 if time >= 0.0 => transition.duration_ms = time,
                0 => return Err(ParseError::unexpected_token()),
                1 => transition.delay_ms = time,
                _ => return Err(ParseError::unexpected_token()),
            }
            time_count += 1;
            continue;
        }
        if !timing_seen {
            if let Ok(timing) = parser.try_parse(parse_timing_function) {
                transition.timing_function = timing;
                timing_seen = true;
                continue;
            }
        }
        if !property_seen {
            let property = parser.expect_ident_cloned()?.to_ascii_lowercase();
            transition.property = match property.as_str() {
                "all" => TransitionProperty::All,
                "none" => TransitionProperty::None,
                _ => TransitionProperty::Property(property),
            };
            property_seen = true;
            continue;
        }
        return Err(ParseError::unexpected_token());
    }
    if !property_seen && time_count == 0 && !timing_seen {
        return Err(ParseError::unexpected_token());
    }
    Ok(transition)
}

pub(crate) fn parse_time(parser: &mut Parser<'_>) -> Result<f32, ParseError<()>> {
    match parser.next()? {
        Token::Dimension { value, unit, .. }
            if value.is_finite() && unit.eq_ignore_ascii_case("ms") =>
        {
            Ok(*value)
        }
        Token::Dimension { value, unit, .. }
            if value.is_finite() && unit.eq_ignore_ascii_case("s") =>
        {
            Ok(*value * 1000.0)
        }
        _ => Err(ParseError::unexpected_token()),
    }
}

pub(crate) fn parse_timing_function(
    parser: &mut Parser<'_>,
) -> Result<TimingFunction, ParseError<()>> {
    if let Ok(ident) = parser.try_parse(|parser| parser.expect_ident_cloned()) {
        return match ident.to_ascii_lowercase().as_str() {
            "ease" => Ok(TimingFunction::Ease),
            "linear" => Ok(TimingFunction::Linear),
            "ease-in" => Ok(TimingFunction::EaseIn),
            "ease-out" => Ok(TimingFunction::EaseOut),
            "ease-in-out" => Ok(TimingFunction::EaseInOut),
            _ => Err(ParseError::unexpected_token()),
        };
    }
    let name = parser.expect_function()?.to_ascii_lowercase();
    parser.parse_nested_block(|arguments| match name.as_str() {
        "cubic-bezier" => {
            let x1 = arguments.expect_number()?;
            arguments.expect_comma()?;
            let y1 = arguments.expect_number()?;
            arguments.expect_comma()?;
            let x2 = arguments.expect_number()?;
            arguments.expect_comma()?;
            let y2 = arguments.expect_number()?;
            arguments.expect_exhausted()?;
            if !(0.0..=1.0).contains(&x1) || !(0.0..=1.0).contains(&x2) {
                return Err(ParseError::unexpected_token());
            }
            Ok(TimingFunction::CubicBezier(x1, y1, x2, y2))
        }
        "steps" => {
            let count = match arguments.next()? {
                Token::Number {
                    int_value: Some(count),
                    ..
                } if *count > 0 => *count as u32,
                _ => return Err(ParseError::unexpected_token()),
            };
            let position = if arguments.try_parse(|parser| parser.expect_comma()).is_ok() {
                let position = arguments.expect_ident()?;
                let position = StepPosition::from_keyword(position.as_ref())
                    .ok_or_else(ParseError::unexpected_token)?;
                if matches!(position, StepPosition::JumpNone) && count <= 1 {
                    return Err(ParseError::unexpected_token());
                }
                position
            } else {
                StepPosition::JumpEnd
            };
            arguments.expect_exhausted()?;
            Ok(TimingFunction::Steps(count, position))
        }
        _ => Err(ParseError::unexpected_token()),
    })
}
