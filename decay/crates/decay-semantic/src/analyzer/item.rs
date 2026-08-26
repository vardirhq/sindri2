//! Containers and functions: their members, fields, and bodies.

use std::collections::HashMap;

use decay_syntax::{FieldDecl, FunctionDecl, Member, Span};

use crate::types::{FunctionType, Type};

use super::{Analyzer, Symbol};

impl Analyzer<'_, '_> {
    pub(super) fn analyze_container(&mut self, container: &decay_syntax::ContainerDecl) {
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

    pub(super) fn insert_member(
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

    pub(super) fn check_field_initializer(
        &mut self,
        field: &FieldDecl,
        members: &HashMap<String, Symbol>,
    ) {
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

    pub(super) fn analyze_function(
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

    pub(super) fn is_container(&self, name: &str) -> bool {
        self.containers.contains(name)
    }
}
