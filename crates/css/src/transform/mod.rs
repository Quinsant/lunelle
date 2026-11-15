use cssparser::{Parser, Token};

mod parser;

pub use parser::parse_transform;

/// Parsed CSS transform operation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TransformFunction {
    /// Translates along the x and y axes.
    Translate(TransformLength, TransformLength),
    /// Scales along the x and y axes.
    Scale(f32, f32),
    /// Rotates by an angle in radians.
    Rotate(f32),
    /// Skews along the x and y axes in radians.
    Skew(f32, f32),
}

/// Pixel or percentage transform distance.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TransformLength {
    /// Absolute pixel distance.
    Px(f32),
    /// Distance relative to the corresponding box dimension.
    Percent(f32),
}

/// Affine 2D matrix used by layout and text rendering.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transform2D {
    /// First coefficient of the affine matrix.
    pub a: f32,
    /// Second coefficient of the affine matrix.
    pub b: f32,
    /// Third coefficient of the affine matrix.
    pub c: f32,
    /// Fourth coefficient of the affine matrix.
    pub d: f32,
    /// Horizontal translation component.
    pub e: f32,
    /// Vertical translation component.
    pub f: f32,
}

impl Default for Transform2D {
    fn default() -> Self {
        Self {
            a: 1.0,
            b: 0.0,
            c: 0.0,
            d: 1.0,
            e: 0.0,
            f: 0.0,
        }
    }
}

impl Transform2D {
    /// Composes parsed functions in CSS order for the supplied box size.
    pub fn from_functions(functions: &[TransformFunction], width: f32, height: f32) -> Self {
        functions.iter().fold(Self::default(), |matrix, function| {
            let next = match *function {
                TransformFunction::Translate(x, y) => Self {
                    e: x.resolve(width),
                    f: y.resolve(height),
                    ..Self::default()
                },
                TransformFunction::Scale(x, y) => Self {
                    a: x,
                    d: y,
                    ..Self::default()
                },
                TransformFunction::Rotate(angle) => {
                    let (sin, cos) = angle.sin_cos();
                    Self {
                        a: cos,
                        b: sin,
                        c: -sin,
                        d: cos,
                        ..Self::default()
                    }
                }
                TransformFunction::Skew(x, y) => Self {
                    a: 1.0,
                    b: y.tan(),
                    c: x.tan(),
                    d: 1.0,
                    ..Self::default()
                },
            };
            matrix.then(next)
        })
    }

    /// Multiplies this matrix by the next transform.
    pub fn then(self, next: Self) -> Self {
        Self {
            a: self.a * next.a + self.c * next.b,
            b: self.b * next.a + self.d * next.b,
            c: self.a * next.c + self.c * next.d,
            d: self.b * next.c + self.d * next.d,
            e: self.a * next.e + self.c * next.f + self.e,
            f: self.b * next.e + self.d * next.f + self.f,
        }
    }

    /// Applies the matrix to one point.
    pub fn apply(self, x: f32, y: f32) -> (f32, f32) {
        (
            self.a * x + self.c * y + self.e,
            self.b * x + self.d * y + self.f,
        )
    }

    /// Wraps the matrix with translations so it rotates/scales around an origin.
    pub fn around_origin(self, origin_x: f32, origin_y: f32) -> Self {
        Self::default()
            .then(translation(origin_x, origin_y))
            .then(self)
            .then(translation(-origin_x, -origin_y))
    }
}

impl TransformLength {
    fn resolve(self, extent: f32) -> f32 {
        match self {
            Self::Px(value) => value,
            Self::Percent(value) => extent * value / 100.0,
        }
    }
}

/// Creates a translation-only affine matrix.
pub fn translation(x: f32, y: f32) -> Transform2D {
    Transform2D {
        e: x,
        f: y,
        ..Transform2D::default()
    }
}

/// Parses a transform origin and resolves it to pixel coordinates.
pub fn parse_transform_origin(source: &str, width: f32, height: f32) -> Option<(f32, f32)> {
    let mut parser = Parser::new(source.trim());
    let first = parse_origin_component(&mut parser, width)?;
    parser.skip_whitespace();
    let second = if parser.is_exhausted() {
        OriginComponent::Percent(50.0)
    } else {
        parse_origin_component(&mut parser, height)?
    };
    parser.expect_exhausted().ok()?;
    Some((first.resolve(width), second.resolve(height)))
}

/// Returns the translation component usable when positioning text glyphs.
pub fn text_translation(style: &crate::ComputedStyle, width: f32, height: f32) -> (f32, f32) {
    let Some(value) = style.properties.get("transform") else {
        return (0.0, 0.0);
    };
    let Some(functions) = parse_transform(value) else {
        return (0.0, 0.0);
    };
    if !functions
        .iter()
        .all(|function| matches!(function, TransformFunction::Translate(_, _)))
    {
        return (0.0, 0.0);
    }
    let origin = style
        .properties
        .get("transform-origin")
        .and_then(|value| parse_transform_origin(value, width, height))
        .unwrap_or((width * 0.5, height * 0.5));
    Transform2D::from_functions(&functions, width, height)
        .around_origin(origin.0, origin.1)
        .apply(0.0, 0.0)
}

#[derive(Clone, Copy)]
enum OriginComponent {
    Length(f32),
    Percent(f32),
}

impl OriginComponent {
    fn resolve(self, extent: f32) -> f32 {
        match self {
            Self::Length(value) => value,
            Self::Percent(value) => extent * value / 100.0,
        }
    }
}

fn parse_origin_component(parser: &mut Parser<'_>, extent: f32) -> Option<OriginComponent> {
    parser.skip_whitespace();
    if let Ok(ident) = parser.try_parse(|parser| parser.expect_ident_cloned()) {
        let value = match ident.to_ascii_lowercase().as_str() {
            "left" | "top" => 0.0,
            "center" => 50.0,
            "right" | "bottom" => 100.0,
            _ => return None,
        };
        return Some(OriginComponent::Percent(value));
    }
    match parser.next().ok()? {
        Token::Percentage { unit_value, .. } => Some(OriginComponent::Percent(unit_value * 100.0)),
        Token::Dimension { value, unit, .. } if unit.eq_ignore_ascii_case("px") => {
            Some(OriginComponent::Length(*value))
        }
        Token::Number { value, .. } if *value == 0.0 => Some(OriginComponent::Length(0.0)),
        _ => {
            let _ = extent;
            None
        }
    }
}
