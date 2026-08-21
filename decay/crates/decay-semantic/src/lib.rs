//! Semantic analysis for the Decay gameplay language.
//!
//! This crate remains engine-agnostic. A host such as Sindri supplies globals
//! and host types through [`Environment`] rather than being compiled into the
//! language itself.

use std::collections::{HashMap, HashSet};

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

    /// How this type is written in Decay source, and in a diagnostic about it.
    ///
    /// Public because a host describing itself — in an error, or in a manifest
    /// a tool reads — needs to name a type the same way the compiler does.
    #[must_use]
    pub fn display_name(&self) -> &str {
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

/// A host type, described by what it offers.
///
/// The language has no way to declare one: `Transform` is a name Decay carries
/// and cannot look inside, so the host is the only thing that can say a
/// transform has a position. Until it did, every member access produced
/// `Unknown`, `Unknown` is compatible with everything, and
/// `this.transfrom.position.x` type-checked cleanly and failed at frame one.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HostType {
    members: HashMap<String, ExternalSymbol>,
}

impl HostType {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn with_value(mut self, name: impl Into<String>, ty: Type) -> Self {
        self.members.insert(name.into(), ExternalSymbol::Value(ty));
        self
    }

    #[must_use]
    pub fn with_function(mut self, name: impl Into<String>, function: FunctionType) -> Self {
        self.members
            .insert(name.into(), ExternalSymbol::Function(function));
        self
    }

    #[must_use]
    pub fn member(&self, name: &str) -> Option<&ExternalSymbol> {
        self.members.get(name)
    }

    /// Whether the host said anything about this type at all.
    ///
    /// A type with no members is treated as *undescribed* rather than as
    /// described-and-empty, so that a host which has not started describing
    /// itself behaves exactly as every host did before types existed. The two
    /// are indistinguishable and only one of them is useful.
    #[must_use]
    pub fn is_described(&self) -> bool {
        !self.members.is_empty()
    }

    /// Every member, for a host emitting a description of itself.
    pub fn members(&self) -> impl Iterator<Item = (&str, &ExternalSymbol)> {
        self.members
            .iter()
            .map(|(name, symbol)| (name.as_str(), symbol))
    }
}

#[derive(Debug, Clone, Default)]
pub struct Environment {
    globals: HashMap<String, ExternalSymbol>,
    types: HashMap<String, HostType>,
    /// What `this` offers beyond the container's own fields.
    ///
    /// `this` is two things at once: the script's own state, and the entity the
    /// host attached it to. A container field always wins, so a script can
    /// never be shadowed by the engine growing a name.
    this: HostType,
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

    /// Describes a named type's members.
    ///
    /// A named type that is *not* described stays permissive: its members are
    /// `Unknown`, exactly as every member was before this existed. That is
    /// deliberate — describing the host is gradual, and a host part-way through
    /// describing itself must not reject scripts that were working.
    pub fn add_type(&mut self, name: impl Into<String>, ty: HostType) {
        self.types.insert(name.into(), ty);
    }

    /// Adds a member to `this`, such as the transform of the entity a script is
    /// attached to.
    pub fn add_this_value(&mut self, name: impl Into<String>, ty: Type) {
        self.this = std::mem::take(&mut self.this).with_value(name, ty);
    }

    /// Adds a callable member to `this`.
    pub fn add_this_function(&mut self, name: impl Into<String>, function: FunctionType) {
        self.this = std::mem::take(&mut self.this).with_function(name, function);
    }

    #[must_use]
    pub fn get_type(&self, name: &str) -> Option<&HostType> {
        self.types.get(name)
    }

    #[must_use]
    pub const fn this(&self) -> &HostType {
        &self.this
    }

    /// Every described type, for a host emitting a description of itself.
    pub fn types(&self) -> impl Iterator<Item = (&str, &HostType)> {
        self.types.iter().map(|(name, ty)| (name.as_str(), ty))
    }

    /// Every global, for the same reason.
    pub fn globals(&self) -> impl Iterator<Item = (&str, &ExternalSymbol)> {
        self.globals
            .iter()
            .map(|(name, symbol)| (name.as_str(), symbol))
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

/// What asking a type for a member found.
///
/// The distinction that matters is between a type the host described and one it
/// did not: only the former can say a member is missing.
enum MemberLookup {
    Found(ExternalSymbol),
    Missing,
    /// A function the container itself declares, reached through `this`, which
    /// is never what the author meant -- see [`Analyzer::member_symbol`].
    ContainerFunction,
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
    /// Every `script` and `component` in the program, so that `this` can be
    /// told apart from a host type of the same shape.
    containers: HashSet<String>,
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
            containers: HashSet::new(),
        }
    }

    fn analyze_program(&mut self, program: &Program) {
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

    fn analyze_container(&mut self, container: &decay_syntax::ContainerDecl) {
        let mut members = HashMap::new();

        for member in &container.members {
            match member {
                Member::Field(field) => {
                    let ty = field.ty.as_ref().map_or(Type::Unknown, Type::from_ref);
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
                            .map(|param| param.ty.as_ref().map_or(Type::Unknown, Type::from_ref))
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
                    self.error(
                        *span,
                        format!("binding `{name}` needs a type or initializer"),
                    );
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
                let actual = value
                    .as_ref()
                    .map_or(Type::Unit, |expr| self.expr_type(expr));
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
            ExprKind::Member { object, field } => self.member_type(object, field, expr.span),
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
            BinaryOp::Less | BinaryOp::LessEqual | BinaryOp::Greater | BinaryOp::GreaterEqual => {
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
                if !Self::compatible(&left_type, &right_type) {
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

        if matches!(op, AssignOp::Assign) {
            self.check_assignable(&target_type, &value_type, value.span);
        } else {
            self.require_type(&target_type, &Type::F32, target.span);
            self.require_type(&value_type, &Type::F32, value.span);
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
            ExprKind::Member { object, field } => self.member_type(object, field, target.span),
            _ => {
                self.error(target.span, "invalid assignment target".to_owned());
                Type::Unknown
            }
        }
    }

    /// The type of `object.field`, when the host has said what `object` is.
    ///
    /// Three outcomes, and which one applies is the whole design. A described
    /// type that has the member gives its type. A described type that does not
    /// is an error, which is the point of the exercise: a misspelled component
    /// field stops being a runtime failure at frame one. An *undescribed* type
    /// stays `Unknown`, so a host that has described half of itself does not
    /// reject scripts that were working against the other half.
    fn member_type(&mut self, object: &Expr, field: &str, span: Span) -> Type {
        let object_type = self.expr_type(object);
        match self.member_symbol(&object_type, field) {
            Some(MemberLookup::Found(ExternalSymbol::Value(ty))) => ty,
            // A function reached without calling it. Decay has no function
            // values, so there is nothing this could evaluate to.
            Some(MemberLookup::Found(ExternalSymbol::Function(_))) => {
                self.error(
                    span,
                    format!(
                        "`{}` is a function on `{}`, and Decay has no function values -- call it",
                        field,
                        object_type.display_name()
                    ),
                );
                Type::Unknown
            }
            Some(MemberLookup::Missing) => {
                self.error(
                    span,
                    format!("`{}` has no member `{field}`", object_type.display_name()),
                );
                Type::Unknown
            }
            Some(MemberLookup::ContainerFunction) => {
                self.error(span, container_function_message(field));
                Type::Unknown
            }
            None => Type::Unknown,
        }
    }

    /// Looks a member up on a type the host may or may not have described.
    ///
    /// `None` means "nothing is known about this type", which is not the same
    /// as "this type has no such member" and must not be reported as one.
    fn member_symbol(&self, object_type: &Type, field: &str) -> Option<MemberLookup> {
        let described = match object_type {
            Type::Named(name) if self.is_container(name) => {
                // `this` is two things: the script's own state, and the entity
                // the host attached it to. The script's own members are asked
                // first, so the engine growing a name can never shadow a
                // field a script already had.
                if let Some(symbol) = self.scopes.first().and_then(|scope| scope.get(field)) {
                    return Some(match &symbol.function {
                        // `this.helper()` lowers to a host path call named
                        // `this.helper`, which no host implements, so it failed
                        // at runtime with `FunctionNotFound` and looked like the
                        // engine's fault. Saying so here costs one diagnostic
                        // and removes the single most confusing thing about
                        // writing a Decay script.
                        Some(_) => MemberLookup::ContainerFunction,
                        None => MemberLookup::Found(ExternalSymbol::Value(symbol.ty.clone())),
                    });
                }
                let this = self.environment.this();
                if !this.is_described() {
                    return None;
                }
                this
            }
            Type::Named(name) => self.environment.get_type(name)?,
            _ => return None,
        };
        Some(match described.member(field) {
            Some(symbol) => MemberLookup::Found(symbol.clone()),
            None => MemberLookup::Missing,
        })
    }

    fn is_container(&self, name: &str) -> bool {
        self.containers.contains(name)
    }

    fn call_type(&mut self, callee: &Expr, args: &[Expr], span: Span) -> Type {
        // A method on a host type: `this.rigidbody.add_impulse(v)`. Resolved
        // before the general path so that the arguments are checked against a
        // real signature rather than accepted because the callee was unknown.
        if let ExprKind::Member { object, field } = &callee.kind {
            let object_type = self.expr_type(object);
            match self.member_symbol(&object_type, field) {
                Some(MemberLookup::Found(ExternalSymbol::Function(function))) => {
                    self.check_call(&function, args, span);
                    return function.return_type;
                }
                Some(MemberLookup::Found(ExternalSymbol::Value(_))) => {
                    self.error(
                        callee.span,
                        format!(
                            "`{field}` on `{}` is a value, not a function",
                            object_type.display_name()
                        ),
                    );
                }
                Some(MemberLookup::Missing) => {
                    self.error(
                        callee.span,
                        format!("`{}` has no member `{field}`", object_type.display_name()),
                    );
                }
                Some(MemberLookup::ContainerFunction) => {
                    self.error(callee.span, container_function_message(field));
                }
                None => {}
            }
            for arg in args {
                self.expr_type(arg);
            }
            return Type::Unknown;
        }
        self.call_named(callee, args, span)
    }

    fn call_named(&mut self, callee: &Expr, args: &[Expr], span: Span) -> Type {
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

    fn check_assignable(&mut self, expected: &Type, actual: &Type, span: Span) {
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

    fn compatible(expected: &Type, actual: &Type) -> bool {
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

/// Decay has no methods, and the mistake of reaching for one is worth naming
/// precisely rather than leaving as a runtime `FunctionNotFound`.
fn container_function_message(field: &str) -> String {
    format!(
        "`{field}` is this script's own function; call it as `{field}(...)` rather than `this.{field}(...)`"
    )
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
    use super::{
        DiagnosticPhase, Environment, FunctionType, HostType, Type, analyze,
        analyze_with_environment,
    };

    #[test]
    fn accepts_typed_gameplay_code() {
        let analysis = analyze(
            r"
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
            ",
        );

        assert!(
            analysis.diagnostics.is_empty(),
            "{:?}",
            analysis.diagnostics
        );
    }

    #[test]
    fn reports_unknown_names_and_type_mismatches() {
        let analysis = analyze(
            r"
            script Broken {
                fn update(dt: f32) {
                    var alive: bool = true;
                    alive = 1.0;
                    missing = dt;
                }
            }
            ",
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
            r"
            script Player {
                fn update() {
                    let speed: f32 = 6.0;
                    speed = 8.0;
                }
            }
            ",
        );

        assert!(analysis.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("cannot assign to immutable `speed`")
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

        assert!(
            analysis.diagnostics.is_empty(),
            "{:?}",
            analysis.diagnostics
        );
    }

    /// A host that has described a type gets its members checked, which is the
    /// whole point: a misspelled component field is a compile error rather than
    /// a runtime failure at frame one.
    #[test]
    fn a_described_type_checks_its_members() {
        let mut environment = Environment::new();
        environment.add_type(
            "Vec3",
            HostType::new()
                .with_value("x", Type::F32)
                .with_value("y", Type::F32)
                .with_value("z", Type::F32),
        );
        environment.add_type(
            "Transform",
            HostType::new().with_value("position", Type::Named("Vec3".to_owned())),
        );
        environment.add_this_value("transform", Type::Named("Transform".to_owned()));

        let good = analyze_with_environment(
            r"script Player { fn update(dt: f32) { this.transform.position.x += dt; } }",
            &environment,
        );
        assert!(good.diagnostics.is_empty(), "{:?}", good.diagnostics);

        let typo = analyze_with_environment(
            r"script Player { fn update(dt: f32) { this.transfrom.position.x += dt; } }",
            &environment,
        );
        assert!(
            typo.diagnostics
                .iter()
                .any(|d| d.message.contains("has no member `transfrom`")),
            "{:?}",
            typo.diagnostics
        );

        let deep_typo = analyze_with_environment(
            r"script Player { fn update(dt: f32) { this.transform.position.w += dt; } }",
            &environment,
        );
        assert!(
            deep_typo
                .diagnostics
                .iter()
                .any(|d| d.message.contains("`Vec3` has no member `w`")),
            "{:?}",
            deep_typo.diagnostics
        );
    }

    /// A member's type is a real type, so what is done with it is checked too.
    #[test]
    fn a_members_type_is_enforced_like_any_other() {
        let mut environment = Environment::new();
        environment.add_type("Sprite", HostType::new().with_value("visible", Type::Bool));
        environment.add_this_value("sprite", Type::Named("Sprite".to_owned()));

        let analysis = analyze_with_environment(
            r"script Player { fn update(dt: f32) { this.sprite.visible = dt; } }",
            &environment,
        );
        assert!(
            analysis
                .diagnostics
                .iter()
                .any(|d| d.message.contains("cannot assign `f32` to `bool`")),
            "{:?}",
            analysis.diagnostics
        );
    }

    /// Describing the host is gradual. A named type nobody has described keeps
    /// behaving exactly as everything did before types existed, so a host
    /// part-way through describing itself does not reject working scripts.
    #[test]
    fn an_undescribed_type_stays_permissive() {
        let mut environment = Environment::new();
        environment.add_this_value("rigidbody", Type::Named("RigidBody".to_owned()));

        let analysis = analyze_with_environment(
            r"script Player { fn update(dt: f32) { this.rigidbody.anything.at.all = dt; } }",
            &environment,
        );
        assert!(
            analysis.diagnostics.is_empty(),
            "{:?}",
            analysis.diagnostics
        );
    }

    /// A container's own field wins over anything the host attached, so the
    /// engine growing a name cannot shadow a script's state.
    #[test]
    fn a_container_field_wins_over_a_host_member() {
        let mut environment = Environment::new();
        environment.add_this_value("speed", Type::Named("Opaque".to_owned()));

        let analysis = analyze_with_environment(
            r"script Player { var speed: f32 = 1.0; fn update(dt: f32) { this.speed += dt; } }",
            &environment,
        );
        assert!(
            analysis.diagnostics.is_empty(),
            "{:?}",
            analysis.diagnostics
        );
    }

    /// Methods on host types are checked against a real signature rather than
    /// waved through because the callee was unknown.
    #[test]
    fn a_host_method_checks_its_arguments() {
        let mut environment = Environment::new();
        environment.add_type(
            "RigidBody",
            HostType::new().with_function(
                "add_impulse",
                FunctionType {
                    params: vec![Type::F32, Type::F32],
                    return_type: Type::Unit,
                },
            ),
        );
        environment.add_this_value("rigidbody", Type::Named("RigidBody".to_owned()));

        let good = analyze_with_environment(
            r"script Player { fn update() { this.rigidbody.add_impulse(0.0, 1.0); } }",
            &environment,
        );
        assert!(good.diagnostics.is_empty(), "{:?}", good.diagnostics);

        let wrong = analyze_with_environment(
            r"script Player { fn update() { this.rigidbody.add_impulse(0.0); } }",
            &environment,
        );
        assert!(
            wrong
                .diagnostics
                .iter()
                .any(|d| d.message.contains("expected 2 argument(s), found 1")),
            "{:?}",
            wrong.diagnostics
        );
    }

    /// Decay has no methods. `this.helper()` lowered to a host path call that no
    /// host implements, so it failed at runtime looking like the engine's fault;
    /// now it says what to write instead.
    #[test]
    fn reaching_for_a_method_says_what_to_write_instead() {
        let analysis = analyze(
            r"script Player { fn helper() -> f32 { return 1.0; } fn update() { this.helper(); } }",
        );
        assert!(
            analysis
                .diagnostics
                .iter()
                .any(|d| d.message.contains("call it as `helper(...)`")),
            "{:?}",
            analysis.diagnostics
        );

        let bare = analyze(
            r"script Player { fn helper() -> f32 { return 1.0; } fn update() { helper(); } }",
        );
        assert!(bare.diagnostics.is_empty(), "{:?}", bare.diagnostics);
    }

    #[test]
    fn catches_duplicate_members_and_locals() {
        let analysis = analyze(
            r"
            script Player {
                let speed: f32 = 1.0;
                let speed: f32 = 2.0;

                fn update() {
                    let value: f32 = 1.0;
                    let value: f32 = 2.0;
                }
            }
            ",
        );

        assert!(
            analysis
                .diagnostics
                .iter()
                .any(|diagnostic| { diagnostic.message.contains("duplicate member `speed`") })
        );
        assert!(
            analysis
                .diagnostics
                .iter()
                .any(|diagnostic| { diagnostic.message.contains("duplicate local `value`") })
        );
    }
}
