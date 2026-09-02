//! What analysis reports, and where it points.

use std::collections::HashMap;

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

/// A member the analyzer resolved on a *value* rather than along a path.
///
/// Most of `a.b.c` is a path the host gives a meaning to, and the IR forwards
/// it whole. A few members belong to a value the language itself understands,
/// and those cannot be forwarded — there is no host to ask what the length of
/// a collection is. The analyzer knows which is which because it knows the
/// types, so it says so here rather than leaving the lowering to guess from a
/// member's name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueMember {
    /// How many elements a collection holds.
    Length,
}

/// A map rather than the set Clippy suggests for the one variant it carries
/// today: what the lowering needs is *which* member, and the next value type to
/// arrive — a position, with an `x` and a `y` — makes that more than one
/// answer. A set would have to become this again, and every call site with it.
#[allow(clippy::zero_sized_map_values)]
pub type ValueMembers = HashMap<Span, ValueMember>;

#[allow(clippy::zero_sized_map_values)]
#[derive(Debug, Clone, PartialEq)]
pub struct Analysis {
    pub program: Program,
    pub diagnostics: Vec<Diagnostic>,
    /// Where the program reads a member of a value, and which one.
    ///
    /// Keyed by the member expression's span, which is unique per expression
    /// and is what the lowering walk has in hand when it reaches one.
    pub value_members: ValueMembers,
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
