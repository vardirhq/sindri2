//! Portable intermediate representation for Decay.
//!
//! The IR is deliberately engine-agnostic and symbolic. It knows about Decay
//! control flow, values, names, member paths and calls, but it does not know
//! what a Transform, Entity, Input service, or any other host concept is.

use decay_semantic::{Analysis, Environment, analyze_with_environment};
use decay_syntax::{
    AssignOp, BinaryOp, Block, Expr, ExprKind, FunctionDecl, Item, Member, Stmt, UnaryOp,
};

#[derive(Debug, Clone, PartialEq)]
pub struct IrProgram {
    pub containers: Vec<IrContainer>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContainerKind {
    Script,
    Component,
}

#[derive(Debug, Clone, PartialEq)]
pub struct IrContainer {
    pub kind: ContainerKind,
    pub name: String,
    pub fields: Vec<IrField>,
    pub functions: Vec<IrFunction>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct IrField {
    pub name: String,
    pub mutable: bool,
    pub exported: bool,
    pub type_name: Option<String>,
    pub initializer: Option<Vec<Instruction>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct IrFunction {
    pub name: String,
    pub params: Vec<String>,
    pub instructions: Vec<Instruction>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Constant {
    Number(f64),
    String(String),
    Bool(bool),
    Null,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Path(pub Vec<String>);

impl Path {
    #[must_use]
    pub fn dotted(&self) -> String {
        self.0.join(".")
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Instruction {
    Push(Constant),
    Load(Path),
    Store(Path),
    Declare { name: String, mutable: bool },
    Unary(UnaryOp),
    Binary(BinaryOp),
    Call { callee: Path, argument_count: usize },
    Pop,
    Return,
    JumpIfFalse(usize),
    Jump(usize),
}

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

struct Lowerer;

impl Lowerer {
    fn lower_program(analysis: &Analysis) -> IrProgram {
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

    fn lower_container(
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

    fn lower_function(function: &FunctionDecl) -> IrFunction {
        let mut instructions = Vec::new();
        Self::lower_block(&function.body, &mut instructions);
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

    fn lower_block(block: &Block, instructions: &mut Vec<Instruction>) {
        for statement in &block.statements {
            Self::lower_stmt(statement, instructions);
        }
    }

    fn lower_stmt(statement: &Stmt, instructions: &mut Vec<Instruction>) {
        match statement {
            Stmt::Binding {
                mutable,
                name,
                initializer,
                ..
            } => {
                instructions.push(Instruction::Declare {
                    name: name.clone(),
                    mutable: *mutable,
                });
                if let Some(initializer) = initializer {
                    Self::lower_expr(initializer, instructions);
                    instructions.push(Instruction::Store(Path(vec![name.clone()])));
                    instructions.push(Instruction::Pop);
                }
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

                Self::lower_block(then_branch, instructions);

                if let Some(else_branch) = else_branch {
                    let jump_to_end = instructions.len();
                    instructions.push(Instruction::Jump(usize::MAX));
                    let else_start = instructions.len();
                    instructions[jump_if_false] = Instruction::JumpIfFalse(else_start);
                    Self::lower_block(else_branch, instructions);
                    let end = instructions.len();
                    instructions[jump_to_end] = Instruction::Jump(end);
                } else {
                    let end = instructions.len();
                    instructions[jump_if_false] = Instruction::JumpIfFalse(end);
                }
            }
            Stmt::Block(block) => Self::lower_block(block, instructions),
        }
    }

    fn lower_expr(expr: &Expr, instructions: &mut Vec<Instruction>) {
        match &expr.kind {
            ExprKind::Identifier(name) => {
                instructions.push(Instruction::Load(Path(vec![name.clone()])))
            }
            ExprKind::Number(value) => {
                instructions.push(Instruction::Push(Constant::Number(*value)))
            }
            ExprKind::String(value) => {
                instructions.push(Instruction::Push(Constant::String(value.clone())))
            }
            ExprKind::Bool(value) => instructions.push(Instruction::Push(Constant::Bool(*value))),
            ExprKind::Null => instructions.push(Instruction::Push(Constant::Null)),
            ExprKind::Group(inner) => Self::lower_expr(inner, instructions),
            ExprKind::Unary { op, expr } => {
                Self::lower_expr(expr, instructions);
                instructions.push(Instruction::Unary(*op));
            }
            ExprKind::Binary { left, op, right } => {
                Self::lower_expr(left, instructions);
                Self::lower_expr(right, instructions);
                instructions.push(Instruction::Binary(*op));
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

    fn path_from_expr(expr: &Expr) -> Option<Path> {
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

#[cfg(test)]
mod tests {
    use decay_semantic::{Environment, Type};

    use super::{Constant, Instruction, Path, lower, lower_with_environment};

    #[test]
    fn lowers_member_assignment_to_symbolic_path() {
        let lowered = lower(
            r#"
            script Player {
                fn update(dt: f32) {
                    this.transform.position.x += 6.0 * dt;
                }
            }
            "#,
        );
        let program = lowered.program.expect("program should lower");
        let instructions = &program.containers[0].functions[0].instructions;

        assert!(instructions.contains(&Instruction::Load(Path(vec![
            "this".into(),
            "transform".into(),
            "position".into(),
            "x".into(),
        ]))));
        assert!(instructions.contains(&Instruction::Push(Constant::Number(6.0))));
        assert!(instructions.contains(&Instruction::Store(Path(vec![
            "this".into(),
            "transform".into(),
            "position".into(),
            "x".into(),
        ]))));
    }

    #[test]
    fn lowers_host_call_without_knowing_host_semantics() {
        let mut environment = Environment::new();
        environment.add_value("Input", Type::Named("Input".to_owned()));
        let lowered = lower_with_environment(
            r#"
            script Player {
                fn update() {
                    Input.axis("left", "right");
                }
            }
            "#,
            &environment,
        );
        let program = lowered.program.expect("program should lower");
        let instructions = &program.containers[0].functions[0].instructions;

        assert!(instructions.iter().any(|instruction| matches!(
            instruction,
            Instruction::Call { callee, argument_count: 2 }
                if callee == &Path(vec!["Input".into(), "axis".into()])
        )));
    }

    #[test]
    fn patches_if_else_jump_targets() {
        let lowered = lower(
            r#"
            script Test {
                fn run(flag: bool) {
                    if flag {
                        var x: f32 = 1.0;
                    } else {
                        var x: f32 = 2.0;
                    }
                }
            }
            "#,
        );
        let program = lowered.program.expect("program should lower");
        let instructions = &program.containers[0].functions[0].instructions;

        for instruction in instructions {
            match instruction {
                Instruction::JumpIfFalse(target) | Instruction::Jump(target) => {
                    assert!(*target <= instructions.len());
                    assert_ne!(*target, usize::MAX);
                }
                _ => {}
            }
        }
    }

    #[test]
    fn refuses_to_lower_semantically_invalid_source() {
        let lowered = lower(
            r#"
            script Broken {
                fn run() {
                    let value: bool = 1.0;
                }
            }
            "#,
        );

        assert!(lowered.program.is_none());
        assert!(!lowered.analysis.diagnostics.is_empty());
    }
}
