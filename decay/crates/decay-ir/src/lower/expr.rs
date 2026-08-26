//! One function per expression form, and the instructions it becomes.

use decay_syntax::{AssignOp, BinaryOp, Expr, ExprKind};

use crate::ir::{Constant, Instruction, Path};

use super::Lowerer;

impl Lowerer {
    /// `&&` and `||`, as branches rather than as an operation over two values
    /// already evaluated.
    ///
    /// This is the whole of short-circuiting: the right operand is lowered
    /// behind a jump the left operand can take, so a left that already decides
    /// the answer skips it. It matters most where it is least visible — a
    /// guard such as `held != null && World.exists(held)` reads as protecting
    /// the call to its right, and only does when the call is skipped.
    ///
    /// The answer is pushed as a constant rather than left as whichever
    /// operand survived, because `&&` and `||` are typed `bool` and should go
    /// on producing one. `JumpIfFalse` pops what it tested and refuses a value
    /// that is not a `bool`, so testing each operand that is reached keeps the
    /// check `Instruction::Binary` used to perform on both.
    pub(super) fn lower_short_circuit(
        left: &Expr,
        op: BinaryOp,
        right: &Expr,
        instructions: &mut Vec<Instruction>,
    ) {
        // Filled with the sites that cannot know their target until the shared
        // tail below exists, and patched once it does.
        let mut to_false = Vec::new();
        let mut to_true = Vec::new();

        Self::lower_expr(left, instructions);
        if matches!(op, BinaryOp::And) {
            to_false.push(instructions.len());
            instructions.push(Instruction::JumpIfFalse(usize::MAX));
        } else {
            // `||` is decided by a *true* left operand, and the only test
            // available jumps on false — so the jump that skips the right
            // operand is the one taken by falling through.
            let past = instructions.len();
            instructions.push(Instruction::JumpIfFalse(usize::MAX));
            to_true.push(instructions.len());
            instructions.push(Instruction::Jump(usize::MAX));
            let right_start = instructions.len();
            instructions[past] = Instruction::JumpIfFalse(right_start);
        }

        Self::lower_expr(right, instructions);
        to_false.push(instructions.len());
        instructions.push(Instruction::JumpIfFalse(usize::MAX));

        let true_target = instructions.len();
        instructions.push(Instruction::Push(Constant::Bool(true)));
        let to_end = instructions.len();
        instructions.push(Instruction::Jump(usize::MAX));

        let false_target = instructions.len();
        instructions.push(Instruction::Push(Constant::Bool(false)));
        let end = instructions.len();

        for site in to_false {
            instructions[site] = Instruction::JumpIfFalse(false_target);
        }
        for site in to_true {
            instructions[site] = Instruction::Jump(true_target);
        }
        instructions[to_end] = Instruction::Jump(end);
    }

    pub(super) fn lower_expr(expr: &Expr, instructions: &mut Vec<Instruction>) {
        match &expr.kind {
            ExprKind::Identifier(name) => {
                instructions.push(Instruction::Load(Path(vec![name.clone()])));
            }
            ExprKind::Number(value) => {
                instructions.push(Instruction::Push(Constant::Number(*value)));
            }
            ExprKind::String(value) => {
                instructions.push(Instruction::Push(Constant::String(value.clone())));
            }
            ExprKind::Bool(value) => instructions.push(Instruction::Push(Constant::Bool(*value))),
            ExprKind::Null => instructions.push(Instruction::Push(Constant::Null)),
            ExprKind::Group(inner) => Self::lower_expr(inner, instructions),
            ExprKind::Unary { op, expr } => {
                Self::lower_expr(expr, instructions);
                instructions.push(Instruction::Unary(*op));
            }
            ExprKind::Binary { left, op, right } => {
                if matches!(op, BinaryOp::And | BinaryOp::Or) {
                    Self::lower_short_circuit(left, *op, right, instructions);
                } else {
                    Self::lower_expr(left, instructions);
                    Self::lower_expr(right, instructions);
                    instructions.push(Instruction::Binary(*op));
                }
            }
            ExprKind::Assign { target, op, value } => {
                let path =
                    Self::path_from_expr(target).unwrap_or_else(|| Path(vec!["<invalid>".into()]));
                if matches!(op, AssignOp::Assign) {
                    Self::lower_expr(value, instructions);
                } else {
                    instructions.push(Instruction::Load(path.clone()));
                    Self::lower_expr(value, instructions);
                    let binary = match op {
                        AssignOp::Add => BinaryOp::Add,
                        AssignOp::Subtract => BinaryOp::Subtract,
                        AssignOp::Multiply => BinaryOp::Multiply,
                        AssignOp::Divide => BinaryOp::Divide,
                        AssignOp::Modulo => BinaryOp::Modulo,
                        AssignOp::Assign => unreachable!(),
                    };
                    instructions.push(Instruction::Binary(binary));
                }
                instructions.push(Instruction::Store(path));
            }
            ExprKind::Member { .. } => {
                let path = Self::path_from_expr(expr)
                    .unwrap_or_else(|| Path(vec!["<invalid-member>".into()]));
                instructions.push(Instruction::Load(path));
            }
            ExprKind::Call { callee, args } => {
                for argument in args {
                    Self::lower_expr(argument, instructions);
                }
                let callee = Self::path_from_expr(callee)
                    .unwrap_or_else(|| Path(vec!["<invalid-call>".into()]));
                instructions.push(Instruction::Call {
                    callee,
                    argument_count: args.len(),
                });
            }
        }
    }

    pub(super) fn path_from_expr(expr: &Expr) -> Option<Path> {
        fn collect(expr: &Expr, parts: &mut Vec<String>) -> bool {
            match &expr.kind {
                ExprKind::Identifier(name) => {
                    parts.push(name.clone());
                    true
                }
                ExprKind::Member { object, field } => {
                    if !collect(object, parts) {
                        return false;
                    }
                    parts.push(field.clone());
                    true
                }
                ExprKind::Group(inner) => collect(inner, parts),
                _ => false,
            }
        }

        let mut parts = Vec::new();
        collect(expr, &mut parts).then_some(Path(parts))
    }
}
