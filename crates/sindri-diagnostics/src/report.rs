use crate::{Diagnostic, DiagnosticRelation, Severity};
use serde::{Deserialize, Serialize};

pub const SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct DiagnosticReport {
    pub schema_version: u32,
    pub success: bool,
    pub diagnostics: Vec<Diagnostic>,
}

impl DiagnosticReport {
    #[must_use]
    pub fn new(diagnostics: Vec<Diagnostic>) -> Self {
        let success = !diagnostics.iter().any(|d| d.severity == Severity::Error);
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
            .filter(|d| d.severity == Severity::Error)
            .count()
    }

    #[must_use]
    pub fn warning_count(&self) -> usize {
        self.diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Warning)
            .count()
    }

    #[must_use]
    pub fn primary(&self) -> impl Iterator<Item = &Diagnostic> {
        self.diagnostics
            .iter()
            .filter(|d| d.relation == Some(DiagnosticRelation::Primary))
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
    fn errors_fail_and_primary_is_queryable() {
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
