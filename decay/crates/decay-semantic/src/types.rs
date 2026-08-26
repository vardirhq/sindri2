//! What a Decay value can be, and what a host says about its own types.

use std::collections::HashMap;

use decay_syntax::TypeRef;

use crate::environment::ExternalSymbol;

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
