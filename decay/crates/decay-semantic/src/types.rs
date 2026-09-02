//! What a Decay value can be, and what a host says about its own types.

use std::borrow::Cow;
use std::collections::HashMap;

use decay_syntax::TypeRef;

/// How a collection type is spelled in Decay source.
pub const ARRAY: &str = "Array";

/// The member a collection offers.
///
/// A property rather than a `len(x)` global, because Decay has no modules and
/// every global name added is one a script can no longer use for its own. A
/// length is a property of the value, and spelling it as one costs nobody a
/// name.
pub const LENGTH: &str = "len";

use crate::environment::ExternalSymbol;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Type {
    F32,
    Bool,
    String,
    Unit,
    Null,
    Named(String),
    /// A fixed-length collection of one element type.
    ///
    /// The only generic type the language has, and it is not user-definable:
    /// the host hands one back, a script reads it, and nothing constructs,
    /// grows, or shrinks one. That is what keeps a collection bounded without
    /// the language having to say anything about memory.
    Array(Box<Type>),
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
            // `Array` written without an argument is `Array<unknown>` rather
            // than a diagnostic. The analyzer reports the missing argument
            // where the type was written; treating it as unknown here keeps
            // one mistake from cascading into every use of the binding.
            ARRAY => Self::Array(Box::new(
                reference
                    .argument
                    .as_ref()
                    .map_or(Self::Unknown, |argument| Self::from_ref(argument)),
            )),
            other => Self::Named(other.to_owned()),
        }
    }

    /// The element type, for a type that holds several of something.
    #[must_use]
    pub fn element(&self) -> Option<&Self> {
        match self {
            Self::Array(element) => Some(element),
            _ => None,
        }
    }

    /// A collection of `element`.
    #[must_use]
    pub fn array_of(element: Self) -> Self {
        Self::Array(Box::new(element))
    }

    /// How this type is written in Decay source, and in a diagnostic about it.
    ///
    /// Public because a host describing itself — in an error, or in a manifest
    /// a tool reads — needs to name a type the same way the compiler does.
    #[must_use]
    pub fn display_name(&self) -> Cow<'_, str> {
        match self {
            Self::F32 => Cow::Borrowed("f32"),
            Self::Bool => Cow::Borrowed("bool"),
            Self::String => Cow::Borrowed("String"),
            Self::Unit => Cow::Borrowed("unit"),
            Self::Null => Cow::Borrowed("null"),
            Self::Named(name) => Cow::Borrowed(name),
            Self::Array(element) => Cow::Owned(format!("{ARRAY}<{}>", element.display_name())),
            Self::Unknown => Cow::Borrowed("unknown"),
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
