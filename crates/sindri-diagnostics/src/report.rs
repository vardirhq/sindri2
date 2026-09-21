use crate::{Diagnostic, DiagnosticRelation, Severity};
use serde::{Deserialize, Serialize};

/// Version of the JSON diagnostics contract.
pub const SCHEMA_VERSION: u32 = 1;

/// A complete machine-readable validation result.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct DiagnosticReport {
    pub schema_version: u32,
    pub success: bool,
    pub diagnostics: Vec<Diagnostic>,
}

impl DiagnosticReport {
    #[must_use]
    pub fn new(diagnostics: Vec<Diagnostic>) -> Self {
        let success = !diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == Severity::Error);
        Self {
            schema_version: SCHEMA_VERSION,
            success,
            diagnostics,
        }
    }

    #[must_use]
    pub fn error_count(&self) -> usize {
        self.diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.severity == Severity::Error)
            .count()
    }

    #[must_use]
    pub fn warning_count(&self) -> usize {
        self.diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.severity == Severity::Warning)
            .count()
    }

    #[must_use]
    pub fn primary(&self) -> impl Iterator<Item = &Diagnostic> {
        self.diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.relation == Some(DiagnosticRelation::Primary))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DiagnosticSource;

    fn diagnostic(severity: Severity, relation: Option<DiagnosticRelation>) -> Diagnostic {
        Diagnostic {
            severity,
            source: DiagnosticSource::Build,
            code: None,
            message: "example".into(),
            location: None,
            notes: Vec::new(),
            suggestion: None,
            rendered: None,
            relation,
        }
    }

    #[test]
    fn errors_make_a_report_fail_and_primary_is_queryable() {
        let report = DiagnosticReport::new(vec![
            diagnostic(Severity::Warning, None),
            diagnostic(Severity::Error, Some(DiagnosticRelation::Primary)),
        ]);
        assert!(!report.success);
        assert_eq!(report.error_count(), 1);
        assert_eq!(report.warning_count(), 1);
        assert_eq!(report.primary().count(), 1);
    }
}
