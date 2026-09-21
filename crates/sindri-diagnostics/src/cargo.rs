use crate::{Diagnostic, DiagnosticCode, DiagnosticSource, Severity, SourceLocation};
use serde::Deserialize;
use std::num::TryFromIntError;

/// Failure to decode Cargo's newline-delimited JSON message stream.
#[derive(Debug)]
pub enum CargoMessageError {
    Json(serde_json::Error),
    Position(TryFromIntError),
}

impl std::fmt::Display for CargoMessageError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Json(error) => write!(formatter, "invalid Cargo JSON message: {error}"),
            Self::Position(error) => write!(formatter, "diagnostic position is too large: {error}"),
        }
    }
}

impl std::error::Error for CargoMessageError {}

impl From<serde_json::Error> for CargoMessageError {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error)
    }
}

impl From<TryFromIntError> for CargoMessageError {
    fn from(error: TryFromIntError) -> Self {
        Self::Position(error)
    }
}

#[derive(Deserialize)]
struct CargoEnvelope {
    reason: String,
    message: Option<RustMessage>,
}

#[derive(Deserialize)]
struct RustMessage {
    message: String,
    code: Option<RustCode>,
    level: String,
    spans: Vec<RustSpan>,
    children: Vec<RustChild>,
    rendered: Option<String>,
}

#[derive(Deserialize)]
struct RustCode {
    code: String,
}

#[derive(Deserialize)]
struct RustSpan {
    file_name: String,
    line_start: u64,
    line_end: u64,
    column_start: u64,
    column_end: u64,
    is_primary: bool,
    suggested_replacement: Option<String>,
}

#[derive(Deserialize)]
struct RustChild {
    message: String,
    level: String,
}

/// Parses diagnostics from `cargo --message-format=json` output.
///
/// Non-diagnostic Cargo events are intentionally ignored, so callers can feed
/// the complete stdout stream without first separating compiler messages.
pub fn parse_cargo_messages(input: &str) -> Result<Vec<Diagnostic>, CargoMessageError> {
    input
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(serde_json::from_str::<CargoEnvelope>)
        .filter_map(|message| match message {
            Ok(envelope) if envelope.reason == "compiler-message" => envelope.message.map(Ok),
            Ok(_) => None,
            Err(error) => Some(Err(CargoMessageError::Json(error))),
        })
        .map(|message| message.and_then(convert_message))
        .collect()
}

fn convert_message(message: RustMessage) -> Result<Diagnostic, CargoMessageError> {
    let primary = message.spans.iter().find(|span| span.is_primary);
    let location = primary.map(location_from_span).transpose()?;
    let suggestion = primary.and_then(|span| span.suggested_replacement.clone());
    let notes = message
        .children
        .into_iter()
        .filter(|child| matches!(child.level.as_str(), "note" | "help"))
        .map(|child| child.message)
        .collect();

    Ok(Diagnostic {
        severity: severity(&message.level),
        source: DiagnosticSource::Rust,
        code: message.code.map(|code| DiagnosticCode(code.code)),
        message: message.message,
        location,
        notes,
        suggestion,
        rendered: message.rendered,\n        relation: None,
    })
}

fn location_from_span(span: &RustSpan) -> Result<SourceLocation, TryFromIntError> {
    Ok(SourceLocation {
        path: span.file_name.clone().into(),
        line: u32::try_from(span.line_start)?,
        column: u32::try_from(span.column_start)?,
        end_line: Some(u32::try_from(span.line_end)?),
        end_column: Some(u32::try_from(span.column_end)?),
    })
}

fn severity(level: &str) -> Severity {
    match level {
        "error" | "failure-note" => Severity::Error,
        "warning" => Severity::Warning,
        "help" => Severity::Help,
        _ => Severity::Note,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_actionable_rust_diagnostic() {
        let input = r#"{"reason":"compiler-artifact","package_id":"ignored"}
{"reason":"compiler-message","message":{"message":"unused variable: `texture`","code":{"code":"unused_variables","explanation":null},"level":"warning","spans":[{"file_name":"src/lib.rs","byte_start":0,"byte_end":1,"line_start":12,"line_end":12,"column_start":9,"column_end":16,"is_primary":true,"text":[],"label":null,"suggested_replacement":"_texture","suggestion_applicability":"MaybeIncorrect","expansion":null}],"children":[{"message":"prefix it with an underscore","code":null,"level":"help","spans":[]}],"rendered":"warning: unused variable"}}"#;

        let diagnostics = parse_cargo_messages(input).unwrap();
        assert_eq!(diagnostics.len(), 1);
        let diagnostic = &diagnostics[0];
        assert_eq!(diagnostic.severity, Severity::Warning);
        assert_eq!(diagnostic.code.as_ref().unwrap().0, "unused_variables");
        assert_eq!(diagnostic.location.as_ref().unwrap().line, 12);
        assert_eq!(diagnostic.suggestion.as_deref(), Some("_texture"));
        assert_eq!(diagnostic.notes, ["prefix it with an underscore"]);
    }
}
