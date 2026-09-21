use crate::{Diagnostic, DiagnosticCode, DiagnosticRelation, DiagnosticSource, Severity, SourceLocation};
use std::collections::BTreeSet;

/// Parses the stable file headers emitted by `cargo fmt --all --check`.
#[must_use]
pub fn parse_rustfmt_diff(input: &str) -> Vec<Diagnostic> {
    let paths: BTreeSet<_> = input
        .lines()
        .filter_map(|line| line.strip_prefix("Diff in "))
        .filter_map(|rest| rest.split_once(':').map(|(path, _)| path))
        .collect();

    paths
        .into_iter()
        .map(|path| Diagnostic {
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
        })
        .collect()
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
        assert_eq!(diagnostics[0].suggestion.as_deref(), Some("run cargo fmt --all"));
    }
}
