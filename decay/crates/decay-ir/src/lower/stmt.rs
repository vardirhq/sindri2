//! Statements and blocks, and the scopes a block opens.

use decay_syntax::{Block, Stmt};

use crate::ir::{Constant, Instruction};

use super::Lowerer;

impl Lowerer {
    pub(super) fn lower_block(block: &Block, instructions: &mut Vec<Instruction>) {
        for statement in &block.statements {
            Self::lower_stmt(statement, instructions);
        }
    }

    /// A block that introduces a scope of its own.
    ///
    /// Every branch is wrapped rather than only the ones that declare
    /// something, so the enter and exit stay paired however the jumps around
    /// them are patched. A `Return` leaving a scope unclosed costs nothing:
    /// the frame goes with it.
    pub(super) fn lower_scoped_block(block: &Block, instructions: &mut Vec<Instruction>) {
        instructions.push(Instruction::ScopeEnter);
        Self::lower_block(block, instructions);
        instructions.push(Instruction::ScopeExit);
    }

    pub(super) fn lower_stmt(statement: &Stmt, instructions: &mut Vec<Instruction>) {
        match statement {
            Stmt::Binding {
                mutable,
                name,
                initializer,
                ..
            } => {
                // The initializer is evaluated before the name exists, which is
                // also what the language means: a binding cannot refer to
                // itself.
                if let Some(initializer) = initializer {
                    Self::lower_expr(initializer, instructions);
                } else {
                    instructions.push(Instruction::Push(Constant::Null));
                }
                instructions.push(Instruction::Declare {
                    name: name.clone(),
                    mutable: *mutable,
                });
            }
            Stmt::Expr { expr, .. } => {
                Self::lower_expr(expr, instructions);
                instructions.push(Instruction::Pop);
            }
            Stmt::Return { value, .. } => {
                if let Some(value) = value {
                    Self::lower_expr(value, instructions);
                }
                instructions.push(Instruction::Return);
            }
            Stmt::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                Self::lower_expr(condition, instructions);
                let jump_if_false = instructions.len();
                instructions.push(Instruction::JumpIfFalse(usize::MAX));

                Self::lower_scoped_block(then_branch, instructions);

                if let Some(else_branch) = else_branch {
                    let jump_to_end = instructions.len();
                    instructions.push(Instruction::Jump(usize::MAX));
                    let else_start = instructions.len();
                    instructions[jump_if_false] = Instruction::JumpIfFalse(else_start);
                    Self::lower_scoped_block(else_branch, instructions);
                    let end = instructions.len();
                    instructions[jump_to_end] = Instruction::Jump(end);
                } else {
                    let end = instructions.len();
                    instructions[jump_if_false] = Instruction::JumpIfFalse(end);
                }
            }
            Stmt::Block(block) => Self::lower_scoped_block(block, instructions),
        }
    }
}
