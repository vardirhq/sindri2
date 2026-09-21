use crate::{Diagnostic, DiagnosticCode, DiagnosticRelation, DiagnosticReport, DiagnosticSource, Severity};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fmt::Write};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckOutcome {
    Success,
    Failure,
    Skipped,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckResult {
    pub name: String,
    pub outcome: CheckOutcome,
    pub fingerprint: Option<String>,
    pub infrastructure: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FailureFingerprint {
    pub value: String,
    pub infrastructure: bool,
}

/// Derives a fingerprint from an already-structured diagnostic report.
///
/// Prefer this over reparsing rendered output whenever the producer has already
/// identified a stable code and source location.
#[must_use]
pub fn fingerprint_report(report: &DiagnosticReport) -> Option<FailureFingerprint> {
    let diagnostic = report
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.severity == Severity::Error)?;
    let code = diagnostic.code.as_ref()?;
    let location = diagnostic
        .location
        .as_ref()
        .map(|location| format!("{}:{}", location.path.display(), location.line));
    let source = match &diagnostic.source {
        DiagnosticSource::Rust => "rust",
        DiagnosticSource::Test => "test",
        DiagnosticSource::Decay => "decay",
        DiagnosticSource::Asset => "asset",
        DiagnosticSource::Scene => "scene",
        DiagnosticSource::Project => "project",
        DiagnosticSource::Build => "build",
        DiagnosticSource::Other(source) => source.as_str(),
    };
    Some(FailureFingerprint {
        value: match location {
            Some(location) => format!("diagnostic:{source}:{}:{location}", code.0),
            None => format!("diagnostic:{source}:{}", code.0),
        },
        infrastructure: false,
    })
}

/// Extracts a stable-enough fingerprint from common CI failure output.
///
/// The fingerprint deliberately prefers compiler/test identities over rendered
/// prose so the same root failure can be recognized across Clippy, test, WASM,
/// and browser jobs without pretending unrelated errors are equivalent.
#[must_use]
pub fn fingerprint_failure(output: &str) -> Option<FailureFingerprint> {
    let lower = output.to_ascii_lowercase();
    if let Some(infrastructure) = infrastructure_fingerprint(&lower) {
        return Some(FailureFingerprint {
            value: infrastructure.into(),
            infrastructure: true,
        });
    }

    let lines = output.lines().collect::<Vec<_>>();
    for (index, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if let Some(code) = rust_error_code(trimmed) {
            let location = source_location_after(&lines, index).unwrap_or_default();
            return Some(FailureFingerprint {
                value: format!("rust:{code}:{location}"),
                infrastructure: false,
            });
        }
    }

    if let Some(test) = failed_test_name(output) {
        return Some(FailureFingerprint {
            value: format!("test:{test}"),
            infrastructure: false,
        });
    }

    first_actionable_line(output).map(|line| FailureFingerprint {
        value: format!("message:{}", normalize_message(line)),
        infrastructure: false,
    })
}

fn infrastructure_fingerprint(lower: &str) -> Option<&'static str> {
    [
        ("http 403", "infra:http-403"),
        ("status code: 403", "infra:http-403"),
        ("rate limit", "infra:rate-limit"),
        ("no space left on device", "infra:disk-full"),
        ("connection timed out", "infra:network-timeout"),
        ("connection reset by peer", "infra:connection-reset"),
        ("the operation was canceled", "infra:runner-cancelled"),
    ]
    .into_iter()
    .find_map(|(needle, fingerprint)| lower.contains(needle).then_some(fingerprint))
}

fn rust_error_code(line: &str) -> Option<&str> {
    let start = line.find("error[E")? + "error[".len();
    let rest = &line[start..];
    let end = rest.find(']')?;
    let code = &rest[..end];
    (code.len() == 5 && code.starts_with('E') && code[1..].chars().all(|c| c.is_ascii_digit()))
        .then_some(code)
}

fn source_location_after(lines: &[&str], error_index: usize) -> Option<String> {
    lines.iter().skip(error_index + 1).take(4).find_map(|line| {
        let location = line.trim().strip_prefix("-->")?.trim();
        let mut parts = location.rsplitn(3, ':');
        let column = parts.next()?;
        let line_number = parts.next()?;
        let path = parts.next()?;
        (column.parse::<u32>().is_ok() && line_number.parse::<u32>().is_ok())
            .then(|| format!("{path}:{line_number}"))
    })
}

fn failed_test_name(output: &str) -> Option<&str> {
    output.lines().find_map(|line| {
        let trimmed = line.trim();
        trimmed
            .strip_prefix("test ")
            .and_then(|rest| rest.strip_suffix(" ... FAILED"))
            .or_else(|| {
                trimmed
                    .strip_prefix("---- ")
                    .and_then(|rest| rest.strip_suffix(" stdout ----"))
            })
    })
}

fn first_actionable_line(output: &str) -> Option<&str> {
    output.lines().map(str::trim).find(|line| {
        !line.is_empty()
            && (line.starts_with("error:")
                || line.starts_with("Error:")
                || line.contains("FAILED")
                || line.contains("failed"))
    })
}

fn normalize_message(line: &str) -> String {
    line.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(160)
        .collect()
}

/// Renders correlated check failures as a compact Markdown summary.
#[must_use]
pub fn render_ci_summary(checks: &[CheckResult]) -> String {
    let diagnostics = correlate_checks(checks);
    let primary = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.relation == Some(DiagnosticRelation::Primary))
        .count();
    let downstream = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.relation == Some(DiagnosticRelation::Downstream))
        .count();
    let independent = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.relation == Some(DiagnosticRelation::Independent))
        .count();

    let mut output = format!(
        "## CI failure correlation\n\n**{primary} primary, {downstream} downstream, {independent} independent failure(s)**\n\n"
    );
    for diagnostic in diagnostics {
        let relation = match diagnostic.relation {
            Some(DiagnosticRelation::Primary) => "primary",
            Some(DiagnosticRelation::Downstream) => "downstream",
            Some(DiagnosticRelation::Independent) | None => "independent",
        };
        let _ = write!(output, "- **{relation}**: {}", diagnostic.message);
        if let Some(fingerprint) = diagnostic.notes.first() {
            let _ = write!(output, " ({fingerprint})");
        }
        output.push('\n');
    }
    output
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
    fn fingerprints_structured_report_by_source_code_and_location() {
        let report = DiagnosticReport::new(vec![Diagnostic {
            severity: Severity::Error,
            source: DiagnosticSource::Build,
            code: Some(DiagnosticCode("FILE_SIZE_LIMIT".into())),
            message: "too large".into(),
            location: Some(crate::SourceLocation {
                path: "crates/a/src/lib.rs".into(),
                line: 601,
                column: 1,
                end_line: None,
                end_column: None,
            }),
            notes: Vec::new(),
            suggestion: None,
            rendered: None,
            relation: None,
        }]);
        assert_eq!(
            fingerprint_report(&report).unwrap().value,
            "diagnostic:build:FILE_SIZE_LIMIT:crates/a/src/lib.rs:601"
        );
    }

    #[test]
    fn fingerprints_compiler_error_by_code_and_location() {
        let output = "error[E0063]: missing field\n  --> crates/a/src/lib.rs:68:9\n";
        assert_eq!(
            fingerprint_failure(output),
            Some(FailureFingerprint {
                value: "rust:E0063:crates/a/src/lib.rs:68".into(),
                infrastructure: false,
            })
        );
    }

    #[test]
    fn pairs_compiler_code_with_its_own_location() {
        let output = "warning: earlier\n  --> crates/warn.rs:2:1\nerror[E0425]: missing\n  --> crates/fail.rs:9:4\n";
        assert_eq!(
            fingerprint_failure(output).unwrap().value,
            "rust:E0425:crates/fail.rs:9"
        );
    }

    #[test]
    fn fingerprints_rust_test_failure_by_name() {
        let output = "test scene::tests::loads_prefab ... FAILED\n";
        assert_eq!(
            fingerprint_failure(output).unwrap().value,
            "test:scene::tests::loads_prefab"
        );
    }

    #[test]
    fn recognizes_infrastructure_before_generic_error_text() {
        let output = "Error: artifact upload failed with HTTP 403";
        assert_eq!(
            fingerprint_failure(output),
            Some(FailureFingerprint {
                value: "infra:http-403".into(),
                infrastructure: true,
            })
        );
    }

    #[test]
    fn renders_correlation_summary() {
        let checks = [
            CheckResult {
                name: "Clippy".into(),
                outcome: CheckOutcome::Failure,
                fingerprint: Some("rust:E0063:a.rs:9".into()),
                infrastructure: false,
            },
            CheckResult {
                name: "Browser target".into(),
                outcome: CheckOutcome::Failure,
                fingerprint: Some("rust:E0063:a.rs:9".into()),
                infrastructure: false,
            },
        ];
        let summary = render_ci_summary(&checks);
        assert!(summary.contains("1 primary, 1 downstream, 0 independent"));
        assert!(summary.contains("**primary**: Clippy failed"));
        assert!(summary.contains("**downstream**: Browser target failed"));
    }

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
