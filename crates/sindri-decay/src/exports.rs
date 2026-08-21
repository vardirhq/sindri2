//! What a script declares it wants authored.
//!
//! This is the capability that justified Decay being statically typed rather
//! than embedding a dynamic language, and until now nothing used it. A property
//! panel needs a **declared, named, typed** field it can draw a widget for
//! without executing anything — and `@export let speed: f32 = 6.0;` is exactly
//! that, sitting in the IR since the language's first commit.
//!
//! The default is the one thing that does require executing something: an
//! initializer is instructions, not a value, so it is evaluated the same way a
//! real instance evaluates it. Doing anything else here would mean a second
//! answer to "what does this field start as".

use decay_ir::IrProgram;
use decay_runtime::{EmptyHost, Runtime, Value};

/// One `@export` field of a script, as an authoring surface sees it.
#[derive(Clone, Debug, PartialEq)]
pub struct ScriptExport {
    pub name: String,
    /// The type the script declared, if it declared one.
    ///
    /// `None` where a field was written without a type annotation. A panel
    /// draws such a field from its default's shape instead, which is the best
    /// anyone can do and is why annotating is worth encouraging.
    pub type_name: Option<String>,
    /// What the field starts as when the scene says nothing.
    pub default: Value,
}

/// Every `@export` field of one container, in declaration order.
///
/// Declaration order rather than sorted: the author chose it, and a panel that
/// reorders someone's fields alphabetically is a panel that fights them.
pub(crate) fn exports_of(program: &IrProgram, script: &str) -> Option<Vec<ScriptExport>> {
    let container = program
        .containers
        .iter()
        .find(|container| container.name == script)?;

    // Instantiated with a host that answers nothing, because an initializer
    // that reached the world would be asking a question the panel has no
    // entity to answer. Such a field simply has no default here, and the panel
    // says so rather than inventing one.
    let mut runtime = Runtime::new(program, EmptyHost);
    let instance = runtime.instantiate(script).ok();

    Some(
        container
            .fields
            .iter()
            .filter(|field| field.exported)
            .map(|field| ScriptExport {
                name: field.name.clone(),
                type_name: field.type_name.clone(),
                default: instance
                    .as_ref()
                    .and_then(|instance| instance.field(&field.name))
                    .cloned()
                    .unwrap_or(Value::Null),
            })
            .collect(),
    )
}
