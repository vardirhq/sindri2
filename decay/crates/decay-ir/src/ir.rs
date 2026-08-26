//! The instruction set, and the shapes a lowered program is made of.
//!
//! Symbolic and engine-agnostic on purpose: this knows about Decay
//! control flow, values, names, member paths, and calls, and nothing
//! about what a Transform or an Entity is.

use decay_syntax::{BinaryOp, UnaryOp};

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
    /// Assigns to a name that already exists, and is subject to its
    /// mutability. Binding a new name is [`Instruction::Declare`].
    Store(Path),
    /// Pops the initial value and binds it to a new name in the innermost
    /// scope.
    ///
    /// Declaring takes the value rather than leaving the slot empty for a
    /// following `Store`, because a `let` binding's own initialization would
    /// then have to be an exception to the runtime's immutability rule. It was
    /// not, so every `let` local failed at runtime; taking the value here means
    /// there is no initializing store to make an exception for.
    Declare {
        name: String,
        mutable: bool,
    },
    Unary(UnaryOp),
    Binary(BinaryOp),
    Call {
        callee: Path,
        argument_count: usize,
    },
    Pop,
    Return,
    JumpIfFalse(usize),
    Jump(usize),
    /// Opens a nested scope, so that a name declared inside a block leaves
    /// with it.
    ///
    /// The analyzer has always scoped blocks; without these the IR did not,
    /// and a shadowing declaration overwrote the name it shadowed for the rest
    /// of the function.
    ScopeEnter,
    /// Closes the scope [`Instruction::ScopeEnter`] opened, discarding what it
    /// declared.
    ScopeExit,
}
