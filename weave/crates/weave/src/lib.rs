//! Weave: styling Sindri's screen UI the way a web page is styled.
//!
//! A stylesheet is rules; a rule is a selector, the viewports it applies to,
//! and declarations. The selector syntax, the cascade and inheritance are
//! CSS's, so what someone knows about styling a page carries over. What a
//! property *means* is the host's business: Weave decides which declarations
//! win for an element, and the host (`sindri-weave`) turns them into Sindri
//! UI data.

use std::collections::BTreeMap;
use thiserror::Error;

mod cascade;
mod composition;
mod media;
mod parse_selector;
mod selector;
pub mod shorthand;
mod structural;

pub use cascade::{Computed, INHERITED, cascade};
pub use composition::{ComposeError, compose, compose_all, imports, resolve_import};
pub use media::{MediaCondition, MediaQuery};
pub use selector::{
    Combinator, Compound, Element, ListKind, Position, Selector, Specificity, States, Structural,
    Tree,
};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Stylesheet {
    pub rules: Vec<Rule>,
}

/// One selector and what it declares.
///
/// A rule written with a selector list, `h1, h2 { … }`, becomes one rule per
/// selector, each with its own specificity, as CSS scores them.
#[derive(Clone, Debug, PartialEq)]
pub struct Rule {
    pub selector: Selector,
    /// The `@media` queries the rule sits inside. All must match: a query
    /// nested in another applies only where both do.
    pub conditions: Vec<MediaQuery>,
    pub declarations: BTreeMap<String, String>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Viewport {
    pub width: f32,
    pub height: f32,
}

#[derive(Debug, Error, PartialEq)]
pub enum ParseError {
    #[error("expected '{{' after selector `{0}`")]
    MissingBlock(String),
    #[error("expected ':' in declaration `{0}`")]
    MissingColon(String),
    #[error("expected closing '}}'")]
    MissingClose,
    #[error("unsupported media query `{0}`")]
    UnsupportedMedia(String),
    #[error("media width is not a finite number: `{0}`")]
    InvalidMediaWidth(String),
    #[error("`{0}` is not a selector Weave understands")]
    InvalidSelector(String),
    #[error(
        "`:{0}` is not a state Weave knows; it knows :hover, :active (or :pressed), :focus, :disabled and :checked"
    )]
    UnsupportedPseudoClass(String),
}

pub fn parse(source: &str) -> Result<Stylesheet, ParseError> {
    parse_rules(strip_comments(source), &[])
}

fn strip_comments(source: &str) -> String {
    let mut out = String::with_capacity(source.len());
    let mut chars = source.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '/' && chars.peek() == Some(&'*') {
            chars.next();
            while let Some(inner) = chars.next() {
                if inner == '*' && chars.peek() == Some(&'/') {
                    chars.next();
                    break;
                }
            }
        } else {
            out.push(ch);
        }
    }
    out
}

fn parse_rules(source: String, inherited: &[MediaQuery]) -> Result<Stylesheet, ParseError> {
    let mut stylesheet = Stylesheet::default();
    let mut rest = source.as_str();
    while !rest.trim_start().is_empty() {
        rest = rest.trim_start();
        if let Some(after_media) = rest.strip_prefix("@media") {
            let open = after_media
                .find('{')
                .ok_or_else(|| ParseError::MissingBlock("@media".into()))?;
            let query = after_media[..open].trim();
            let (body, tail) = take_block(&after_media[open + 1..])?;
            let mut conditions = inherited.to_vec();
            conditions.push(media::parse_query(query)?);
            let nested = parse_rules(body.to_owned(), &conditions)?;
            stylesheet.rules.extend(nested.rules);
            rest = tail;
            continue;
        }
        let open = rest
            .find('{')
            .ok_or_else(|| ParseError::MissingBlock(rest.trim().into()))?;
        let selector_text = rest[..open].trim();
        let (body, tail) = take_block(&rest[open + 1..])?;
        let mut declarations = BTreeMap::new();
        for raw in body
            .split(';')
            .map(str::trim)
            .filter(|line| !line.is_empty())
        {
            let Some((name, value)) = raw.split_once(':') else {
                return Err(ParseError::MissingColon(raw.to_owned()));
            };
            let (name, value) = (name.trim(), value.trim());
            if let Some(longhands) = shorthand::expand(name, value) {
                for (longhand, side) in longhands {
                    declarations.insert(longhand.to_owned(), side);
                }
            } else {
                declarations.insert(name.to_owned(), value.to_owned());
            }
        }
        for selector in selector::parse_list(selector_text)? {
            stylesheet.rules.push(Rule {
                selector,
                conditions: inherited.to_vec(),
                declarations: declarations.clone(),
            });
        }
        rest = tail;
    }
    Ok(stylesheet)
}

fn take_block(source: &str) -> Result<(&str, &str), ParseError> {
    let mut depth = 1usize;
    for (index, ch) in source.char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Ok((&source[..index], &source[index + 1..]));
                }
            }
            _ => {}
        }
    }
    Err(ParseError::MissingClose)
}

#[cfg(test)]
mod tests {
    use super::{Selector, Viewport, parse};

    #[test]
    fn a_selector_list_is_one_rule_per_selector() {
        let sheet = parse(".primary, #play { width: 280px; }").expect("valid Weave");
        assert_eq!(sheet.rules.len(), 2);
        assert_eq!(sheet.rules[0].selector, Selector::class("primary"));
        assert_eq!(sheet.rules[1].selector, Selector::id("play"));
        assert_eq!(sheet.rules[1].declarations["width"], "280px");
    }

    #[test]
    fn nested_media_applies_where_both_queries_do() {
        let sheet = parse(
            "@media (max-width: 700px) { @media (orientation: portrait) { #menu { width: 90vw; } } }",
        )
        .expect("valid Weave");
        let rule = &sheet.rules[0];
        let applies = |width, height| {
            rule.conditions
                .iter()
                .all(|query| query.matches(Viewport { width, height }))
        };
        assert!(applies(390.0, 844.0));
        assert!(!applies(690.0, 400.0), "narrow but landscape");
        assert!(!applies(1440.0, 2000.0), "portrait but wide");
    }
}
