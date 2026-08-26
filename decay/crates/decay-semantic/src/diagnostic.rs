//! What analysis reports, and where it points.

use decay_syntax::{Program, Span};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticPhase {
    Syntax,
    Semantic,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub phase: DiagnosticPhase,
    pub message: String,
    pub span: Span,
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Analysis {
    pub program: Program,
    pub diagnostics: Vec<Diagnostic>,
}

/// Decay has no methods, and the mistake of reaching for one is worth naming
/// precisely rather than leaving as a runtime `FunctionNotFound`.
pub(crate) fn container_function_message(field: &str) -> String {
    format!(
        "`{field}` is this script's own function; call it as `{field}(...)` rather than `this.{field}(...)`"
    )
}

pub(crate) fn line_column(source: &str, offset: usize) -> (usize, usize) {
    let prefix = &source[..offset.min(source.len())];
    let line = prefix.bytes().filter(|byte| *byte == b'\n').count() + 1;
    let column = prefix
        .rsplit_once('\n')
        .map_or(prefix.len() + 1, |(_, tail)| tail.len() + 1);
    (line, column)
}
