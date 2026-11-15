use std::collections::BTreeMap;

use cssparser::{ParseError, Parser, Token};
use lunelle_macros::css_comma_list;

use super::{
    Animation, AnimationDirection, AnimationFillMode, AnimationIterationCount, AnimationPlayState,
};
use crate::transitions::{parse_time, parse_timing_function};

/// Resolves computed animation declarations into typed entries for timeline
/// evaluation. Shorter longhand lists repeat their final item as CSS requires.
pub fn resolve_animations(properties: &BTreeMap<String, String>) -> Option<Vec<Animation>> {
    let shorthand = match properties.get("animation") {
        Some(value) => Some(parse_animation(value)?),
        None => None,
    };
    let names = match properties.get("animation-name") {
        Some(value) => Some(css_comma_list!(
            value,
            |parser: &mut Parser<'_>| -> Result<String, ParseError<()>> {
                match parser.next()? {
                    Token::Ident(name) => Ok(name.to_string()),
                    Token::QuotedString(name) => Ok(name.to_string()),
                    _ => Err(ParseError::unexpected_token()),
                }
            }
        )?),
        None => None,
    };

    if shorthand.is_none() && names.is_none() {
        return Some(Vec::new());
    }
    let count = names
        .as_ref()
        .map_or_else(|| shorthand.as_ref().map_or(0, Vec::len), Vec::len);
    let mut animations = (0..count)
        .map(|index| {
            shorthand
                .as_ref()
                .and_then(|items| items.get(index % items.len().max(1)))
                .cloned()
                .unwrap_or_default()
        })
        .collect::<Vec<_>>();
    if let Some(names) = names {
        for (index, animation) in animations.iter_mut().enumerate() {
            animation.name = names[index].clone();
        }
    }

    apply_longhand(
        properties,
        "animation-duration",
        &mut animations,
        parse_nonnegative_time,
        |animation, value| animation.duration_ms = value,
    )?;
    apply_longhand(
        properties,
        "animation-delay",
        &mut animations,
        parse_time,
        |animation, value| animation.delay_ms = value,
    )?;
    apply_longhand(
        properties,
        "animation-timing-function",
        &mut animations,
        parse_timing_function,
        |animation, value| animation.timing_function = value,
    )?;
    apply_longhand(
        properties,
        "animation-iteration-count",
        &mut animations,
        parse_iteration_count,
        |animation, value| animation.iteration_count = value,
    )?;
    apply_longhand(
        properties,
        "animation-direction",
        &mut animations,
        parse_direction,
        |animation, value| animation.direction = value,
    )?;
    apply_longhand(
        properties,
        "animation-fill-mode",
        &mut animations,
        parse_fill_mode,
        |animation, value| animation.fill_mode = value,
    )?;
    apply_longhand(
        properties,
        "animation-play-state",
        &mut animations,
        parse_play_state,
        |animation, value| animation.play_state = value,
    )?;
    Some(animations)
}

fn apply_longhand<T: Copy>(
    properties: &BTreeMap<String, String>,
    property: &str,
    animations: &mut [Animation],
    parse: impl Fn(&mut Parser<'_>) -> Result<T, ParseError<()>>,
    mut set: impl FnMut(&mut Animation, T),
) -> Option<()> {
    let Some(source) = properties.get(property) else {
        return Some(());
    };
    let values = css_comma_list!(source, parse)?;
    for (index, animation) in animations.iter_mut().enumerate() {
        set(animation, values[index % values.len()]);
    }
    Some(())
}

fn parse_nonnegative_time(parser: &mut Parser<'_>) -> Result<f32, ParseError<()>> {
    let value = parse_time(parser)?;
    if value >= 0.0 {
        Ok(value)
    } else {
        Err(ParseError::unexpected_token())
    }
}

fn parse_direction(parser: &mut Parser<'_>) -> Result<AnimationDirection, ParseError<()>> {
    AnimationDirection::from_keyword(parser.expect_ident()?.as_ref())
        .ok_or_else(ParseError::unexpected_token)
}

fn parse_fill_mode(parser: &mut Parser<'_>) -> Result<AnimationFillMode, ParseError<()>> {
    AnimationFillMode::from_keyword(parser.expect_ident()?.as_ref())
        .ok_or_else(ParseError::unexpected_token)
}

fn parse_play_state(parser: &mut Parser<'_>) -> Result<AnimationPlayState, ParseError<()>> {
    AnimationPlayState::from_keyword(parser.expect_ident()?.as_ref())
        .ok_or_else(ParseError::unexpected_token)
}

/// Parses the CSS `animation` shorthand into one entry per comma-separated animation.
pub fn parse_animation(source: &str) -> Option<Vec<Animation>> {
    css_comma_list!(source, parse_animation_item)
}

fn parse_animation_item(parser: &mut Parser<'_>) -> Result<Animation, ParseError<()>> {
    let mut animation = Animation::default();
    let mut duration_seen = false;
    let mut delay_seen = false;
    let mut timing_seen = false;
    let mut iteration_seen = false;
    let mut direction_seen = false;
    let mut fill_seen = false;
    let mut play_state_seen = false;
    let mut name_seen = false;

    while !parser.is_exhausted() {
        parser.skip_whitespace();
        if parser.is_exhausted() {
            break;
        }
        if !timing_seen {
            if let Ok(timing) = parser.try_parse(parse_timing_function) {
                animation.timing_function = timing;
                timing_seen = true;
                continue;
            }
        }
        if let Ok(time) = parser.try_parse(parse_time) {
            if !duration_seen {
                if time < 0.0 {
                    return Err(ParseError::unexpected_token());
                }
                animation.duration_ms = time;
                duration_seen = true;
            } else if !delay_seen {
                animation.delay_ms = time;
                delay_seen = true;
            } else {
                return Err(ParseError::unexpected_token());
            }
            continue;
        }
        if !iteration_seen {
            if let Ok(count) = parser.try_parse(parse_iteration_count) {
                animation.iteration_count = count;
                iteration_seen = true;
                continue;
            }
        }
        let (ident, quoted_name) = match parser.next()? {
            Token::Ident(value) => (value.to_string(), false),
            Token::QuotedString(value) => (value.to_string(), true),
            _ => return Err(ParseError::unexpected_token()),
        };
        if !quoted_name {
            match ident.to_ascii_lowercase().as_str() {
                "normal" | "reverse" | "alternate" | "alternate-reverse" if !direction_seen => {
                    animation.direction = match ident.to_ascii_lowercase().as_str() {
                        "reverse" => AnimationDirection::Reverse,
                        "alternate" => AnimationDirection::Alternate,
                        "alternate-reverse" => AnimationDirection::AlternateReverse,
                        _ => AnimationDirection::Normal,
                    };
                    direction_seen = true;
                    continue;
                }
                "none" | "forwards" | "backwards" | "both" if !fill_seen => {
                    animation.fill_mode = match ident.to_ascii_lowercase().as_str() {
                        "forwards" => AnimationFillMode::Forwards,
                        "backwards" => AnimationFillMode::Backwards,
                        "both" => AnimationFillMode::Both,
                        _ => AnimationFillMode::None,
                    };
                    fill_seen = true;
                    if ident.eq_ignore_ascii_case("none") {
                        animation.name = ident;
                        name_seen = true;
                    }
                    continue;
                }
                "running" | "paused" if !play_state_seen => {
                    animation.play_state = if ident.eq_ignore_ascii_case("paused") {
                        AnimationPlayState::Paused
                    } else {
                        AnimationPlayState::Running
                    };
                    play_state_seen = true;
                    continue;
                }
                _ => {}
            }
        }
        if name_seen {
            return Err(ParseError::unexpected_token());
        }
        animation.name = ident;
        name_seen = true;
    }

    if !(duration_seen
        || delay_seen
        || timing_seen
        || iteration_seen
        || direction_seen
        || fill_seen
        || play_state_seen
        || name_seen)
    {
        return Err(ParseError::unexpected_token());
    }
    Ok(animation)
}

fn parse_iteration_count(
    parser: &mut Parser<'_>,
) -> Result<AnimationIterationCount, ParseError<()>> {
    if let Ok(ident) = parser.try_parse(|parser| parser.expect_ident_cloned()) {
        return if ident.eq_ignore_ascii_case("infinite") {
            Ok(AnimationIterationCount::Infinite)
        } else {
            Err(ParseError::unexpected_token())
        };
    }
    match parser.next()? {
        Token::Number { value, .. } if value.is_finite() && *value >= 0.0 => {
            Ok(AnimationIterationCount::Count(*value))
        }
        _ => Err(ParseError::unexpected_token()),
    }
}
