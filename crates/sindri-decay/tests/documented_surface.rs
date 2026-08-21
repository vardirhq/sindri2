//! Does `docs/scripting.md` describe the surface a script actually has?
//!
//! The document lists, in tables, every path and call a script can reach. That
//! list is believed — it is the only place an author looks to find out what
//! they can write — and a believed list that is quietly wrong is worse than no
//! list at all. `docs/capabilities.md` exists in this repository for exactly
//! that reason.
//!
//! So the tables are parsed and compared against what the host actually
//! describes. A surface that grows without the documentation growing with it
//! fails here, and so does a document that promises something withdrawn.
//!
//! When this fails, the fix is in the document, not in the assertion.

use std::collections::BTreeSet;

use decay_semantic::{Environment, ExternalSymbol, Type};
use sindri_decay::environment;

const DOC: &str = include_str!("../../../docs/scripting.md");

/// The first backticked cell of every table row in the document.
///
/// Tables are the only place the surface is listed, and a row always leads with
/// the thing it describes, so this finds the claims without needing to know
/// which table it is reading.
fn documented_cells() -> Vec<String> {
    DOC.lines()
        .filter_map(|line| {
            let line = line.trim();
            let rest = line.strip_prefix("| `")?;
            let (cell, _) = rest.split_once('`')?;
            Some(cell.to_owned())
        })
        .collect()
}

/// Expands `position.{x,y,z}` into three names.
///
/// The document groups axes because listing nine near-identical rows would bury
/// the two that differ. Expanding here means the shorthand costs nothing.
fn expand(name: &str) -> Vec<String> {
    let Some((head, rest)) = name.split_once('{') else {
        return vec![name.to_owned()];
    };
    let Some((choices, tail)) = rest.split_once('}') else {
        return vec![name.to_owned()];
    };
    choices
        .split(',')
        .map(|choice| format!("{head}{}{tail}", choice.trim()))
        .collect()
}

/// Everything the document claims, split by what it is.
struct Documented {
    entity_paths: BTreeSet<String>,
    input_calls: BTreeSet<String>,
    time_values: BTreeSet<String>,
}

fn documented() -> Documented {
    let mut documented = Documented {
        entity_paths: BTreeSet::new(),
        input_calls: BTreeSet::new(),
        time_values: BTreeSet::new(),
    };
    for cell in documented_cells() {
        for name in expand(&cell) {
            // A call is written with its arguments; the name is what precedes
            // them.
            let name = name
                .split_once('(')
                .map_or(name.clone(), |(head, _)| head.to_owned());
            if let Some(rest) = name.strip_prefix("Input.") {
                documented.input_calls.insert(rest.to_owned());
            } else if let Some(rest) = name.strip_prefix("Time.") {
                documented.time_values.insert(rest.to_owned());
            } else if name.starts_with("this.") {
                documented.entity_paths.insert(name);
            }
        }
    }
    documented
}

/// Every complete path under `this`, as the analyzer describes them.
fn described_entity_paths(environment: &Environment) -> BTreeSet<String> {
    fn walk(
        environment: &Environment,
        prefix: String,
        symbol: &ExternalSymbol,
        into: &mut BTreeSet<String>,
    ) {
        let ExternalSymbol::Value(ty) = symbol else {
            return;
        };
        let Type::Named(name) = ty else {
            into.insert(prefix);
            return;
        };
        let described = environment
            .get_type(name)
            .unwrap_or_else(|| panic!("`{name}` is named by the surface but never described"));
        for (field, member) in described.members() {
            walk(environment, format!("{prefix}.{field}"), member, into);
        }
    }

    let mut paths = BTreeSet::new();
    for (member, symbol) in environment.this().members() {
        walk(environment, format!("this.{member}"), symbol, &mut paths);
    }
    paths
}

fn described_members(environment: &Environment, namespace: &str) -> BTreeSet<String> {
    environment
        .get_type(namespace)
        .unwrap_or_else(|| panic!("`{namespace}` is not described"))
        .members()
        .map(|(name, _)| name.to_owned())
        .collect()
}

#[test]
fn the_document_lists_exactly_the_paths_a_script_can_reach() {
    let environment = environment();
    let documented = documented();

    assert_eq!(
        documented.entity_paths,
        described_entity_paths(&environment),
        "docs/scripting.md and the host surface disagree about what a script \
         can reach on its entity. The document is the thing to fix."
    );
}

#[test]
fn the_document_lists_exactly_the_keyboard_calls_a_script_can_make() {
    let environment = environment();
    assert_eq!(
        documented().input_calls,
        described_members(&environment, "Input"),
        "docs/scripting.md and the host surface disagree about the keyboard"
    );
}

#[test]
fn the_document_lists_exactly_what_a_script_can_ask_about_the_frame() {
    let environment = environment();
    assert_eq!(
        documented().time_values,
        described_members(&environment, "Time"),
        "docs/scripting.md and the host surface disagree about time"
    );
}

/// The maths functions are prose rather than a table, so they are read from the
/// sentence that lists them.
#[test]
fn the_document_lists_exactly_the_maths_a_script_can_do() {
    let sentence = DOC
        .split_once("That is the entire standard library")
        .expect("the document still says what the standard library is")
        .0;
    let listed: BTreeSet<String> = sentence
        .rsplit("### Maths")
        .next()
        .expect("the maths section")
        .split('`')
        .skip(1)
        .step_by(2)
        .map(str::to_owned)
        .collect();

    let described: BTreeSet<String> = environment()
        .globals()
        .filter(|(name, symbol)| matches!(symbol, ExternalSymbol::Function(_)) && *name != "print")
        .map(|(name, _)| name.to_owned())
        .collect();

    assert_eq!(
        listed, described,
        "docs/scripting.md and the host disagree about the standard library"
    );
}
