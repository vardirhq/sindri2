//! Reaching into a value: a container's field, or a host type's member.

use decay_syntax::{Expr, Span};

use crate::diagnostic::container_function_message;
use crate::environment::ExternalSymbol;
use crate::types::Type;

use super::{Analyzer, MemberLookup};

impl Analyzer<'_, '_> {
    /// The type of `object.field`, when the host has said what `object` is.
    ///
    /// Three outcomes, and which one applies is the whole design. A described
    /// type that has the member gives its type. A described type that does not
    /// is an error, which is the point of the exercise: a misspelled component
    /// field stops being a runtime failure at frame one. An *undescribed* type
    /// stays `Unknown`, so a host that has described half of itself does not
    /// reject scripts that were working against the other half.
    pub(super) fn member_type(&mut self, object: &Expr, field: &str, span: Span) -> Type {
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
    pub(super) fn member_symbol(&self, object_type: &Type, field: &str) -> Option<MemberLookup> {
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
}
