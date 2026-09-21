//! Structured, tool-neutral diagnostics for Sindri projects and build tooling.
//!
//! Producers raise diagnostics once; terminals, CI, editors, and coding agents
//! can render the same data without scraping human prose.

mod cargo;
mod diagnostic;
mod github;
mod report;
mod terminal;
mod test;

pub use cargo::{CargoMessageError, parse_cargo_messages};
pub use diagnostic::{
    Diagnostic, DiagnosticCode, DiagnosticRelation, DiagnosticSource, Severity, SourceLocation,
};
pub use github::GithubAnnotation;
pub use report::{DiagnosticReport, SCHEMA_VERSION};
pub use terminal::render_terminal_report;
pub use test::parse_nextest_failure;
