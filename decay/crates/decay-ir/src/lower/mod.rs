//! Turning an analysed program into instructions.
//!
//! A new statement or expression form is a function in the matching
//! leaf and one arm in the match that dispatches to it.

mod expr;
mod stmt;

use decay_semantic::Analysis;
use decay_syntax::{FunctionDecl, Item, Member};

use crate::ir::{ContainerKind, Instruction, IrContainer, IrField, IrFunction, IrProgram};

pub(crate) struct Lowerer;

impl Lowerer {
    pub(crate) fn lower_program(analysis: &Analysis) -> IrProgram {
        let containers = analysis
            .program
            .items
            .iter()
            .map(|item| match item {
                Item::Script(container) => Self::lower_container(ContainerKind::Script, container),
                Item::Component(container) => {
                    Self::lower_container(ContainerKind::Component, container)
                }
            })
            .collect();

        IrProgram { containers }
    }

    pub(crate) fn lower_container(
        kind: ContainerKind,
        container: &decay_syntax::ContainerDecl,
    ) -> IrContainer {
        let mut fields = Vec::new();
        let mut functions = Vec::new();

        for member in &container.members {
            match member {
                Member::Field(field) => {
                    let initializer = field.initializer.as_ref().map(|expr| {
                        let mut instructions = Vec::new();
                        Self::lower_expr(expr, &mut instructions);
                        instructions
                    });
                    fields.push(IrField {
                        name: field.name.clone(),
                        mutable: field.mutable,
                        exported: field
                            .attributes
                            .iter()
                            .any(|attribute| attribute.name == "export"),
                        type_name: field.ty.as_ref().map(|ty| ty.name.clone()),
                        initializer,
                    });
                }
                Member::Function(function) => functions.push(Self::lower_function(function)),
            }
        }

        IrContainer {
            kind,
            name: container.name.clone(),
            fields,
            functions,
        }
    }

    pub(crate) fn lower_function(function: &FunctionDecl) -> IrFunction {
        let mut instructions = Vec::new();
        // A function body is where a loop can exist, and each body starts with
        // none open; a field initializer is an expression and cannot contain one.
        Self::lower_block(
            &function.body,
            &mut instructions,
            &mut stmt::Loops::default(),
        );
        if !matches!(instructions.last(), Some(Instruction::Return)) {
            instructions.push(Instruction::Return);
        }

        IrFunction {
            name: function.name.clone(),
            params: function
                .params
                .iter()
                .map(|param| param.name.clone())
                .collect(),
            instructions,
        }
    }
}
