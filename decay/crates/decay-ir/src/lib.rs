//! Portable intermediate representation for Decay.
//!
//! The IR is deliberately engine-agnostic and symbolic. It knows about Decay
//! control flow, values, names, member paths and calls, but it does not know
//! what a Transform, Entity, Input service, or any other host concept is.
//!
//! `ir` is the instruction set; `lower` is the walk that produces it, one file
//! per kind of thing it walks into.

mod ir;
mod lower;

#[cfg(test)]
mod tests;

use decay_semantic::{Analysis, Environment, analyze_with_environment};

use lower::Lowerer;

pub use ir::{
    Constant, ContainerKind, Instruction, IrContainer, IrField, IrFunction, IrProgram, Path,
};

#[derive(Debug, Clone, PartialEq)]
pub struct Lowered {
    pub analysis: Analysis,
    pub program: Option<IrProgram>,
}

#[must_use]
pub fn lower(source: &str) -> Lowered {
    lower_with_environment(source, &Environment::default())
}

#[must_use]
pub fn lower_with_environment(source: &str, environment: &Environment) -> Lowered {
    let analysis = analyze_with_environment(source, environment);
    if !analysis.diagnostics.is_empty() {
        return Lowered {
            analysis,
            program: None,
        };
    }

    let program = Some(Lowerer::lower_program(&analysis));
    Lowered { analysis, program }
}
