use std::collections::{BTreeMap, BTreeSet};

use thiserror::Error;

use crate::{ParseError, Stylesheet, parse};

#[derive(Debug, Error, PartialEq)]
pub enum ComposeError {
    #[error("missing Weave source `{0}`")]
    MissingSource(String),
    #[error("invalid @use directive `{0}`")]
    InvalidUse(String),
    #[error("Weave import `{reference}` from `{from}` escapes the stylesheet root")]
    EscapesRoot { from: String, reference: String },
    #[error("circular Weave imports: {0}")]
    Cycle(String),
    #[error("{path}:{line}:{column}: {error}")]
    Parse {
        path: String,
        line: usize,
        column: usize,
        #[source]
        error: ParseError,
    },
}

/// Returns the top-level `@use` references declared before the first rule.
///
/// Weave deliberately keeps composition simple: imports are file-level and
/// must appear before selectors or media queries. This makes resolution
/// deterministic and keeps imported rules equivalent to having been written
/// before the importing file's own rules.
pub fn imports(source: &str) -> Result<Vec<String>, ComposeError> {
    split_imports(source).map(|(imports, _)| imports)
}

/// Resolve a project-relative import path from one stylesheet asset id.
///
/// Asset ids always use forward slashes, regardless of host platform.
pub fn resolve_import(source: &str, reference: &str) -> Result<String, ComposeError> {
    let mut parts = source
        .rsplit_once('/')
        .map_or_else(Vec::new, |(parent, _)| {
            parent
                .split('/')
                .filter(|part| !part.is_empty())
                .map(str::to_owned)
                .collect::<Vec<_>>()
        });

    let reference = reference.trim();
    if reference.is_empty() || reference.starts_with('/') {
        return Err(ComposeError::EscapesRoot {
            from: source.to_owned(),
            reference: reference.to_owned(),
        });
    }

    for part in reference.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                if parts.pop().is_none() {
                    return Err(ComposeError::EscapesRoot {
                        from: source.to_owned(),
                        reference: reference.to_owned(),
                    });
                }
            }
            other => parts.push(other.to_owned()),
        }
    }

    if parts.is_empty() {
        return Err(ComposeError::EscapesRoot {
            from: source.to_owned(),
            reference: reference.to_owned(),
        });
    }
    Ok(parts.join("/"))
}

/// Compose one entry stylesheet from a complete map of available Weave sources.
///
/// Imported rules are inserted before the importing file's own rules. Normal
/// Weave specificity and source order therefore continue to decide winners.
pub fn compose(
    entry: &str,
    sources: &BTreeMap<String, String>,
) -> Result<Stylesheet, ComposeError> {
    let mut stack = Vec::new();
    let mut emitted = BTreeSet::new();
    let expanded = expand(entry, sources, &mut stack, &mut emitted)?;
    parse(&expanded).map_err(|error| {
        let (line, column) = locate_parse_error(&expanded, 0, &error);
        ComposeError::Parse {
            path: entry.to_owned(),
            line,
            column,
            error,
        }
    })
}

/// Compose every independent stylesheet root in a source map.
///
/// A source referenced by `@use` is not emitted as a second top-level
/// stylesheet. Independent files remain independent roots, preserving the
/// pre-composition behavior where a project could ship more than one `.weave`
/// asset.
pub fn compose_all(
    sources: &BTreeMap<String, String>,
) -> Result<Vec<(String, Stylesheet)>, ComposeError> {
    let mut imported = BTreeSet::new();
    for (source_id, source) in sources {
        for reference in imports(source)? {
            imported.insert(resolve_import(source_id, &reference)?);
        }
    }

    let mut roots = sources
        .keys()
        .filter(|id| !imported.contains(*id))
        .cloned()
        .collect::<Vec<_>>();

    if roots.is_empty() && !sources.is_empty() {
        // A graph with no roots is necessarily cyclic (or references only
        // within a cycle). Composing one source produces the useful cycle
        // diagnostic instead of silently returning no stylesheets.
        roots.push(
            sources
                .keys()
                .next()
                .expect("source map is non-empty")
                .clone(),
        );
    }

    roots
        .into_iter()
        .map(|entry| compose(&entry, sources).map(|sheet| (entry, sheet)))
        .collect()
}

fn expand(
    id: &str,
    sources: &BTreeMap<String, String>,
    stack: &mut Vec<String>,
    emitted: &mut BTreeSet<String>,
) -> Result<String, ComposeError> {
    if emitted.contains(id) {
        return Ok(String::new());
    }
    if let Some(index) = stack.iter().position(|entry| entry == id) {
        let mut cycle = stack[index..].to_vec();
        cycle.push(id.to_owned());
        return Err(ComposeError::Cycle(cycle.join(" -> ")));
    }

    let source = sources
        .get(id)
        .ok_or_else(|| ComposeError::MissingSource(id.to_owned()))?;
    let (references, body) = split_imports(source)?;
    validate_source(id, source, body)?;
    stack.push(id.to_owned());

    let mut expanded = String::new();
    for reference in references {
        let imported_id = resolve_import(id, &reference)?;
        expanded.push_str(&expand(&imported_id, sources, stack, emitted)?);
        if !expanded.ends_with('\n') {
            expanded.push('\n');
        }
    }
    expanded.push_str(body);

    stack.pop();
    emitted.insert(id.to_owned());
    Ok(expanded)
}

fn validate_source(id: &str, source: &str, body: &str) -> Result<(), ComposeError> {
    let body_offset = source.len().saturating_sub(body.len());
    parse(body).map_err(|error| {
        let (line, column) = locate_parse_error(source, body_offset, &error);
        ComposeError::Parse {
            path: id.to_owned(),
            line,
            column,
            error,
        }
    })?;
    Ok(())
}

fn locate_parse_error(source: &str, start: usize, error: &ParseError) -> (usize, usize) {
    let needle = match error {
        ParseError::MissingBlock(value)
        | ParseError::MissingColon(value)
        | ParseError::UnsupportedMedia(value)
        | ParseError::InvalidMediaWidth(value) => Some(value.as_str()),
        ParseError::MissingClose => None,
    };
    let offset = needle
        .and_then(|needle| source.get(start..)?.find(needle).map(|found| start + found))
        .unwrap_or_else(|| source.len().saturating_sub(1));
    line_column(source, offset)
}

fn line_column(source: &str, offset: usize) -> (usize, usize) {
    let prefix = &source[..offset.min(source.len())];
    let line = prefix.bytes().filter(|byte| *byte == b'\n').count() + 1;
    let column = prefix.rsplit('\n').next().map_or(1, |tail| tail.chars().count() + 1);
    (line, column)
}

fn split_imports(source: &str) -> Result<(Vec<String>, &str), ComposeError> {
    let mut imports = Vec::new();
    let mut rest = source;

    loop {
        let trimmed = rest.trim_start();
        if !trimmed.starts_with("@use") {
            return Ok((imports, trimmed));
        }

        let after_keyword = trimmed[4..].trim_start();
        let Some(quote) = after_keyword.chars().next() else {
            return Err(ComposeError::InvalidUse(trimmed.to_owned()));
        };
        if quote != '"' && quote != '\'' {
            return Err(ComposeError::InvalidUse(trimmed.to_owned()));
        }

        let after_quote = &after_keyword[quote.len_utf8()..];
        let Some(close) = after_quote.find(quote) else {
            return Err(ComposeError::InvalidUse(trimmed.to_owned()));
        };
        let reference = &after_quote[..close];
        let after_reference = after_quote[close + quote.len_utf8()..].trim_start();
        let Some(after_semicolon) = after_reference.strip_prefix(';') else {
            return Err(ComposeError::InvalidUse(trimmed.to_owned()));
        };

        if reference.trim().is_empty() {
            return Err(ComposeError::InvalidUse(trimmed.to_owned()));
        }
        imports.push(reference.trim().to_owned());
        rest = after_semicolon;
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::{ComposeError, compose, compose_all, imports, resolve_import};
    use crate::{Selector, Viewport};

    #[test]
    fn imports_are_composed_before_local_rules() {
        let sources = BTreeMap::from([
            (
                "ui.weave".into(),
                "@use \"shared/theme.weave\"; .button { width: 300px; }".into(),
            ),
            (
                "shared/theme.weave".into(),
                ".button { width: 200px; color: white; }".into(),
            ),
        ]);

        let sheet = compose("ui.weave", &sources).expect("styles compose");
        assert_eq!(sheet.rules.len(), 2);
        assert_eq!(sheet.rules[0].selector, Selector::Class("button".into()));
        assert_eq!(sheet.rules[0].declarations["width"], "200px");
        assert_eq!(sheet.rules[1].declarations["width"], "300px");
        assert!(sheet.rules[1].applies_with_classes(
            "play",
            &["button"],
            &["sindri.ui.button"],
            Viewport {
                width: 1280.0,
                height: 720.0,
            }
        ));
    }

    #[test]
    fn nested_relative_imports_resolve_from_the_importing_file() {
        let sources = BTreeMap::from([
            ("ui.weave".into(), "@use \"panels/hud.weave\";".into()),
            (
                "panels/hud.weave".into(),
                "@use \"../shared/theme.weave\"; .hud { color: white; }".into(),
            ),
            (
                "shared/theme.weave".into(),
                ".root { background: black; }".into(),
            ),
        ]);

        let sheet = compose("ui.weave", &sources).expect("nested styles compose");
        assert_eq!(sheet.rules.len(), 2);
        assert_eq!(
            resolve_import("panels/hud.weave", "../shared/theme.weave").unwrap(),
            "shared/theme.weave"
        );
    }

    #[test]
    fn imported_parse_errors_keep_their_file_and_location() {
        let sources = BTreeMap::from([
            ("ui.weave".into(), "@use \"shared/theme.weave\";".into()),
            (
                "shared/theme.weave".into(),
                "\n\n.button { width 200px; }".into(),
            ),
        ]);

        let error = compose("ui.weave", &sources).expect_err("invalid import must fail");
        assert_eq!(
            error.to_string(),
            "shared/theme.weave:3:11: expected ':' in declaration `width 200px`"
        );
    }

    #[test]
    fn cycles_are_reported_with_the_import_chain() {
        let sources = BTreeMap::from([
            ("a.weave".into(), "@use \"b.weave\";".into()),
            ("b.weave".into(), "@use \"a.weave\";".into()),
        ]);

        assert_eq!(
            compose("a.weave", &sources),
            Err(ComposeError::Cycle("a.weave -> b.weave -> a.weave".into()))
        );
    }

    #[test]
    fn compose_all_does_not_emit_imported_files_twice() {
        let sources = BTreeMap::from([
            (
                "ui.weave".into(),
                "@use \"theme.weave\"; .button { width: 300px; }".into(),
            ),
            ("theme.weave".into(), ".button { color: white; }".into()),
            ("debug.weave".into(), ".debug { color: white; }".into()),
        ]);

        let roots = compose_all(&sources).expect("roots compose");
        assert_eq!(
            roots.iter().map(|(id, _)| id.as_str()).collect::<Vec<_>>(),
            vec!["debug.weave", "ui.weave"]
        );
    }

    #[test]
    fn use_directives_must_be_complete_and_top_level() {
        assert_eq!(
            imports("@use theme.weave;"),
            Err(ComposeError::InvalidUse("@use theme.weave;".into()))
        );
        assert_eq!(
            imports("@use \"theme.weave\"; .x { width: 1px; }"),
            Ok(vec!["theme.weave".into()])
        );
    }
}
