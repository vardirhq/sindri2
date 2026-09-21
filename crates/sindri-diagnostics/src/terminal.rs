use std::fmt::Write;

use crate::{Diagnostic, DiagnosticRelation, DiagnosticReport};

#[must_use]
pub fn render_terminal_report(report: &DiagnosticReport) -> String {
    if report.success {
        return format!(
            "Sindri diagnostics: clean ({} warning(s))",
            report.warning_count()
        );
    }
    let mut output = format!(
        "Sindri diagnostics: {} error(s), {} warning(s)\n",
        report.error_count(),
        report.warning_count()
    );
    for diagnostic in &report.diagnostics {
        render_diagnostic(&mut output, diagnostic);
    }
    output
}

fn render_diagnostic(output: &mut String, diagnostic: &Diagnostic) {
    let relation = match diagnostic.relation {
        Some(DiagnosticRelation::Primary) => "ROOT CAUSE",
        Some(DiagnosticRelation::Downstream) => "DOWNSTREAM",
        Some(DiagnosticRelation::Independent) => "INDEPENDENT",
        None => "DIAGNOSTIC",
    };
    let _ = write!(output, "\n{relation}: {}", diagnostic.message);
    if let Some(location) = &diagnostic.location {
        let _ = write!(
            output,
            "\n  at {}:{}:{}",
            location.path.display(),
            location.line,
            location.column
        );
    }
    for note in &diagnostic.notes {
        let _ = write!(output, "\n  {note}");
    }
    if let Some(suggestion) = &diagnostic.suggestion {
        let _ = write!(output, "\n  suggestion: {suggestion}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DiagnosticCode, DiagnosticSource, Severity, SourceLocation};

    #[test]
    fn highlights_root_cause() {
        let report = DiagnosticReport::new(vec![Diagnostic {
            severity: Severity::Error,
            source: DiagnosticSource::Test,
            code: Some(DiagnosticCode("TEST_FAILURE".into())),
            message: "fixture is malformed".into(),
            location: Some(SourceLocation {
                path: "src/test.rs".into(),
                line: 12,
                column: 4,
                end_line: None,
                end_column: None,
            }),
            notes: vec!["failed test: parses_fixture".into()],
            suggestion: None,
            rendered: None,
            relation: Some(DiagnosticRelation::Primary),
        }]);
        let rendered = render_terminal_report(&report);
        assert!(rendered.contains("ROOT CAUSE: fixture is malformed"));
        assert!(rendered.contains("src/test.rs:12:4"));
    }
}
