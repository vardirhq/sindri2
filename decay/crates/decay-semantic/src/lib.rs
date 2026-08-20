//! Semantic analysis for the Decay gameplay language.
//!
//! This crate remains engine-agnostic. A host such as Sindri supplies globals
//! and host types through [`Environment`] rather than being compiled into the
//! language itself.

use std::collections::HashMap;

use decay_syntax::{
    AssignOp, BinaryOp, Block, Expr, ExprKind, FieldDecl, FunctionDecl, Item, Member, Program,
    Span, Stmt, TypeRef, UnaryOp, parse,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Type {
    F32,
    Bool,
    String,
    Unit,
    Null,
    Named(String),
    Unknown,
}

impl Type {
    #[must_use]
    pub fn from_ref(reference: &TypeRef) -> Self {
        match reference.name.as_str() {
            "f32" => Self::F32,
            "bool" => Self::Bool,
            "String" | "string" => Self::String,
            "unit" | "void" => Self::Unit,
            other => Self::Named(other.to_owned()),
        }
    }

    fn display_name(&self) -> &str {
        match self {
            Self::F32 => "f32",
            Self::Bool => "bool",
            Self::String => "String",
            Self::Unit => "unit",
            Self::Null => "null",
            Self::Named(name) => name,
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionType {
    pub params: Vec<Type>,
    pub return_type: Type,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExternalSymbol {
    Value(Type),
    Function(FunctionType),
}

#[derive(Debug, Clone, Default)]
pub struct Environment {
    globals: HashMap<String, ExternalSymbol>,
}

impl Environment {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_value(&mut self, name: impl Into<String>, ty: Type) {
        self.globals.insert(name.into(), ExternalSymbol::Value(ty));
    }

    pub fn add_function(&mut self, name: impl Into<String>, function: FunctionType) {
        self.globals
            .insert(name.into(), ExternalSymbol::Function(function));
    }
}

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

#[derive(Debug, Clone, PartialEq)]
pub struct Analysis {
    pub program: Program,
    pub diagnostics: Vec<Diagnostic>,
}

#[must_use]
pub fn analyze(source: &str) -> Analysis {
    analyze_with_environment(source, &Environment::default())
}

#[must_use]
pub fn analyze_with_environment(source: &str, environment: &Environment) -> Analysis {
    let parsed = parse(source);
    let mut diagnostics = parsed
        .diagnostics
        .into_iter()
        .map(|diagnostic| Diagnostic {
            phase: DiagnosticPhase::Syntax,
            message: diagnostic.message,
            span: diagnostic.span,
            line: diagnostic.line,
            column: diagnostic.column,
        })
        .collect::<Vec<_>>();

    let mut analyzer = Analyzer::new(source, environment, &mut diagnostics);
    analyzer.analyze_program(&parsed.program);

    Analysis {
        program: parsed.program,
        diagnostics,
    }
}

#[derive(Debug, Clone)]
struct Symbol {
    ty: Type,
    mutable: bool,
    function: Option<FunctionType>,
}

struct Analyzer<'a, 'd> {
    source: &'a str,
    environment: &'a Environment,
    diagnostics: &'d mut Vec<Diagnostic>,
    scopes: Vec<HashMap<String, Symbol>>,
    current_return: Type,
}

impl<'a, 'd> Analyzer<'a, 'd> {
    fn new(
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
        }
    }

    fn analyze_program(&mut self, program: &Program) {
        let mut containers = HashMap::<String, Span>::new();

        for item in &program.items {
            let container = match item {
                Item::Script(container) | Item::Component(container) => container,
            };

            if containers.insert(container.name.clone(), container.span).is_some() {
                self.error(
                    container.span,
                    format!("duplicate declaration `{}`", container.name),
                );
            }

            self.analyze_container(container);
        }
    }

    fn analyze_container(&mut self, container: &decay_syntax::ContainerDecl) {
        let mut members = HashMap::new();

        for member in &container.members {
            match member {
                Member::Field(field) => {
                    let ty = field
                        .ty
                        .as_ref()
                        .map_or(Type::Unknown, Type::from_ref);
                    self.insert_member(
                        &mut members,
                        &field.name,
                        Symbol {
                            ty,
                            mutable: field.mutable,
                            function: None,
                        },
                        field.span,
                    );
                }
                Member::Function(function) => {
                    let signature = FunctionType {
                        params: function
                            .params
                            .iter()
                            .map(|param| {
                                param.ty.as_ref().map_or(Type::Unknown, Type::from_ref)
                            })
                            .collect(),
                        return_type: function
                            .return_type
                            .as_ref()
                            .map_or(Type::Unit, Type::from_ref),
                    };
                    self.insert_member(
                        &mut members,
                        &function.name,
                        Symbol {
                            ty: Type::Unknown,
                            mutable: false,
                            function: Some(signature),
                        },
                        function.span,
                    );
                }
            }
        }

        for member in &container.members {
            if let Member::Field(field) = member {
                self.check_field_initializer(field, &members);
            }
        }

        for member in &container.members {
            if let Member::Function(function) = member {
                self.analyze_function(container, function, &members);
            }
        }
    }

    fn insert_member(
        &mut self,
        members: &mut HashMap<String, Symbol>,
        name: &str,
        symbol: Symbol,
        span: Span,
    ) {
        if members.insert(name.to_owned(), symbol).is_some() {
            self.error(span, format!("duplicate member `{name}`"));
        }
    }

    fn check_field_initializer(&mut self, field: &FieldDecl, members: &HashMap<String, Symbol>) {
        let Some(initializer) = &field.initializer else {
            if field.ty.is_none() {
                self.error(
                    field.span,
                    format!("field `{}` needs a type or initializer", field.name),
                );
            }
            return;
        };

        self.scopes = vec![members.clone()];
        let actual = self.expr_type(initializer);
        if let Some(expected) = field.ty.as_ref().map(Type::from_ref) {
            self.check_assignable(&expected, &actual, initializer.span);
        }
        self.scopes.clear();
    }

    fn analyze_function(
        &mut self,
        container: &decay_syntax::ContainerDecl,
        function: &FunctionDecl,
        members: &HashMap<String, Symbol>,
    ) {
        self.current_return = function
            .return_type
            .as_ref()
            .map_or(Type::Unit, Type::from_ref);
        self.scopes = vec![members.clone(), HashMap::new()];

        self.define_local(
            "this",
            Symbol {
                ty: Type::Named(container.name.clone()),
                mutable: false,
                function: None,
            },
            function.span,
        );

        for param in &function.params {
            self.define_local(
                &param.name,
                Symbol {
                    ty: param.ty.as_ref().map_or(Type::Unknown, Type::from_ref),
                    mutable: false,
                    function: None,
                },
                param.span,
            );
        }

        self.analyze_block(&function.body, false);
        self.scopes.clear();
        self.current_return = Type::Unit;
    }

    fn analyze_block(&mut self, block: &Block, create_scope: bool) {
        if create_scope {
            self.scopes.push(HashMap::new());
        }

        for statement in &block.statements {
            self.analyze_stmt(statement);
        }

        if create_scope {
            self.scopes.pop();
        }
    }

    fn analyze_stmt(&mut self, statement: &Stmt) {
        match statement {
            Stmt::Binding {
                mutable,
                name,
                ty,
                initializer,
                span,
            } => {
                let initializer_type = initializer
                    .as_ref()
                    .map_or(Type::Unknown, |expr| self.expr_type(expr));
                let declared_type = ty.as_ref().map(Type::from_ref);

                if declared_type.is_none() && initializer.is_none() {
                    self.error(*span, format!("binding `{name}` needs a type or initializer"));
                }

                if let Some(expected) = &declared_type {
                    self.check_assignable(expected, &initializer_type, *span);
                }

                self.define_local(
                    name,
                    Symbol {
                        ty: declared_type.unwrap_or(initializer_type),
                        mutable: *mutable,
                        function: None,
                    },
                    *span,
                );
            }
            Stmt::Expr { expr, .. } => {
                self.expr_type(expr);
            }
            Stmt::Return { value, span } => {
                let actual = value.as_ref().map_or(Type::Unit, |expr| self.expr_type(expr));
                let expected = self.current_return.clone();
                self.check_assignable(&expected, &actual, *span);
            }
            Stmt::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                let condition_type = self.expr_type(condition);
                if !matches!(condition_type, Type::Bool | Type::Unknown) {
                    self.error(
                        condition.span,
                        format!(
                            "if condition must be `bool`, found `{}`",
                            condition_type.display_name()
                        ),
                    );
                }
                self.analyze_block(then_branch, true);
                if let Some(else_branch) = else_branch {
                    self.analyze_block(else_branch, true);
                }
            }
            Stmt::Block(block) => self.analyze_block(block, true),
        }
    }

    fn expr_type(&mut self, expr: &Expr) -> Type {
        match &expr.kind {
            ExprKind::Identifier(name) => self.resolve_identifier(name, expr.span),
            ExprKind::Number(_) => Type::F32,
            ExprKind::String(_) => Type::String,
            ExprKind::Bool(_) => Type::Bool,
            ExprKind::Null => Type::Null,
            ExprKind::Group(inner) => self.expr_type(inner),
            ExprKind::Unary { op, expr: inner } => {
                let inner_type = self.expr_type(inner);
                match op {
                    UnaryOp::Negate => {
                        self.require_type(&inner_type, &Type::F32, inner.span);
                        Type::F32
                    }
                    UnaryOp::Not => {
                        self.require_type(&inner_type, &Type::Bool, inner.span);
                        Type::Bool
                    }
                }
            }
            ExprKind::Binary { left, op, right } => self.binary_type(left, *op, right),
            ExprKind::Assign { target, op, value } => self.assignment_type(target, *op, value),
            ExprKind::Member { object, .. } => {
                self.expr_type(object);
                Type::Unknown
            }
            ExprKind::Call { callee, args } => self.call_type(callee, args, expr.span),
        }
    }

    fn binary_type(&mut self, left: &Expr, op: BinaryOp, right: &Expr) -> Type {
        let left_type = self.expr_type(left);
        let right_type = self.expr_type(right);

        match op {
            BinaryOp::Add | BinaryOp::Subtract | BinaryOp::Multiply | BinaryOp::Divide => {
                self.require_type(&left_type, &Type::F32, left.span);
                self.require_type(&right_type, &Type::F32, right.span);
                Type::F32
            }
            BinaryOp::Less
            | BinaryOp::LessEqual
            | BinaryOp::Greater
            | BinaryOp::GreaterEqual => {
                self.require_type(&left_type, &Type::F32, left.span);
                self.require_type(&right_type, &Type::F32, right.span);
                Type::Bool
            }
            BinaryOp::And | BinaryOp::Or => {
                self.require_type(&left_type, &Type::Bool, left.span);
                self.require_type(&right_type, &Type::Bool, right.span);
                Type::Bool
            }
            BinaryOp::Equal | BinaryOp::NotEqual => {
                if !self.compatible(&left_type, &right_type) {
                    self.error(
                        right.span,
                        format!(
                            "cannot compare `{}` with `{}`",
                            left_type.display_name(),
                            right_type.display_name()
                        ),
                    );
                }
                Type::Bool
            }
        }
    }

    fn assignment_type(&mut self, target: &Expr, op: AssignOp, value: &Expr) -> Type {
        let target_type = self.assignment_target_type(target);
        let value_type = self.expr_type(value);

        if !matches!(op, AssignOp::Assign) {
            self.require_type(&target_type, &Type::F32, target.span);
            self.require_type(&value_type, &Type::F32, value.span);
        } else {
            self.check_assignable(&target_type, &value_type, value.span);
        }

        target_type
    }

    fn assignment_target_type(&mut self, target: &Expr) -> Type {
        match &target.kind {
            ExprKind::Identifier(name) => {
                if let Some(symbol) = self.lookup(name).cloned() {
                    if symbol.function.is_some() {
                        self.error(target.span, format!("cannot assign to function `{name}`"));
                    } else if !symbol.mutable {
                        self.error(target.span, format!("cannot assign to immutable `{name}`"));
                    }
                    symbol.ty
                } else {
                    self.error(target.span, format!("unknown name `{name}`"));
                    Type::Unknown
                }
            }
            ExprKind::Member { object, .. } => {
                self.expr_type(object);
                Type::Unknown
            }
            _ => {
                self.error(target.span, "invalid assignment target".to_owned());
                Type::Unknown
            }
        }
    }

    fn call_type(&mut self, callee: &Expr, args: &[Expr], span: Span) -> Type {
        if let ExprKind::Identifier(name) = &callee.kind {
            if let Some(symbol) = self.lookup(name).cloned() {
                if let Some(function) = symbol.function {
                    self.check_call(&function, args, span);
                    return function.return_type;
                }
                self.error(callee.span, format!("`{name}` is not callable"));
                for arg in args {
                    self.expr_type(arg);
                }
                return Type::Unknown;
            }

            if let Some(external) = self.environment.globals.get(name).cloned() {
                match external {
                    ExternalSymbol::Function(function) => {
                        self.check_call(&function, args, span);
                        return function.return_type;
                    }
                    ExternalSymbol::Value(_) => {
                        self.error(callee.span, format!("`{name}` is not callable"));
                    }
                }
                for arg in args {
                    self.expr_type(arg);
                }
                return Type::Unknown;
            }
        }

        self.expr_type(callee);
        for arg in args {
            self.expr_type(arg);
        }
        Type::Unknown
    }

    fn check_call(&mut self, function: &FunctionType, args: &[Expr], span: Span) {
        if function.params.len() != args.len() {
            self.error(
                span,
                format!(
                    "expected {} argument(s), found {}",
                    function.params.len(),
                    args.len()
                ),
            );
        }

        for (argument, expected) in args.iter().zip(&function.params) {
            let actual = self.expr_type(argument);
            self.check_assignable(expected, &actual, argument.span);
        }
    }

    fn resolve_identifier(&mut self, name: &str, span: Span) -> Type {
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

    fn define_local(&mut self, name: &str, symbol: Symbol, span: Span) {
        let Some(scope) = self.scopes.last_mut() else {
            return;
        };
        if scope.insert(name.to_owned(), symbol).is_some() {
            self.error(span, format!("duplicate local `{name}`"));
        }
    }

    fn lookup(&self, name: &str) -> Option<&Symbol> {
        self.scopes.iter().rev().find_map(|scope| scope.get(name))
    }

    fn require_type(&mut self, actual: &Type, expected: &Type, span: Span) {
        if !self.compatible(expected, actual) {
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

    fn check_assignable(&mut self, expected: &Type, actual: &Type, span: Span) {
        if !self.compatible(expected, actual) {
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

    fn compatible(&self, expected: &Type, actual: &Type) -> bool {
        matches!(expected, Type::Unknown)
            || matches!(actual, Type::Unknown)
            || expected == actual
            || matches!((expected, actual), (Type::Named(_), Type::Null))
    }

    fn error(&mut self, span: Span, message: String) {
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

fn line_column(source: &str, offset: usize) -> (usize, usize) {
    let prefix = &source[..offset.min(source.len())];
    let line = prefix.bytes().filter(|byte| *byte == b'\n').count() + 1;
    let column = prefix
        .rsplit_once('\n')
        .map_or(prefix.len() + 1, |(_, tail)| tail.len() + 1);
    (line, column)
}

#[cfg(test)]
mod tests {
    use super::{DiagnosticPhase, Environment, FunctionType, Type, analyze, analyze_with_environment};

    #[test]
    fn accepts_typed_gameplay_code() {
        let analysis = analyze(
            r#"
            script Player {
                let speed: f32 = 6.0;

                fn update(dt: f32) {
                    var movement: f32 = 1.0;
                    movement += speed * dt;

                    if movement > 0.0 {
                        movement = movement - 1.0;
                    }
                }
            }
            "#,
        );

        assert!(analysis.diagnostics.is_empty(), "{:?}", analysis.diagnostics);
    }

    #[test]
    fn reports_unknown_names_and_type_mismatches() {
        let analysis = analyze(
            r#"
            script Broken {
                fn update(dt: f32) {
                    var alive: bool = true;
                    alive = 1.0;
                    missing = dt;
                }
            }
            "#,
        );

        assert!(analysis.diagnostics.iter().any(|diagnostic| {
            diagnostic.phase == DiagnosticPhase::Semantic
                && diagnostic.message.contains("cannot assign `f32` to `bool`")
        }));
        assert!(analysis.diagnostics.iter().any(|diagnostic| {
            diagnostic.phase == DiagnosticPhase::Semantic
                && diagnostic.message.contains("unknown name `missing`")
        }));
    }

    #[test]
    fn rejects_assignment_to_immutable_bindings() {
        let analysis = analyze(
            r#"
            script Player {
                fn update() {
                    let speed: f32 = 6.0;
                    speed = 8.0;
                }
            }
            "#,
        );

        assert!(analysis.diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("cannot assign to immutable `speed`")
        }));
    }

    #[test]
    fn host_globals_are_injected_without_engine_dependencies() {
        let mut environment = Environment::new();
        environment.add_function(
            "delta",
            FunctionType {
                params: vec![],
                return_type: Type::F32,
            },
        );
        environment.add_value("Input", Type::Named("Input".to_owned()));

        let analysis = analyze_with_environment(
            r#"
            script Player {
                fn update() {
                    let dt: f32 = delta();
                    Input.axis("left", "right");
                }
            }
            "#,
            &environment,
        );

        assert!(analysis.diagnostics.is_empty(), "{:?}", analysis.diagnostics);
    }

    #[test]
    fn catches_duplicate_members_and_locals() {
        let analysis = analyze(
            r#"
            script Player {
                let speed: f32 = 1.0;
                let speed: f32 = 2.0;

                fn update() {
                    let value: f32 = 1.0;
                    let value: f32 = 2.0;
                }
            }
            "#,
        );

        assert!(analysis.diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("duplicate member `speed`")
        }));
        assert!(analysis.diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("duplicate local `value`")
        }));
    }
}
