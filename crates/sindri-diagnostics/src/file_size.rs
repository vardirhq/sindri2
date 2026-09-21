use crate::{
    Diagnostic, DiagnosticCode, DiagnosticRelation, DiagnosticSource, Severity, SourceLocation,
};
use std::path::PathBuf;

/// Parses violations emitted by `scripts/check-file-size.py`.
///
/// Expected violation lines have the form `<count>  <path>`. Summary and
/// guidance lines are ignored, so callers may pass the script's complete stderr.
#[must_use]
pub fn parse_file_size_violations(input: &str) -> Vec<Diagnostic> {
    input.lines().filter_map(parse_violation).collect()
}

fn parse_violation(line: &str) -> Option<Diagnostic> {
    let mut fields = line.split_whitespace();
    let count = fields.next()?.parse::<u32>().ok()?;
    let path = PathBuf::from(fields.next()?);
    if path.extension().and_then(|value| value.to_str()) != Some("rs") {
        return None;
    }

    Some(Diagnostic {
        severity: Severity::Error,
        source: DiagnosticSource::Build,
        code: Some(DiagnosticCode("FILE_SIZE_LIMIT".into())),
        message: format!("{} has {count} lines and exceeds the source-file cap", path.display()),
        location: Some(SourceLocation {
            path,
            line: 1,
            column: 1,
            end_line: None,
            end_column: None,
        }),
        notes: Vec::new(),
        suggestion: Some("split the file by responsibility; see docs/module-layout.md".into()),
        rendered: None,
        relation: Some(DiagnosticRelation::Primary),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_file_size_violations_from_complete_output() {
        let input = "2 file(s) over the 600-line cap:\n\n    742  crates/a/src/lib.rs\n    611  crates/b/src/main.rs\n\nSplit them by responsibility — see docs/module-layout.md.\n";
        let diagnostics = parse_file_size_violations(input);
        assert_eq!(diagnostics.len(), 2);
        assert_eq!(diagnostics[0].code.as_ref().unwrap().0, "FILE_SIZE_LIMIT");
        assert_eq!(
            diagnostics[0].location.as_ref().unwrap().path,
            PathBuf::from("crates/a/src/lib.rs")
        );
        assert!(diagnostics[0].message.contains("742 lines"));
    }

    #[test]
    fn ignores_success_and_guidance_lines() {
        let diagnostics =
            parse_file_size_violations("All 421 Rust files are within 600 lines.\n");
        assert!(diagnostics.is_empty());
    }
}
