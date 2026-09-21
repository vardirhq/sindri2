use std::path::PathBuf;

use serde::Deserialize;

use crate::{
    Diagnostic, DiagnosticCode, DiagnosticReport, DiagnosticSource, Severity, SourceLocation,
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DecayReport {
    schema_version: u32,
    diagnostics: Vec<DecayDiagnostic>,
}

#[derive(Debug, Deserialize)]
struct DecayDiagnostic {
    path: PathBuf,
    line: u32,
    column: u32,
    severity: DecaySeverity,
    code: String,
    message: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
enum DecaySeverity {
    Error,
    Warning,
}

#[derive(Debug)]
pub enum DecayReportError {
    Json(serde_json::Error),
    UnsupportedSchema(u32),
}

impl std::fmt::Display for DecayReportError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Json(error) => write!(formatter, "invalid Decay diagnostic JSON: {error}"),
            Self::UnsupportedSchema(version) => {
                write!(
                    formatter,
                    "unsupported Decay diagnostic schema version {version}"
                )
            }
        }
    }
}

impl std::error::Error for DecayReportError {}

impl From<serde_json::Error> for DecayReportError {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error)
    }
}

/// Convert the versioned Decay checker JSON contract into Sindri's
/// tool-neutral diagnostic report.
///
/// # Errors
/// Returns an error when the input is not valid JSON or uses an unsupported schema version.
pub fn parse_decay_report(input: &str) -> Result<DiagnosticReport, DecayReportError> {
    let report: DecayReport = serde_json::from_str(input)?;
    if report.schema_version != 1 {
        return Err(DecayReportError::UnsupportedSchema(report.schema_version));
    }

    let diagnostics = report
        .diagnostics
        .into_iter()
        .map(|diagnostic| Diagnostic {
            severity: match diagnostic.severity {
                DecaySeverity::Error => Severity::Error,
                DecaySeverity::Warning => Severity::Warning,
            },
            source: DiagnosticSource::Decay,
            code: Some(DiagnosticCode(diagnostic.code)),
            message: diagnostic.message,
            location: Some(SourceLocation {
                path: diagnostic.path,
                line: diagnostic.line,
                column: diagnostic.column,
                end_line: None,
                end_column: None,
            }),
            notes: Vec::new(),
            suggestion: None,
            rendered: None,
            relation: None,
        })
        .collect();

    Ok(DiagnosticReport::new(diagnostics))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_errors_and_runtime_reminders() {
        let report = parse_decay_report(
            r#"{
                "schemaVersion": 1,
                "success": false,
                "filesChecked": 1,
                "errorCount": 1,
                "reminderCount": 1,
                "diagnostics": [
                    {
                        "path": "game/player.decay",
                        "line": 4,
                        "column": 7,
                        "severity": "error",
                        "code": "decay-semantic",
                        "source": "decay-semantic",
                        "message": "unknown name",
                        "span": {"start": 20, "end": 24}
                    },
                    {
                        "path": "game/player.decay",
                        "line": 8,
                        "column": 3,
                        "severity": "warning",
                        "code": "runtime-property-write",
                        "source": "decay-runtime-contract",
                        "message": "use live state",
                        "span": {"start": 50, "end": 50}
                    }
                ]
            }"#,
        )
        .expect("valid Decay report");

        assert!(!report.success);
        assert_eq!(report.error_count(), 1);
        assert_eq!(report.warning_count(), 1);
        assert_eq!(report.diagnostics[0].source, DiagnosticSource::Decay);
        assert_eq!(
            report.diagnostics[0].location.as_ref().unwrap().path,
            PathBuf::from("game/player.decay")
        );
    }

    #[test]
    fn rejects_unknown_schema_versions() {
        let error = parse_decay_report(r#"{"schemaVersion":2,"diagnostics":[]}"#)
            .expect_err("schema must be rejected");
        assert!(matches!(error, DecayReportError::UnsupportedSchema(2)));
    }
}
