//! The Decay host surface, as a description something else can read.
//!
//! Read from [`sindri_decay::environment`], which is the same description the
//! analyzer type-checks scripts against and the same one the runtime host
//! answers. So a call that appears here is a call that exists, and one that
//! exists appears here — there is no third list to forget to update.
//!
//! What the environment does *not* carry is parameter names: a host function
//! is registered as its parameter types and its return type, because that is
//! all type-checking needs. Names for the arguments live in the prose of
//! `docs/scripting.md`, and inventing them here would be a second source of
//! truth for something this crate cannot actually know.

use decay_semantic::{Environment, ExternalSymbol, FunctionType, HostType, Type};
use serde_json::{Value, json};

/// Everything a Decay script may name.
pub(crate) struct DecayApi {
    /// Names in scope without qualification: `sin`, `print`, and each namespace
    /// value such as `Input`.
    pub(crate) globals: Vec<Symbol>,
    /// What `this` offers beyond the script's own fields.
    pub(crate) this: Vec<Symbol>,
    /// Every described host type, such as `World` or `Transform`.
    pub(crate) types: Vec<Namespace>,
}

/// A named host type and what it offers.
pub(crate) struct Namespace {
    pub(crate) name: String,
    pub(crate) members: Vec<Symbol>,
}

/// One thing a script can name: a value it reads, or a function it calls.
pub(crate) enum Symbol {
    Value {
        name: String,
        type_name: String,
    },
    Function {
        name: String,
        params: Vec<String>,
        returns: String,
    },
}

impl Symbol {
    pub(crate) fn name(&self) -> &str {
        match self {
            Self::Value { name, .. } | Self::Function { name, .. } => name,
        }
    }

    fn to_json(&self) -> Value {
        match self {
            Self::Value { name, type_name } => json!({
                "name": name,
                "kind": "value",
                "type": type_name,
            }),
            Self::Function {
                name,
                params,
                returns,
            } => json!({
                "name": name,
                "kind": "function",
                "parameters": params,
                "returns": returns,
            }),
        }
    }
}

impl DecayApi {
    pub(crate) fn to_json(&self) -> Value {
        json!({
            "schema_version": crate::SCHEMA_VERSION,
            "engine_version": env!("CARGO_PKG_VERSION"),
            "generated_by": crate::REGENERATE_COMMAND,
            "about": "Every namespace, call, and member a Decay script may name, \
        derived from the host surface the analyzer and the runtime share. Parameter \
        names are not recorded; see docs/scripting.md for what each argument means.",
            "globals": symbols_json(&self.globals),
            "this": symbols_json(&self.this),
            "types": self
                .types
                .iter()
                .map(|namespace| json!({
                    "name": namespace.name,
                    "members": symbols_json(&namespace.members),
                }))
                .collect::<Vec<_>>(),
        })
    }
}

fn symbols_json(symbols: &[Symbol]) -> Vec<Value> {
    symbols.iter().map(Symbol::to_json).collect()
}

/// Describes the host surface this engine build actually offers.
pub(crate) fn describe() -> DecayApi {
    let environment = sindri_decay::environment();

    DecayApi {
        globals: sorted(
            environment
                .globals()
                .map(|(name, symbol)| symbol_of(name, symbol)),
        ),
        this: members_of(environment.this()),
        types: sorted_types(&environment),
    }
}

fn sorted_types(environment: &Environment) -> Vec<Namespace> {
    let mut types: Vec<Namespace> = environment
        .types()
        .map(|(name, host_type)| Namespace {
            name: name.to_owned(),
            members: members_of(host_type),
        })
        .collect();
    types.sort_by(|left, right| left.name.cmp(&right.name));
    types
}

fn members_of(host_type: &HostType) -> Vec<Symbol> {
    sorted(
        host_type
            .members()
            .map(|(name, symbol)| symbol_of(name, symbol)),
    )
}

/// Sorted by name, because the environment holds these in hash maps and a
/// generated file that reorders itself between runs is a file nobody can diff.
fn sorted(symbols: impl Iterator<Item = Symbol>) -> Vec<Symbol> {
    let mut symbols: Vec<Symbol> = symbols.collect();
    symbols.sort_by(|left, right| left.name().cmp(right.name()));
    symbols
}

fn symbol_of(name: &str, symbol: &ExternalSymbol) -> Symbol {
    match symbol {
        ExternalSymbol::Value(ty) => Symbol::Value {
            name: name.to_owned(),
            type_name: type_name(ty),
        },
        ExternalSymbol::Function(FunctionType {
            params,
            return_type,
        }) => Symbol::Function {
            name: name.to_owned(),
            params: params.iter().map(type_name).collect(),
            returns: type_name(return_type),
        },
    }
}

/// How a type is written in Decay source, so a reader can copy it into a script.
fn type_name(ty: &Type) -> String {
    match ty {
        Type::F32 => "f32".to_owned(),
        Type::Bool => "bool".to_owned(),
        Type::String => "String".to_owned(),
        Type::Unit => "unit".to_owned(),
        Type::Null => "null".to_owned(),
        Type::Named(name) => name.clone(),
        Type::Array(element) => format!("Array<{}>", type_name(element)),
        Type::Unknown => "unknown".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::{Symbol, describe, type_name};
    use decay_semantic::Type;

    #[test]
    fn the_description_carries_a_call_the_host_answers() {
        let api = describe();
        let world = api
            .types
            .iter()
            .find(|namespace| namespace.name == "World")
            .expect("the world namespace is described");

        let find = world
            .members
            .iter()
            .find(|member| member.name() == "find")
            .expect("World.find is on the surface");

        match find {
            Symbol::Function { params, .. } => {
                assert_eq!(params, &["String".to_owned()], "find takes one name");
            }
            Symbol::Value { .. } => panic!("World.find is a call, not a value"),
        }
    }

    #[test]
    fn members_are_sorted_so_the_file_does_not_churn() {
        let api = describe();
        for namespace in &api.types {
            let mut names: Vec<&str> = namespace.members.iter().map(Symbol::name).collect();
            let sorted = {
                let mut copy = names.clone();
                copy.sort_unstable();
                copy
            };
            names.dedup();
            assert_eq!(namespace.members.len(), names.len(), "no duplicate members");
            assert_eq!(
                namespace
                    .members
                    .iter()
                    .map(Symbol::name)
                    .collect::<Vec<_>>(),
                sorted,
                "{} members are sorted",
                namespace.name
            );
        }
    }

    #[test]
    fn an_array_type_reads_as_decay_writes_it() {
        assert_eq!(
            type_name(&Type::Array(Box::new(Type::Named("Entity".to_owned())))),
            "Array<Entity>"
        );
    }
}
