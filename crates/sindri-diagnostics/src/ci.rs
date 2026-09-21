use crate::{Diagnostic, DiagnosticCode, DiagnosticRelation, DiagnosticSource, Severity};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CheckOutcome {
    Success,
    Failure,
    Skipped,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckResult {
    pub name: String,
    pub outcome: CheckOutcome,
    pub fingerprint: Option<String>,
    pub infrastructure: bool,
}

/// Correlates check-level failures so duplicate manifestations of one error do
/// not masquerade as separate root causes.
#[must_use]
pub fn correlate_checks(checks: &[CheckResult]) -> Vec<Diagnostic> {
    let mut first_for_fingerprint = BTreeMap::<&str, &str>::new();
    let mut diagnostics = Vec::new();

    for check in checks {
        if check.outcome != CheckOutcome::Failure {
            continue;
        }
        let relation = if check.infrastructure {
            DiagnosticRelation::Independent
        } else if let Some(fingerprint) = check.fingerprint.as_deref() {
            if first_for_fingerprint
                .insert(fingerprint, &check.name)
                .is_some()
            {
                DiagnosticRelation::Downstream
            } else {
                DiagnosticRelation::Primary
            }
        } else {
            DiagnosticRelation::Independent
        };
        diagnostics.push(Diagnostic {
            severity: Severity::Error,
            source: DiagnosticSource::Build,
            code: Some(DiagnosticCode(if check.infrastructure {
                "CI_INFRASTRUCTURE_FAILURE".into()
            } else {
                "CI_CHECK_FAILURE".into()
            })),
            message: format!("{} failed", check.name),
            location: None,
            notes: check
                .fingerprint
                .as_ref()
                .map(|fingerprint| vec![format!("failure fingerprint: {fingerprint}")])
                .unwrap_or_default(),
            suggestion: None,
            rendered: None,
            relation: Some(relation),
        });
    }
    diagnostics
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn groups_duplicate_failures_and_keeps_infrastructure_independent() {
        let diagnostics = correlate_checks(&[
            CheckResult {
                name: "Clippy".into(),
                outcome: CheckOutcome::Failure,
                fingerprint: Some("E0063:github.rs:68".into()),
                infrastructure: false,
            },
            CheckResult {
                name: "Tests".into(),
                outcome: CheckOutcome::Failure,
                fingerprint: Some("E0063:github.rs:68".into()),
                infrastructure: false,
            },
            CheckResult {
                name: "Browser".into(),
                outcome: CheckOutcome::Failure,
                fingerprint: Some("artifact-403".into()),
                infrastructure: true,
            },
            CheckResult {
                name: "Format".into(),
                outcome: CheckOutcome::Success,
                fingerprint: None,
                infrastructure: false,
            },
        ]);
        assert_eq!(diagnostics[0].relation, Some(DiagnosticRelation::Primary));
        assert_eq!(
            diagnostics[1].relation,
            Some(DiagnosticRelation::Downstream)
        );
        assert_eq!(
            diagnostics[2].relation,
            Some(DiagnosticRelation::Independent)
        );
    }
}
