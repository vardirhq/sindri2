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

pub use cascade::{Computed, INHERITED, Matched, cascade, matched};
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
    /// Where the rule was written, for tools that show it back.
    pub origin: Origin,
}

/// Where a rule came from: its file, its line, and its selector as written.
///
/// What a devtools panel shows beside a matched rule, so the rule can be
/// found and edited. The file is the asset ID composition read it from, and
/// empty for a stylesheet parsed on its own.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Origin {
    pub file: String,
    pub line: usize,
    pub selector: String,
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
    let stripped = strip_comments(source);
    let mut source = Source {
        root: &stripped,
        markers: vec![(0, String::new(), 1)],
    };
    parse_rules(&mut source, &stripped, &[])
}

/// The whole text being parsed, and where each file composed into it
/// begins: a byte offset, the file, and the line that offset is on.
struct Source<'a> {
    root: &'a str,
    markers: Vec<(usize, String, usize)>,
}

impl Source<'_> {
    /// The file and line of `text`, a slice of the root.
    fn locate(&self, text: &str) -> (String, usize) {
        let offset = (text.as_ptr() as usize).saturating_sub(self.root.as_ptr() as usize);
        let (start, file, line) = self
            .markers
            .iter()
            .rev()
            .find(|(start, _, _)| *start <= offset)
            .cloned()
            .unwrap_or_default();
        let between = self.root.get(start..offset).unwrap_or_default();
        (
            file,
            line + between.bytes().filter(|byte| *byte == b'\n').count(),
        )
    }
}

/// The directive composition writes before each file's own rules, so a rule
/// can say where it was written: `@origin "ui/menu.weave" 12;`. Internal to
/// Weave; a stylesheet has no reason to write it.
pub(crate) const ORIGIN: &str = "@origin";

fn strip_comments(source: &str) -> String {
    let mut out = String::with_capacity(source.len());
    let mut chars = source.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '/' && chars.peek() == Some(&'*') {
            chars.next();
            let mut comment = String::new();
            while let Some(inner) = chars.next() {
                if inner == '*' && chars.peek() == Some(&'/') {
                    chars.next();
                    break;
                }
                comment.push(inner);
            }
            out.push_str(&keep_lines(&comment));
        } else {
            out.push(ch);
        }
    }
    out
}

/// A comment's newlines, which stripping keeps so every line number after it
/// still counts from the file's first line.
fn keep_lines(text: &str) -> String {
    text.chars().filter(|c| *c == '\n').collect()
}

fn parse_rules<'a>(
    source: &mut Source<'a>,
    text: &'a str,
    inherited: &[MediaQuery],
) -> Result<Stylesheet, ParseError> {
    let mut stylesheet = Stylesheet::default();
    let mut rest = text;
    while !rest.trim_start().is_empty() {
        rest = rest.trim_start();
        if let Some(after) = rest.strip_prefix(ORIGIN) {
            let end = after
                .find(';')
                .ok_or_else(|| ParseError::MissingBlock(ORIGIN.into()))?;
            let (file, line) = after[..end]
                .trim()
                .rsplit_once(' ')
                .ok_or_else(|| ParseError::MissingBlock(ORIGIN.into()))?;
            let line = line
                .parse()
                .map_err(|_| ParseError::MissingBlock(ORIGIN.into()))?;
            let offset = (after.as_ptr() as usize - source.root.as_ptr() as usize) + end + 1;
            source
                .markers
                .push((offset, file.trim().trim_matches('"').to_owned(), line));
            rest = &after[end + 1..];
            continue;
        }
        if let Some(after_media) = rest.strip_prefix("@media") {
            let open = after_media
                .find('{')
                .ok_or_else(|| ParseError::MissingBlock("@media".into()))?;
            let query = after_media[..open].trim();
            let (body, tail) = take_block(&after_media[open + 1..])?;
            let mut conditions = inherited.to_vec();
            conditions.push(media::parse_query(query)?);
            let nested = parse_rules(source, body, &conditions)?;
            stylesheet.rules.extend(nested.rules);
            rest = tail;
            continue;
        }
        let open = rest
            .find('{')
            .ok_or_else(|| ParseError::MissingBlock(rest.trim().into()))?;
        let selector_text = rest[..open].trim();
        let (file, line) = source.locate(rest);
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
        for (written, selector) in parse_selector::parse_list_written(selector_text)? {
            stylesheet.rules.push(Rule {
                selector,
                conditions: inherited.to_vec(),
                declarations: declarations.clone(),
                origin: Origin {
                    file: file.clone(),
                    line,
                    selector: written.to_owned(),
                },
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
