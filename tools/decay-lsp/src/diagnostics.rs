use decay_semantic::{Diagnostic, DiagnosticPhase};
use decay_syntax::Span;
use serde_json::{Value, json};

use crate::support::span_range;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Severity {
    Error,
    Warning,
}

impl Severity {
    const fn label(self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Warning => "warning",
        }
    }

    const fn lsp(self) -> u8 {
        match self {
            Self::Error => 1,
            Self::Warning => 2,
        }
    }
}

/// Tool-neutral diagnostic produced once and rendered for LSP, CLI, or JSON.
///
/// The span remains a Decay byte span. Renderers translate it to their own
/// position convention at the boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StructuredDiagnostic {
    pub(crate) severity: Severity,
    pub(crate) code: String,
    pub(crate) source: &'static str,
    pub(crate) message: String,
    pub(crate) span: Span,
}

impl StructuredDiagnostic {
    pub(crate) fn from_compiler(diagnostic: Diagnostic) -> Self {
        let (source, code) = match diagnostic.phase {
            DiagnosticPhase::Syntax => ("decay-syntax", "decay-syntax"),
            DiagnosticPhase::Semantic => ("decay-semantic", "decay-semantic"),
        };
        Self {
            severity: Severity::Error,
            code: code.to_owned(),
            source,
            message: diagnostic.message,
            span: diagnostic.span,
        }
    }

    pub(crate) fn reminder(
        code: &'static str,
        message: &'static str,
        offset: usize,
    ) -> Self {
        Self {
            severity: Severity::Warning,
            code: code.to_owned(),
            source: "decay-runtime-contract",
            message: message.to_owned(),
            span: Span {
                start: offset,
                end: offset,
            },
        }
    }

    pub(crate) fn lsp_json(&self, source: &str) -> Value {
        json!({
            "range": span_range(source, self.span),
            "severity": self.severity.lsp(),
            "code": self.code,
            "source": self.source,
            "message": self.message
        })
    }

    pub(crate) fn json(&self, path: &str, line: usize, column: usize) -> Value {
        json!({
            "path": path,
            "line": line,
            "column": column,
            "severity": self.severity.label(),
            "code": self.code,
            "source": self.source,
            "message": self.message,
            "span": {
                "start": self.span.start,
                "end": self.span.end
            }
        })
    }

    pub(crate) fn human(
        &self,
        path: &str,
        line: usize,
        column: usize,
    ) -> String {
        format!(
            "{path}:{line}:{column}: {}[{}]: {}",
            self.severity.label(),
            self.code,
            self.message
        )
    }
}

#[cfg(test)]
mod tests {
    use decay_semantic::DiagnosticPhase;

    use super::*;

    #[test]
    fn compiler_diagnostic_keeps_phase_identity() {
        let diagnostic = Diagnostic {
            phase: DiagnosticPhase::Semantic,
            message: "unknown name".to_owned(),
            span: Span { start: 4, end: 8 },
            line: 1,
            column: 5,
        };
        let structured = StructuredDiagnostic::from_compiler(diagnostic);
        assert_eq!(structured.code, "decay-semantic");
        assert_eq!(structured.source, "decay-semantic");
        assert_eq!(structured.severity, Severity::Error);
    }

    #[test]
    fn json_contains_machine_readable_location_and_code() {
        let diagnostic =
            StructuredDiagnostic::reminder("runtime-property-write", "use live state", 9);
        let value = diagnostic.json("game.decay", 2, 4);
        assert_eq!(value["path"], "game.decay");
        assert_eq!(value["line"], 2);
        assert_eq!(value["severity"], "warning");
        assert_eq!(value["code"], "runtime-property-write");
    }
}
