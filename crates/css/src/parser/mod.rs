use cssparser::{
    AtRuleParser, ParseError, Parser, ParserState, QualifiedRuleParser, StyleSheetParser,
};

use crate::{CssDiagnostic, Keyframe, Keyframes, StyleRule, StyleSheet, media::MediaCondition};

mod declarations;

use declarations::parse_declarations;

/// Parses a source string into stylesheet rules while retaining diagnostics.
pub(crate) fn parse_stylesheet(source: &str) -> StyleSheet {
    let mut parser = Parser::new(source);
    let mut rule_parser = CssRuleParser {
        diagnostics: Vec::new(),
        rule: 0,
        source_order: 0,
        media_conditions: Vec::new(),
        keyframes: Vec::new(),
    };
    let rules = parse_rules(&mut parser, &mut rule_parser);
    let (unterminated_comment, unterminated_block) = source_structure_errors(source);
    if unterminated_comment {
        rule_parser.diagnostics.push(CssDiagnostic {
            rule: rule_parser.rule + 1,
            message: "unterminated CSS comment".into(),
        });
    }
    if unterminated_block {
        rule_parser.diagnostics.push(CssDiagnostic {
            rule: rule_parser.rule + 1,
            message: "unterminated rule block".into(),
        });
    }
    StyleSheet {
        rules,
        keyframes: rule_parser.keyframes,
        diagnostics: rule_parser.diagnostics,
    }
}

/// Detects unterminated comments and rule blocks for clearer diagnostics.
fn source_structure_errors(source: &str) -> (bool, bool) {
    let mut depth = 0usize;
    let mut quote = None;
    let mut escaped = false;
    let mut chars = source.chars().peekable();
    while let Some(character) = chars.next() {
        if let Some(quote_char) = quote {
            if escaped {
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else if character == quote_char {
                quote = None;
            }
            continue;
        }
        if character == '/' && chars.peek() == Some(&'*') {
            chars.next();
            let mut closed = false;
            while let Some(comment_char) = chars.next() {
                if comment_char == '*' && chars.peek() == Some(&'/') {
                    chars.next();
                    closed = true;
                    break;
                }
            }
            if !closed {
                return (true, depth != 0);
            }
            continue;
        }
        if character == '\'' || character == '"' {
            quote = Some(character);
            continue;
        }
        match character {
            '{' => depth += 1,
            '}' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    if quote.is_some() {
        // The tokenizer reports the unterminated string; do not misreport its
        // contents as an unterminated rule block as well.
        (false, false)
    } else {
        (false, depth != 0)
    }
}

/// Runs cssparser's rule iterator and converts failures to diagnostics.
fn parse_rules<'i>(input: &mut Parser<'i>, rule_parser: &mut CssRuleParser) -> Vec<StyleRule> {
    let mut rules = Vec::new();
    let mut errors = Vec::new();
    {
        let parser = StyleSheetParser::<'_, '_, CssRuleParser>::new(input, rule_parser);
        for result in parser {
            match result {
                Ok(parsed_rules) => rules.extend(parsed_rules),
                Err((error, _, _)) => errors.push(format!("invalid CSS rule: {error:?}")),
            }
        }
    }
    for message in errors {
        rule_parser.diagnostics.push(CssDiagnostic {
            rule: rule_parser.rule + 1,
            message,
        });
    }
    rules
}

/// State shared while parsing nested stylesheet rules and at-rules.
struct CssRuleParser {
    diagnostics: Vec<CssDiagnostic>,
    rule: usize,
    source_order: usize,
    media_conditions: Vec<MediaCondition>,
    keyframes: Vec<Keyframes>,
}

/// Supported at-rule preludes passed to block handling.
enum AtRulePrelude {
    Media(String),
    Keyframes(String),
}

impl<'i> AtRuleParser<'i> for CssRuleParser {
    type Prelude = AtRulePrelude;
    type AtRule = Vec<StyleRule>;
    type Error = ();

    fn parse_prelude(
        &mut self,
        name: cssparser::CowRcStr<'i>,
        input: &mut Parser<'i>,
    ) -> Result<Self::Prelude, ParseError<Self::Error>> {
        if name.eq_ignore_ascii_case("media") {
            let start = input.position();
            while input.next_including_whitespace_and_comments().is_ok() {}
            return Ok(AtRulePrelude::Media(
                input.slice(start..input.position()).trim().to_owned(),
            ));
        }
        if name.eq_ignore_ascii_case("keyframes") {
            let name = input.expect_ident()?.to_string();
            input.expect_exhausted()?;
            return Ok(AtRulePrelude::Keyframes(name));
        }
        Err(ParseError::unexpected_token())
    }

    fn parse_block(
        &mut self,
        prelude: Self::Prelude,
        _start: &ParserState,
        input: &mut Parser<'i>,
    ) -> Result<Self::AtRule, ParseError<Self::Error>> {
        match prelude {
            AtRulePrelude::Media(prelude) => {
                let conditions = crate::media::parse_media_conditions(&prelude)
                    .ok_or_else(ParseError::unexpected_token)?;
                let mut nested = CssRuleParser {
                    diagnostics: Vec::new(),
                    rule: self.rule,
                    source_order: self.source_order,
                    media_conditions: self
                        .media_conditions
                        .iter()
                        .copied()
                        .chain(conditions)
                        .collect(),
                    keyframes: Vec::new(),
                };
                let rules = parse_rules(input, &mut nested);
                self.rule = nested.rule;
                self.source_order = nested.source_order;
                self.diagnostics.extend(nested.diagnostics);
                self.keyframes.extend(nested.keyframes);
                Ok(rules)
            }
            AtRulePrelude::Keyframes(name) => {
                let mut keyframe_parser = CssKeyframeParser {
                    diagnostics: Vec::new(),
                };
                let mut frames = Vec::new();
                let mut errors = Vec::new();
                {
                    let parser = StyleSheetParser::<'_, '_, CssKeyframeParser>::new(
                        input,
                        &mut keyframe_parser,
                    );
                    for result in parser {
                        match result {
                            Ok(parsed_frames) => frames.extend(parsed_frames),
                            Err((error, _, _)) => {
                                errors.push(format!("invalid keyframe rule: {error:?}"));
                            }
                        }
                    }
                }
                self.rule += 1;
                let rule = self.rule;
                self.diagnostics.extend(
                    keyframe_parser
                        .diagnostics
                        .into_iter()
                        .map(|message| CssDiagnostic { rule, message }),
                );
                self.diagnostics.extend(
                    errors
                        .into_iter()
                        .map(|message| CssDiagnostic { rule, message }),
                );
                self.keyframes.push(Keyframes { name, frames });
                Ok(Vec::new())
            }
        }
    }
}

impl<'i> QualifiedRuleParser<'i> for CssRuleParser {
    type Prelude = String;
    type QualifiedRule = Vec<StyleRule>;
    type Error = ();

    fn parse_prelude(
        &mut self,
        input: &mut Parser<'i>,
    ) -> Result<Self::Prelude, ParseError<Self::Error>> {
        let start = input.position();
        while input.next_including_whitespace_and_comments().is_ok() {}
        Ok(input.slice(start..input.position()).trim().to_owned())
    }

    fn parse_block(
        &mut self,
        selector: String,
        _start: &ParserState,
        input: &mut Parser<'i>,
    ) -> Result<Self::QualifiedRule, ParseError<Self::Error>> {
        self.rule += 1;
        let rule = self.rule;
        let mut declaration_diagnostics = Vec::new();
        let declarations = parse_declarations(input, "declaration", &mut declaration_diagnostics)?;
        self.diagnostics.extend(
            declaration_diagnostics
                .into_iter()
                .map(|message| CssDiagnostic { rule, message }),
        );
        let source_order = self.source_order;
        self.source_order += 1;
        Ok(vec![StyleRule {
            selector,
            declarations,
            media_conditions: self.media_conditions.clone(),
            source_order,
        }])
    }
}

/// Parser state for one `@keyframes` block.
struct CssKeyframeParser {
    diagnostics: Vec<String>,
}

impl<'i> AtRuleParser<'i> for CssKeyframeParser {
    type Prelude = String;
    type AtRule = Vec<Keyframe>;
    type Error = ();

    fn parse_prelude(
        &mut self,
        _name: cssparser::CowRcStr<'i>,
        _input: &mut Parser<'i>,
    ) -> Result<Self::Prelude, ParseError<Self::Error>> {
        Err(ParseError::unexpected_token())
    }
}

impl<'i> QualifiedRuleParser<'i> for CssKeyframeParser {
    type Prelude = String;
    type QualifiedRule = Vec<Keyframe>;
    type Error = ();

    fn parse_prelude(
        &mut self,
        input: &mut Parser<'i>,
    ) -> Result<Self::Prelude, ParseError<Self::Error>> {
        let start = input.position();
        while input.next_including_whitespace_and_comments().is_ok() {}
        Ok(input.slice(start..input.position()).trim().to_owned())
    }

    fn parse_block(
        &mut self,
        selectors: String,
        _start: &ParserState,
        input: &mut Parser<'i>,
    ) -> Result<Self::QualifiedRule, ParseError<Self::Error>> {
        let mut offsets = Vec::new();
        for selector in selectors.split(',').map(str::trim) {
            let offset = if selector.eq_ignore_ascii_case("from") {
                Some(0.0)
            } else if selector.eq_ignore_ascii_case("to") {
                Some(1.0)
            } else {
                selector
                    .strip_suffix('%')
                    .and_then(|value| value.trim().parse::<f32>().ok())
                    .filter(|value| value.is_finite() && (0.0..=100.0).contains(value))
                    .map(|value| value / 100.0)
            };
            if let Some(offset) = offset {
                offsets.push(offset);
            } else {
                self.diagnostics
                    .push(format!("invalid keyframe selector `{selector}`"));
            }
        }
        let declarations =
            parse_declarations(input, "keyframe declaration", &mut self.diagnostics)?;
        Ok(offsets
            .into_iter()
            .map(|offset| Keyframe {
                offset,
                declarations: declarations.clone(),
            })
            .collect())
    }
}
