//! Turning an analysed program into instructions.
//!
//! A new statement or expression form is a function in the matching
//! leaf and one arm in the match that dispatches to it.

mod expr;
mod stmt;

use decay_semantic::{Analysis, ValueMember, ValueMembers};
use decay_syntax::{FunctionDecl, Item, Member, Span};

use crate::ir::{ContainerKind, Instruction, IrContainer, IrField, IrFunction, IrProgram};

/// The walk, and what the analysis told it.
///
/// Stateful because lowering is not purely syntactic: a member read may be a
/// path the host answers or a property of a value the language owns, and only
/// the analysis knows which. Guessing from the member's name would make `len`
/// a name no host type could ever use.
#[allow(clippy::zero_sized_map_values)]
pub(crate) struct Lowerer<'a> {
    value_members: &'a ValueMembers,
}

impl<'a> Lowerer<'a> {
    pub(crate) fn lower_program(analysis: &'a Analysis) -> IrProgram {
        let lowerer = Self {
            value_members: &analysis.value_members,
        };
        let containers = analysis
            .program
            .items
            .iter()
            .map(|item| match item {
                Item::Script(container) => {
                    lowerer.lower_container(ContainerKind::Script, container)
                }
                Item::Component(container) => {
                    lowerer.lower_container(ContainerKind::Component, container)
                }
            })
            .collect();

        IrProgram { containers }
    }

    /// What the analysis decided a member read at this span is, if anything.
    pub(super) fn value_member(&self, span: Span) -> Option<ValueMember> {
        self.value_members.get(&span).copied()
    }

    pub(crate) fn lower_container(
        &self,
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
                        self.lower_expr(expr, &mut instructions);
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
                Member::Function(function) => functions.push(self.lower_function(function)),
            }
        }

        IrContainer {
            kind,
            name: container.name.clone(),
            fields,
            functions,
        }
    }

    pub(crate) fn lower_function(&self, function: &FunctionDecl) -> IrFunction {
        let mut instructions = Vec::new();
        // A function body is where a loop can exist, and each body starts with
        // none open; a field initializer is an expression and cannot contain one.
        self.lower_block(
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
