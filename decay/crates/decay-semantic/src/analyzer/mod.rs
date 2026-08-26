//! The walk over a parsed program, and the scope it keeps while walking.
//!
//! The traversal is here; each kind of thing it walks into has a file of
//! its own. A new statement form, expression form, or member rule is an
//! arm in the matching leaf, not a change to this file.

mod call;
mod expr;
mod item;
mod member;
mod stmt;

use std::collections::{HashMap, HashSet};

use decay_syntax::{Item, Program, Span};

use crate::diagnostic::{Diagnostic, DiagnosticPhase, line_column};
use crate::environment::{Environment, ExternalSymbol};
use crate::types::{FunctionType, Type};

/// What asking a type for a member found.
///
/// The distinction that matters is between a type the host described and one it
/// did not: only the former can say a member is missing.
pub(super) enum MemberLookup {
    Found(ExternalSymbol),
    Missing,
    /// A function the container itself declares, reached through `this`, which
    /// is never what the author meant -- see [`Analyzer::member_symbol`].
    ContainerFunction,
}

#[derive(Debug, Clone)]
pub(super) struct Symbol {
    ty: Type,
    mutable: bool,
    function: Option<FunctionType>,
}

pub(super) struct Analyzer<'a, 'd> {
    source: &'a str,
    environment: &'a Environment,
    diagnostics: &'d mut Vec<Diagnostic>,
    scopes: Vec<HashMap<String, Symbol>>,
    current_return: Type,
    /// Every `script` and `component` in the program, so that `this` can be
    /// told apart from a host type of the same shape.
    containers: HashSet<String>,
    /// How many `while` bodies enclose the statement being analyzed, so that a
    /// `break` or `continue` with nothing to break out of is refused here
    /// rather than lowered into a jump with no target.
    loop_depth: usize,
}

impl<'a, 'd> Analyzer<'a, 'd> {
    pub(super) fn new(
        source: &'a str,
        environment: &'a Environment,
        diagnostics: &'d mut Vec<Diagnostic>,
    ) -> Self {
        Self {
            source,
            environment,
            diagnostics,
            scopes: Vec::new(),
            current_return: Type::Unit,
            containers: HashSet::new(),
            loop_depth: 0,
        }
    }

    pub(super) fn analyze_program(&mut self, program: &Program) {
        let mut containers = HashMap::<String, Span>::new();

        // Collected up front: a function body may name `this`, and `this` can
        // only be resolved once it is known which names are containers.
        for item in &program.items {
            let (Item::Script(container) | Item::Component(container)) = item;
            self.containers.insert(container.name.clone());
        }

        for item in &program.items {
            let container = match item {
                Item::Script(container) | Item::Component(container) => container,
            };

            if containers
                .insert(container.name.clone(), container.span)
                .is_some()
            {
                self.error(
                    container.span,
                    format!("duplicate declaration `{}`", container.name),
                );
            }

            self.analyze_container(container);
        }
    }

    pub(super) fn resolve_identifier(&mut self, name: &str, span: Span) -> Type {
        if let Some(symbol) = self.lookup(name) {
            if symbol.function.is_some() {
                return Type::Unknown;
            }
            return symbol.ty.clone();
        }

        if let Some(symbol) = self.environment.globals.get(name) {
            return match symbol {
                ExternalSymbol::Value(ty) => ty.clone(),
                ExternalSymbol::Function(_) => Type::Unknown,
            };
        }

        self.error(span, format!("unknown name `{name}`"));
        Type::Unknown
    }

    pub(super) fn lookup(&self, name: &str) -> Option<&Symbol> {
        self.scopes.iter().rev().find_map(|scope| scope.get(name))
    }

    pub(super) fn require_type(&mut self, actual: &Type, expected: &Type, span: Span) {
        if !Self::compatible(expected, actual) {
            self.error(
                span,
                format!(
                    "expected `{}`, found `{}`",
                    expected.display_name(),
                    actual.display_name()
                ),
            );
        }
    }

    pub(super) fn check_assignable(&mut self, expected: &Type, actual: &Type, span: Span) {
        if !Self::compatible(expected, actual) {
            self.error(
                span,
                format!(
                    "cannot assign `{}` to `{}`",
                    actual.display_name(),
                    expected.display_name()
                ),
            );
        }
    }

    pub(super) fn compatible(expected: &Type, actual: &Type) -> bool {
        matches!(expected, Type::Unknown)
            || matches!(actual, Type::Unknown)
            || expected == actual
            || matches!((expected, actual), (Type::Named(_), Type::Null))
    }

    pub(super) fn error(&mut self, span: Span, message: String) {
        let (line, column) = line_column(self.source, span.start);
        self.diagnostics.push(Diagnostic {
            phase: DiagnosticPhase::Semantic,
            message,
            span,
            line,
            column,
        });
    }
}
