//! Structured, tool-neutral diagnostics for Sindri projects and build tooling.
//!
//! Producers raise diagnostics once; terminals, CI, editors, and coding agents
//! can render the same data without scraping human prose.

mod cargo;
mod ci;
mod decay;
mod diagnostic;
mod file_size;
mod format;
mod github;
mod report;
mod terminal;
mod test;

pub use cargo::{CargoMessageError, parse_cargo_messages};
pub use ci::{
    CheckOutcome, CheckResult, FailureFingerprint, correlate_checks, fingerprint_failure,
};
pub use decay::{DecayReportError, parse_decay_report};
pub use diagnostic::{
    Diagnostic, DiagnosticCode, DiagnosticRelation, DiagnosticSource, Severity, SourceLocation,
};
pub use file_size::parse_file_size_violations;
pub use format::parse_rustfmt_diff;
pub use github::GithubAnnotation;
pub use report::{DiagnosticReport, SCHEMA_VERSION};
pub use terminal::render_terminal_report;
pub use test::parse_nextest_failure;
