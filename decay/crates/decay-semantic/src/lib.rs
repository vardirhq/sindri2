//! Semantic analysis for the Decay gameplay language.
//!
//! This crate remains engine-agnostic. A host such as Sindri supplies globals
//! and host types through [`Environment`] rather than being compiled into the
//! language itself.
//!
//! `types` and `environment` are what the analysis is told; `analyzer` is the
//! walk that checks a program against them, one file per kind of thing it
//! walks into; `diagnostic` is what it reports.

mod analyzer;
mod diagnostic;
mod environment;
mod types;

#[cfg(test)]
mod tests;

use decay_syntax::parse;

use analyzer::Analyzer;

pub use diagnostic::{Analysis, Diagnostic, DiagnosticPhase, ValueMember, ValueMembers};
pub use environment::{Environment, ExternalSymbol};
pub use types::{FunctionType, HostType, Type};

#[must_use]
pub fn analyze(source: &str) -> Analysis {
    analyze_with_environment(source, &Environment::default())
}

// See `ValueMembers` for why the analysis carries a map of a zero-sized value.
#[allow(clippy::zero_sized_map_values)]
#[must_use]
pub fn analyze_with_environment(source: &str, environment: &Environment) -> Analysis {
    let parsed = parse(source);
    let mut diagnostics = parsed
        .diagnostics
        .into_iter()
        .map(|diagnostic| Diagnostic {
            phase: DiagnosticPhase::Syntax,
            message: diagnostic.message,
            span: diagnostic.span,
            line: diagnostic.line,
            column: diagnostic.column,
        })
        .collect::<Vec<_>>();

    let mut value_members = ValueMembers::new();
    let mut analyzer = Analyzer::new(source, environment, &mut diagnostics, &mut value_members);
    analyzer.analyze_program(&parsed.program);

    Analysis {
        program: parsed.program,
        diagnostics,
        value_members,
    }
}
