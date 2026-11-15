use cssparser::{Delimiter, ParseError, Parser, Token};

use crate::Declaration;

/// Parses declaration blocks and records invalid declarations as diagnostics.
pub(super) fn parse_declarations(
    input: &mut Parser<'_>,
    context: &str,
    diagnostics: &mut Vec<String>,
) -> Result<Vec<Declaration>, ParseError<()>> {
    let mut declarations = Vec::new();
    while !input.is_exhausted() {
        input.skip_whitespace();
        let declaration_start = input.state();
        let Ok(Token::Ident(name)) = input.next() else {
            if input.is_exhausted() {
                break;
            }
            diagnostics.push(format!("invalid {context}: expected a property name"));
            let _ = input.parse_until_after(Delimiter::Semicolon, |_| Ok::<_, ParseError<()>>(()));
            continue;
        };
        let name = name.to_ascii_lowercase();
        if input.expect_colon().is_err() {
            diagnostics.push(format!("invalid {context} `{name}`: expected `:`"));
            let _ = input.parse_until_after(Delimiter::Semicolon, |_| Ok::<_, ParseError<()>>(()));
            continue;
        }
        let value_start = input.position();
        let value = input.parse_until_before(Delimiter::Semicolon | Delimiter::Bang, |value| {
            while value.next_including_whitespace_and_comments().is_ok() {}
            Ok::<_, ParseError<()>>(value.slice(value_start..value.position()).trim().to_owned())
        })?;
        let important = input.try_parse(cssparser::parse_important).is_ok();
        declarations.push(Declaration {
            name,
            value,
            important,
        });
        let _ = input.try_parse(|input| input.expect_semicolon());
        if input.position() == declaration_start.position() {
            break;
        }
    }
    Ok(declarations)
}
