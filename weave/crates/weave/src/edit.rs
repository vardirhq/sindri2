//! Changing one declaration in a stylesheet's source, as devtools do.
//!
//! The rule is found where its [`Origin`](crate::Origin) says it was written,
//! by line and selector, and only the value is replaced: every other byte of
//! the file, comments, spacing and all, stays as the author left it.

/// Why a declaration could not be written back.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum EditError {
    #[error("no rule `{0}` starts on that line; the file changed since it was read")]
    RuleNotFound(String),
    #[error(
        "`{0}` is not written in that rule by itself; it may come from a shorthand such as `padding`"
    )]
    PropertyNotFound(String),
    #[error("a value cannot contain `;`, `{{` or `}}`")]
    UnsafeValue,
}

/// `source` with `property` in the rule `selector`, written on `line`,
/// set to `value`.
///
/// `selector` is one selector of the rule as written: a rule written as a
/// list, `a, b { … }`, is found by any of them.
pub fn set_declaration(
    source: &str,
    line: usize,
    selector: &str,
    property: &str,
    value: &str,
) -> Result<String, EditError> {
    let value = value.trim();
    if value.is_empty() || value.contains([';', '{', '}']) {
        return Err(EditError::UnsafeValue);
    }
    let rule_start =
        line_start(source, line).ok_or_else(|| EditError::RuleNotFound(selector.to_owned()))?;
    let open = find_code(source, rule_start, '{')
        .ok_or_else(|| EditError::RuleNotFound(selector.to_owned()))?;
    let written = strip(&source[rule_start..open]);
    let names_it = written
        .split(',')
        .map(str::trim)
        .any(|part| part == selector.trim());
    if !names_it {
        return Err(EditError::RuleNotFound(selector.to_owned()));
    }
    let close =
        matching_close(source, open).ok_or_else(|| EditError::RuleNotFound(selector.to_owned()))?;
    let (start, end) = value_range(source, open + 1, close, property)
        .ok_or_else(|| EditError::PropertyNotFound(property.to_owned()))?;
    Ok(format!("{}{value}{}", &source[..start], &source[end..]))
}

/// The byte offset where `line` (counted from one) begins.
fn line_start(source: &str, line: usize) -> Option<usize> {
    if line == 1 {
        return Some(0);
    }
    source
        .match_indices('\n')
        .nth(line.checked_sub(2)?)
        .map(|(at, _)| at + 1)
}

/// The first `wanted` at or after `from` that is not inside a comment.
fn find_code(source: &str, from: usize, wanted: char) -> Option<usize> {
    let bytes = source.as_bytes();
    let mut at = from;
    while at < bytes.len() {
        if bytes[at..].starts_with(b"/*") {
            at += source[at..].find("*/").map_or(bytes.len(), |end| end + 2);
            continue;
        }
        if bytes[at] == wanted as u8 {
            return Some(at);
        }
        at += 1;
    }
    None
}

/// The `}` that closes the block opened at `open`.
fn matching_close(source: &str, open: usize) -> Option<usize> {
    let bytes = source.as_bytes();
    let mut depth = 0_usize;
    let mut at = open;
    while at < bytes.len() {
        if bytes[at..].starts_with(b"/*") {
            at += source[at..].find("*/").map_or(bytes.len(), |end| end + 2);
            continue;
        }
        match bytes[at] {
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(at);
                }
            }
            _ => {}
        }
        at += 1;
    }
    None
}

/// Where the value of `property` is in the block between `start` and `end`,
/// trimmed of the spaces around it, if the rule declares it itself. The last
/// declaration of it wins, as in CSS, so that is the one changed.
fn value_range(source: &str, start: usize, end: usize, property: &str) -> Option<(usize, usize)> {
    let mut found = None;
    let mut declaration = start;
    loop {
        let stop = find_code(source, declaration, ';')
            .filter(|at| *at < end)
            .unwrap_or(end);
        let colon = find_code(source, declaration, ':').filter(|at| *at < stop);
        if let Some(colon) = colon
            && strip(&source[declaration..colon]).trim() == property
        {
            let raw = &source[colon + 1..stop];
            let leading = raw.len() - raw.trim_start().len();
            let trailing = raw.len() - raw.trim_end().len();
            found = Some((colon + 1 + leading, stop - trailing));
        }
        if stop >= end {
            break;
        }
        declaration = stop + 1;
    }
    found
}

/// `text` without its comments.
fn strip(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(open) = rest.find("/*") {
        out.push_str(&rest[..open]);
        rest = rest[open..]
            .find("*/")
            .map_or("", |close| &rest[open + close + 2..]);
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::{EditError, set_declaration};

    const SHEET: &str = "/* The menu. */\n\
        .menu, .panel {\n  color: #fff; /* white; on dark */\n  width: 200px;\n}\n\
        \n.button:hover { color: red }\n";

    #[test]
    fn only_the_value_changes() {
        let edited = set_declaration(SHEET, 2, ".panel", "width", "240px").expect("edits");
        assert_eq!(edited, SHEET.replace("200px", "240px"));
        let last = set_declaration(SHEET, 7, ".button:hover", "color", "#0f0").expect("edits");
        assert_eq!(last, SHEET.replace("color: red", "color: #0f0"));
        // The comment after the declaration is kept, and so is its `;`.
        let coloured = set_declaration(SHEET, 2, ".menu", "color", "#eee").expect("edits");
        assert!(
            coloured.contains("color: #eee; /* white; on dark */"),
            "{coloured}"
        );
    }

    #[test]
    fn what_cannot_be_found_or_written_is_refused() {
        assert_eq!(
            set_declaration(SHEET, 3, ".menu", "color", "red"),
            Err(EditError::RuleNotFound(".menu".into()))
        );
        assert_eq!(
            set_declaration(SHEET, 2, ".menu", "padding-top", "4px"),
            Err(EditError::PropertyNotFound("padding-top".into()))
        );
        assert_eq!(
            set_declaration(SHEET, 2, ".menu", "color", "red; width: 0"),
            Err(EditError::UnsafeValue)
        );
    }
}
