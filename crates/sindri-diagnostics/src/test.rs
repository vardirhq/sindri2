use crate::{Diagnostic, DiagnosticCode, DiagnosticRelation, DiagnosticSource, Severity, SourceLocation};

/// Parses one nextest failure block into a structured diagnostic.
///
/// "Not run" suite counts are deliberately not converted to failures.
#[must_use]
pub fn parse_nextest_failure(input: &str) -> Option<Diagnostic> {
    let test_name = input.lines().find(|line| line.contains("FAIL") && line.contains("::"))
        .and_then(test_name_from_fail_line)?;
    let panic_line = input.lines().find(|line| line.contains("panicked at "));
    let location = panic_line.and_then(location_from_panic);
    let message = panic_line.and_then(panic_message)
        .unwrap_or_else(|| format!("test failed: {test_name}"));

    Some(Diagnostic {
        severity: Severity::Error,
        source: DiagnosticSource::Test,
        code: Some(DiagnosticCode("TEST_FAILURE".into())),
        message,
        location,
        notes: vec![format!("failed test: {test_name}")],
        suggestion: None,
        rendered: None,
        relation: Some(DiagnosticRelation::Primary),
    })
}

fn test_name_from_fail_line(line: &str) -> Option<String> {
    let cleaned = strip_ansi(line);
    let marker = cleaned.find("FAIL")?;
    let after = cleaned[marker + 4..].trim();
    let after_timing = after.strip_prefix('[').and_then(|value| value.split_once(']'))
        .map_or(after, |(_, rest)| rest.trim());
    (!after_timing.is_empty()).then(|| after_timing.to_owned())
}

fn location_from_panic(line: &str) -> Option<SourceLocation> {
    let cleaned = strip_ansi(line);
    let (_, tail) = cleaned.split_once("panicked at ")?;
    let (path, line, column) = parse_location(tail)?;
    Some(SourceLocation { path: path.into(), line, column, end_line: None, end_column: None })
}

fn parse_location(value: &str) -> Option<(&str, u32, u32)> {
    let location = value.split_once(": ")?.0.trim_matches(|c| c == '\'' || c == '"');
    let mut parts = location.rsplitn(3, ':');
    let column = parts.next()?.parse().ok()?;
    let line = parts.next()?.parse().ok()?;
    Some((parts.next()?, line, column))
}

fn panic_message(line: &str) -> Option<String> {
    let cleaned = strip_ansi(line);
    let (_, tail) = cleaned.split_once("panicked at ")?;
    let (_, message) = tail.split_once(": ")?;
    Some(message.to_owned())
}

fn strip_ansi(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    let mut chars = value.chars();
    while let Some(character) = chars.next() {
        if character == '\u{1b}' {
            for escape in chars.by_ref() {
                if escape == 'm' { break; }
            }
        } else {
            output.push(character);
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_failed_test_and_panic_location() {
        let input = "FAIL [ 0.007s] sindri-diagnostics cargo::tests::extracts_actionable_rust_diagnostic\nthread 'cargo::tests::extracts_actionable_rust_diagnostic' panicked at crates/sindri-diagnostics/src/cargo.rs:142:55: called Result::unwrap() on an Err value: trailing characters";
        let diagnostic = parse_nextest_failure(input).unwrap();
        assert_eq!(diagnostic.source, DiagnosticSource::Test);
        assert_eq!(diagnostic.relation, Some(DiagnosticRelation::Primary));
        assert_eq!(diagnostic.location.as_ref().unwrap().line, 142);
        assert!(diagnostic.message.contains("trailing characters"));
    }
}
