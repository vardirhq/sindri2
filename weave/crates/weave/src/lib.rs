use std::collections::BTreeMap;
use thiserror::Error;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Stylesheet { pub rules: Vec<Rule> }

#[derive(Clone, Debug, PartialEq)]
pub struct Rule {
    pub selector: Selector,
    pub condition: Option<MediaCondition>,
    pub declarations: BTreeMap<String, String>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Selector { Id(String), Type(String) }

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MediaCondition { MaxWidth(f32), MinWidth(f32), Portrait, Landscape }

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Viewport { pub width: f32, pub height: f32 }

impl MediaCondition {
    #[must_use]
    pub fn matches(self, viewport: Viewport) -> bool {
        match self {
            Self::MaxWidth(width) => viewport.width <= width,
            Self::MinWidth(width) => viewport.width >= width,
            Self::Portrait => viewport.height >= viewport.width,
            Self::Landscape => viewport.width > viewport.height,
        }
    }
}

impl Rule {
    #[must_use]
    pub fn applies(&self, id: &str, component_types: &[&str], viewport: Viewport) -> bool {
        let selector_matches = match &self.selector {
            Selector::Id(expected) => expected == id,
            Selector::Type(expected) => component_types.iter().any(|kind| *kind == expected),
        };
        selector_matches && self.condition.is_none_or(|condition| condition.matches(viewport))
    }
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
}

pub fn parse(source: &str) -> Result<Stylesheet, ParseError> {
    parse_rules(strip_comments(source), None)
}

fn strip_comments(source: &str) -> String {
    let mut out = String::with_capacity(source.len());
    let mut chars = source.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '/' && chars.peek() == Some(&'*') {
            chars.next();
            while let Some(inner) = chars.next() {
                if inner == '*' && chars.peek() == Some(&'/') { chars.next(); break; }
            }
        } else { out.push(ch); }
    }
    out
}

fn parse_rules(source: String, inherited: Option<MediaCondition>) -> Result<Stylesheet, ParseError> {
    let mut stylesheet = Stylesheet::default();
    let mut rest = source.as_str();
    while !rest.trim_start().is_empty() {
        rest = rest.trim_start();
        if let Some(after_media) = rest.strip_prefix("@media") {
            let open = after_media.find('{').ok_or_else(|| ParseError::MissingBlock("@media".into()))?;
            let query = after_media[..open].trim();
            let (body, tail) = take_block(&after_media[open + 1..])?;
            let nested = parse_rules(body.to_owned(), Some(parse_media(query)?))?;
            stylesheet.rules.extend(nested.rules);
            rest = tail;
            continue;
        }
        let open = rest.find('{').ok_or_else(|| ParseError::MissingBlock(rest.trim().into()))?;
        let selector_text = rest[..open].trim();
        let (body, tail) = take_block(&rest[open + 1..])?;
        let selector = selector_text.strip_prefix('#').map_or_else(
            || Selector::Type(selector_text.to_owned()),
            |id| Selector::Id(id.trim().to_owned()),
        );
        let mut declarations = BTreeMap::new();
        for raw in body.split(';').map(str::trim).filter(|line| !line.is_empty()) {
            let Some((name, value)) = raw.split_once(':') else { return Err(ParseError::MissingColon(raw.to_owned())); };
            declarations.insert(name.trim().to_owned(), value.trim().to_owned());
        }
        stylesheet.rules.push(Rule { selector, condition: inherited, declarations });
        rest = tail;
    }
    Ok(stylesheet)
}

fn take_block(source: &str) -> Result<(&str, &str), ParseError> {
    let mut depth = 1usize;
    for (index, ch) in source.char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => { depth -= 1; if depth == 0 { return Ok((&source[..index], &source[index + 1..])); } }
            _ => {}
        }
    }
    Err(ParseError::MissingClose)
}

fn parse_media(query: &str) -> Result<MediaCondition, ParseError> {
    let query = query.trim();
    if query == "(orientation: portrait)" { return Ok(MediaCondition::Portrait); }
    if query == "(orientation: landscape)" { return Ok(MediaCondition::Landscape); }
    for (prefix, make) in [
        ("(max-width:", MediaCondition::MaxWidth as fn(f32) -> MediaCondition),
        ("(min-width:", MediaCondition::MinWidth as fn(f32) -> MediaCondition),
    ] {
        if let Some(value) = query.strip_prefix(prefix).and_then(|rest| rest.strip_suffix(')')) {
            let value = value.trim().strip_suffix("px").unwrap_or(value.trim()).trim();
            let width: f32 = value.parse().map_err(|_| ParseError::InvalidMediaWidth(value.to_owned()))?;
            if !width.is_finite() { return Err(ParseError::InvalidMediaWidth(value.to_owned())); }
            return Ok(make(width));
        }
    }
    Err(ParseError::UnsupportedMedia(query.to_owned()))
}

#[cfg(test)]
mod tests {
    use super::{MediaCondition, Selector, Viewport, parse};

    #[test]
    fn parses_responsive_rules() {
        let sheet = parse("#menu { width: 420px; } @media (max-width: 700px) { #menu { width: 90vw; } }").expect("valid Weave");
        assert_eq!(sheet.rules.len(), 2);
        assert_eq!(sheet.rules[0].selector, Selector::Id("menu".into()));
        assert_eq!(sheet.rules[1].condition, Some(MediaCondition::MaxWidth(700.0)));
        assert!(sheet.rules[1].applies("menu", &[], Viewport { width: 390.0, height: 844.0 }));
    }
}
