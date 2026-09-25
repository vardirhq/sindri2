//! Reading a selector list from its text.

use crate::ParseError;
use crate::selector::{Combinator, Compound, ListKind, Selector, States, Structural};

/// Parses a selector list, `a, b > c`, into its selectors.
pub(crate) fn parse_list(text: &str) -> Result<Vec<Selector>, ParseError> {
    top_level(text, ',')
        .into_iter()
        .map(|part| parse_selector(part.trim()))
        .collect()
}

/// `text` split at `separator` where it is not inside parentheses, so the
/// commas of `:not(a, b)` stay with it.
fn top_level(text: &str, separator: char) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0_usize;
    let mut start = 0;
    for (at, c) in text.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            _ if c == separator && depth == 0 => {
                parts.push(&text[start..at]);
                start = at + c.len_utf8();
            }
            _ => {}
        }
    }
    parts.push(&text[start..]);
    parts
}

fn parse_selector(text: &str) -> Result<Selector, ParseError> {
    let invalid = || ParseError::InvalidSelector(text.to_owned());
    let chars: Vec<char> = text.chars().collect();
    let mut position = 0;
    let mut compounds = Vec::new();
    let mut combinators = Vec::new();
    loop {
        let compound = parse_compound(&chars, &mut position, text)?;
        if compound.is_empty() && !starts_universal(&chars, position) {
            return Err(invalid());
        }
        compounds.push(compound);
        let mut saw_space = false;
        while chars.get(position).is_some_and(|c| c.is_whitespace()) {
            saw_space = true;
            position += 1;
        }
        match chars.get(position) {
            None => break,
            Some(joint @ ('>' | '+' | '~')) => {
                let combinator = match joint {
                    '>' => Combinator::Child,
                    '+' => Combinator::NextSibling,
                    _ => Combinator::LaterSibling,
                };
                position += 1;
                while chars.get(position).is_some_and(|c| c.is_whitespace()) {
                    position += 1;
                }
                combinators.push(combinator);
            }
            Some(_) if saw_space => combinators.push(Combinator::Descendant),
            Some(_) => return Err(invalid()),
        }
    }
    Ok(Selector {
        compounds,
        combinators,
    })
}

/// Whether the compound just parsed was written as `*`, which is empty but
/// still a selector.
fn starts_universal(chars: &[char], position: usize) -> bool {
    chars[..position]
        .iter()
        .rev()
        .find(|c| !c.is_whitespace())
        .is_some_and(|c| *c == '*')
}

fn parse_compound(
    chars: &[char],
    position: &mut usize,
    text: &str,
) -> Result<Compound, ParseError> {
    let mut compound = Compound::default();
    if chars.get(*position) == Some(&'*') {
        *position += 1;
    } else if chars.get(*position).is_some_and(|c| is_name_start(*c)) {
        let mut name = take_name(chars, position);
        // `sindri.ui.text`, the long form of an element name written before
        // Weave had short ones, is read as one name rather than as the
        // element `sindri` with classes `ui` and `text`.
        if name == "sindri" {
            while chars.get(*position) == Some(&'.')
                && chars.get(*position + 1).is_some_and(|c| is_name_start(*c))
            {
                *position += 1;
                name.push('.');
                name.push_str(&take_name(chars, position));
            }
        }
        compound.element = Some(name);
    }
    loop {
        match chars.get(*position) {
            Some('#') => {
                *position += 1;
                let id = take_name(chars, position);
                if id.is_empty() {
                    return Err(ParseError::InvalidSelector(text.to_owned()));
                }
                compound.id = Some(id);
            }
            Some('.') => {
                *position += 1;
                let class = take_name(chars, position);
                if class.is_empty() {
                    return Err(ParseError::InvalidSelector(text.to_owned()));
                }
                compound.classes.push(class);
            }
            Some(':') => {
                *position += 1;
                let name = take_name(chars, position);
                let argument = take_argument(chars, position, text)?;
                pseudo_class(&mut compound, &name, argument.as_deref())?;
            }
            _ => return Ok(compound),
        }
    }
}

/// What a pseudo-class adds to its compound.
fn pseudo_class(
    compound: &mut Compound,
    name: &str,
    argument: Option<&str>,
) -> Result<(), ParseError> {
    let state = match (name, argument) {
        ("hover", None) => Some(States::HOVER),
        ("active" | "pressed", None) => Some(States::ACTIVE),
        ("focus", None) => Some(States::FOCUS),
        ("disabled", None) => Some(States::DISABLED),
        ("checked", None) => Some(States::CHECKED),
        _ => None,
    };
    if let Some(state) = state {
        compound.states = compound.states.with(state);
        return Ok(());
    }
    let kind = match name {
        "not" => Some(ListKind::Not),
        "is" => Some(ListKind::Is),
        "where" => Some(ListKind::Where),
        _ => None,
    };
    if let (Some(kind), Some(argument)) = (kind, argument) {
        compound.lists.push((kind, parse_list(argument)?));
        return Ok(());
    }
    let structural = Structural::parse(name, argument)
        .ok_or_else(|| ParseError::UnsupportedPseudoClass(name.to_owned()))?;
    compound.structural.push(structural);
    Ok(())
}

/// A pseudo-class's parenthesised argument, if it has one, without the
/// parentheses. Nested parentheses stay inside it.
fn take_argument(
    chars: &[char],
    position: &mut usize,
    text: &str,
) -> Result<Option<String>, ParseError> {
    if chars.get(*position) != Some(&'(') {
        return Ok(None);
    }
    let mut depth = 0_usize;
    let start = *position + 1;
    while let Some(c) = chars.get(*position) {
        match c {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    let argument = chars[start..*position].iter().collect();
                    *position += 1;
                    return Ok(Some(argument));
                }
            }
            _ => {}
        }
        *position += 1;
    }
    Err(ParseError::InvalidSelector(text.to_owned()))
}

const fn is_name_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_' || c == '-' || !c.is_ascii()
}

fn take_name(chars: &[char], position: &mut usize) -> String {
    let start = *position;
    while chars
        .get(*position)
        .is_some_and(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-' || !c.is_ascii())
    {
        *position += 1;
    }
    chars[start..*position].iter().collect()
}
