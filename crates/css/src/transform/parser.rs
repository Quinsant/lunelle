use cssparser::{ParseError, Parser};

use super::{TransformFunction, TransformLength};

/// Parses supported CSS transform functions.
pub fn parse_transform(source: &str) -> Option<Vec<TransformFunction>> {
    let source = source.trim();
    if source.eq_ignore_ascii_case("none") {
        return Some(Vec::new());
    }
    let mut parser = Parser::new(source);
    let mut functions = Vec::new();
    while !parser.is_exhausted() {
        parser.skip_whitespace();
        if parser.is_exhausted() {
            break;
        }
        let function = parser.expect_function().ok()?;
        let name = function.as_ref().to_ascii_lowercase();
        let parsed = parser
            .parse_nested_block(|arguments| {
                parse_transform_function(&name, arguments)
                    .ok_or_else(ParseError::<()>::unexpected_token)
            })
            .ok()?;
        functions.push(parsed);
    }
    Some(functions)
}

fn parse_transform_function(name: &str, arguments: &mut Parser<'_>) -> Option<TransformFunction> {
    let result = match name {
        "translate" | "translatex" | "translatey" => {
            let first = parse_length(arguments)?;
            let second = if name == "translate" {
                arguments.skip_whitespace();
                if arguments.is_exhausted() {
                    TransformLength::Px(0.0)
                } else {
                    arguments.expect_comma().ok()?;
                    parse_length(arguments)?
                }
            } else {
                TransformLength::Px(0.0)
            };
            arguments.expect_exhausted().ok()?;
            match name {
                "translatex" => TransformFunction::Translate(first, TransformLength::Px(0.0)),
                "translatey" => TransformFunction::Translate(TransformLength::Px(0.0), first),
                _ => TransformFunction::Translate(first, second),
            }
        }
        "scale" | "scalex" | "scaley" => {
            let first = parse_number(arguments)?;
            arguments.skip_whitespace();
            let second = if name == "scale" && !arguments.is_exhausted() {
                arguments.expect_comma().ok()?;
                parse_number(arguments)?
            } else {
                first
            };
            arguments.expect_exhausted().ok()?;
            match name {
                "scalex" => TransformFunction::Scale(first, 1.0),
                "scaley" => TransformFunction::Scale(1.0, first),
                _ => TransformFunction::Scale(first, second),
            }
        }
        "rotate" => {
            let angle = parse_angle(arguments)?;
            arguments.expect_exhausted().ok()?;
            TransformFunction::Rotate(angle)
        }
        "skew" | "skewx" | "skewy" => {
            let first = parse_angle(arguments)?;
            arguments.skip_whitespace();
            let second = if name == "skew" && !arguments.is_exhausted() {
                arguments.expect_comma().ok()?;
                parse_angle(arguments)?
            } else {
                0.0
            };
            arguments.expect_exhausted().ok()?;
            match name {
                "skewx" => TransformFunction::Skew(first, 0.0),
                "skewy" => TransformFunction::Skew(0.0, first),
                _ => TransformFunction::Skew(first, second),
            }
        }
        _ => return None,
    };
    Some(result)
}

fn parse_length(parser: &mut Parser<'_>) -> Option<TransformLength> {
    match parser.next().ok()? {
        cssparser::Token::Dimension { value, unit, .. } if unit.eq_ignore_ascii_case("px") => {
            Some(TransformLength::Px(*value))
        }
        cssparser::Token::Percentage { unit_value, .. } => {
            Some(TransformLength::Percent(*unit_value * 100.0))
        }
        cssparser::Token::Number { value, .. } if *value == 0.0 => Some(TransformLength::Px(0.0)),
        _ => None,
    }
}

fn parse_number(parser: &mut Parser<'_>) -> Option<f32> {
    match parser.next().ok()? {
        cssparser::Token::Number { value, .. } if value.is_finite() => Some(*value),
        _ => None,
    }
}

fn parse_angle(parser: &mut Parser<'_>) -> Option<f32> {
    match parser.next().ok()? {
        cssparser::Token::Dimension { value, unit, .. } if unit.eq_ignore_ascii_case("deg") => {
            Some(value.to_radians())
        }
        cssparser::Token::Dimension { value, unit, .. } if unit.eq_ignore_ascii_case("rad") => {
            Some(*value)
        }
        cssparser::Token::Dimension { value, unit, .. } if unit.eq_ignore_ascii_case("turn") => {
            Some(*value * std::f32::consts::TAU)
        }
        cssparser::Token::Number { value, .. } if *value == 0.0 => Some(0.0),
        _ => None,
    }
}
