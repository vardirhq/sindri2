use crate::{
    Diagnostic, DiagnosticCode, DiagnosticRelation, DiagnosticSource, Severity, SourceLocation,
};
use std::collections::BTreeSet;
use std::path::PathBuf;

/// Parses the stable file headers emitted by `cargo fmt --all --check`.
#[must_use]
pub fn parse_rustfmt_diff(input: &str) -> Vec<Diagnostic> {
    let mut diagnostics = parse_rust_errors(input);
    let paths: BTreeSet<_> = input
        .lines()
        .filter_map(|line| line.strip_prefix("Diff in "))
        .filter_map(|rest| rest.split_once(':').map(|(path, _)| path))
        .collect();

    diagnostics.extend(paths.into_iter().map(|path| Diagnostic {
            severity: Severity::Error,
            source: DiagnosticSource::Build,
            code: Some(DiagnosticCode("RUSTFMT_REQUIRED".into())),
            message: format!("{path} requires rustfmt"),
            location: Some(SourceLocation {
                path: path.into(),
                line: 1,
                column: 1,
                end_line: None,
                end_column: None,
            }),
            notes: Vec::new(),
            suggestion: Some("run cargo fmt --all".into()),
            rendered: None,
            relation: Some(DiagnosticRelation::Primary),
        }));
    diagnostics
}

fn parse_rust_errors(input: &str) -> Vec<Diagnostic> {
    let lines: Vec<_> = input.lines().collect();
    let mut diagnostics = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        let Some(message) = line.strip_prefix("error: ") else { continue; };
        let Some(location_line) = lines.get(index + 1) else { continue; };
        let Some(location) = parse_arrow_location(location_line.trim()) else { continue; };
        diagnostics.push(Diagnostic {
            severity: Severity::Error,
            source: DiagnosticSource::Rust,
            code: Some(DiagnosticCode("RUST_PARSE_ERROR".into())),
            message: message.to_owned(),
            location: Some(location),
            notes: Vec::new(),
            suggestion: None,
            rendered: None,
            relation: Some(DiagnosticRelation::Primary),
        });
    }
    diagnostics
}

fn parse_arrow_location(line: &str) -> Option<SourceLocation> {
    let location = line.strip_prefix("--> ")?.trim();
    let (path_and_line, column) = location.rsplit_once(':')?;
    let (path, line) = path_and_line.rsplit_once(':')?;
    Some(SourceLocation {
        path: PathBuf::from(path),
        line: line.parse().ok()?,
        column: column.parse().ok()?,
        end_line: None,
        end_column: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collapses_multiple_diffs_for_the_same_file() {
        let input = "Diff in /work/src/a.rs:10:\n-old\n+new\nDiff in /work/src/a.rs:30:\n-old\n+new\nDiff in /work/src/b.rs:4:\n";
        let diagnostics = parse_rustfmt_diff(input);
        assert_eq!(diagnostics.len(), 2);
        assert_eq!(diagnostics[0].code.as_ref().unwrap().0, "RUSTFMT_REQUIRED");
        assert_eq!(
            diagnostics[0].suggestion.as_deref(),
            Some("run cargo fmt --all")
        );
    }

    #[test]
    fn distinguishes_parse_errors_from_formatting_diffs() {
        let input = "error: expected one of `!` or `::`\n  --> crates/example/src/lib.rs:85:38\n   |\nDiff in crates/other/src/lib.rs:4:\n";
        let diagnostics = parse_rustfmt_diff(input);
        assert_eq!(diagnostics.len(), 2);
        assert_eq!(diagnostics[0].code.as_ref().unwrap().0, "RUST_PARSE_ERROR");
        assert_eq!(diagnostics[0].source, DiagnosticSource::Rust);
        let location = diagnostics[0].location.as_ref().unwrap();
        assert_eq!(location.path, PathBuf::from("crates/example/src/lib.rs"));
        assert_eq!(location.line, 85);
        assert_eq!(location.column, 38);
        assert_eq!(diagnostics[1].code.as_ref().unwrap().0, "RUSTFMT_REQUIRED");
    }
}
