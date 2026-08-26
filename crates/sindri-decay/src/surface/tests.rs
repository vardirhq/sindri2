//! The one test that keeps the two readers of this surface in step.
//!
//! Walking the description and asserting the host answers every path in it
//! is what stops the analyzer and the host being shipped disagreeing.

use decay_ir::Path;
use decay_runtime::{Host, Value};
use decay_semantic::{Environment, ExternalSymbol, Type};
use sindri_core::{EntityData, Transform3D, World};
use sindri_platform::InputState;

use crate::surface::names;
use crate::{ScriptContext, WorldHost, environment, surface::ENTITY};

fn blackboard() -> crate::Blackboard {
    crate::Blackboard::new()
}

fn context(input: &InputState) -> ScriptContext<'_> {
    ScriptContext {
        input,
        delta_seconds: 0.5,
        elapsed_seconds: 1.5,
    }
}

/// An entity carrying every component the surface can reach into, so a path
/// that needs one is not refused for the entity's sake.
fn world() -> (World, sindri_core::EntityId) {
    let mut world = World::default();
    let entity = world.spawn(EntityData {
        transform_3d: Some(Transform3D::default()),
        components: [(
            names::SPRITE_COMPONENT.to_owned(),
            serde_json::json!({
                "texture": "procedural:checkerboard",
                "tint": [1.0, 1.0, 1.0, 1.0],
                "layer": 0
            }),
        )]
        .into_iter()
        .collect(),
        ..EntityData::default()
    });
    (world, entity)
}

/// Every path the analyzer will accept, the host answers.
///
/// Walked out of the *environment* rather than the tree, and that direction
/// is the point. A path the host can reach but nothing describes is merely
/// unreachable; a path the analyzer accepts and the host cannot answer is a
/// clean compile followed by `UnknownPath` at frame one, which is the
/// failure this module exists to make impossible.
#[test]
fn the_host_answers_every_path_the_analyzer_accepts() {
    let (mut world, entity) = world();
    let input = InputState::default();
    let mut board = blackboard();
    let mut audio = Vec::new();
    let environment = environment();
    let mut checked = 0;

    for (member, symbol) in environment.this().members() {
        for (parts, terminal) in walk(
            &environment,
            vec!["this".to_owned(), member.to_owned()],
            symbol,
        ) {
            let path = Path(parts);
            let dotted = path.dotted();
            assert_eq!(
                terminal,
                Type::F32,
                "{dotted} ends in `{}`, and the host only carries numbers here",
                terminal.display_name()
            );

            let mut host =
                WorldHost::new(&mut world, entity, context(&input), &mut board, &mut audio);
            assert!(
                matches!(host.load(None, &path), Ok(Some(Value::Number(_)))),
                "the analyzer accepts {dotted} and the host cannot read it"
            );
            let mut host =
                WorldHost::new(&mut world, entity, context(&input), &mut board, &mut audio);
            assert_eq!(
                host.store(None, &path, Value::Number(1.0)),
                Ok(true),
                "the analyzer accepts {dotted} and the host cannot write it"
            );
            checked += 1;
        }
    }
    assert!(checked > 0, "the surface describes nothing at all");
}

/// The namespaced globals -- `Input` and `Time` -- are described and reached
/// by a different mechanism from `this`, so they get the same treatment
/// rather than being trusted.
#[test]
fn the_host_answers_every_global_the_analyzer_describes() {
    let (mut world, entity) = world();
    let input = InputState::default();
    let mut board = blackboard();
    let mut audio = Vec::new();
    let environment = environment();
    let mut checked = 0;

    for (namespace, symbol) in environment.globals() {
        let ExternalSymbol::Value(Type::Named(type_name)) = symbol else {
            continue;
        };
        let described = environment
            .get_type(type_name)
            .unwrap_or_else(|| panic!("`{type_name}` is offered but never described"));

        for (name, member) in described.members() {
            let path = Path(vec![namespace.to_owned(), name.to_owned()]);
            let dotted = path.dotted();
            // A spare entity per call, because one of these calls removes
            // whatever it is given and the rest still need a world.
            let spare = world.spawn(EntityData {
                transform_3d: Some(Transform3D::default()),
                ..EntityData::default()
            });
            let grid_name = format!("surface-grid-{checked}");
            let grid = world.spawn(EntityData {
                source_id: Some(
                    sindri_core::SceneEntityId::new(grid_name.clone()).expect("stable test id"),
                ),
                transform_3d: Some(Transform3D::default()),
                components: [
                    (
                        super::TILEMAP_COMPONENT.to_owned(),
                        serde_json::json!({
                            "columns": 2,
                            "rows": 1,
                            "space": "world",
                            "texture": "tiles.png",
                            "palette": ["tile"],
                            "tiles": [0, 0]
                        }),
                    ),
                    (
                        "sindri.grid.navigation".to_owned(),
                        serde_json::json!({ "walls": [] }),
                    ),
                ]
                .into_iter()
                .collect(),
                ..EntityData::default()
            });
            let mover = world.spawn(EntityData {
                transform_3d: Some(Transform3D {
                    position: [0.5, -0.5, 0.0],
                    ..Transform3D::default()
                }),
                components: [(
                    "sindri.grid.occupant".to_owned(),
                    serde_json::json!({
                        "grid": grid_name,
                        "footprint": [[0, 0]]
                    }),
                )]
                .into_iter()
                .collect(),
                ..EntityData::default()
            });
            let target = world.spawn(EntityData {
                transform_3d: Some(Transform3D {
                    position: [1.5, -0.5, 0.0],
                    ..Transform3D::default()
                }),
                ..EntityData::default()
            });
            let mut host =
                WorldHost::new(&mut world, entity, context(&input), &mut board, &mut audio);
            match member {
                ExternalSymbol::Value(_) => assert!(
                    matches!(host.load(None, &path), Ok(Some(_))),
                    "the analyzer describes {dotted} and the host cannot read it"
                ),
                ExternalSymbol::Function(signature) => {
                    let args =
                        arguments_for(&signature.params, namespace, [mover, grid, target], spare);
                    assert!(
                        matches!(host.call(None, &path, &args), Ok(Some(_))),
                        "the analyzer describes {dotted} and the host does not perform it"
                    );
                }
            }
            checked += 1;
        }
    }
    assert!(checked > 0, "no namespaced globals are described");
}

/// One call's arguments, built from its declared parameter types.
///
/// Built rather than assumed, so a namespace whose calls take something
/// other than key names is still exercised properly. Grid calls need three
/// distinct semantic roles — who moves, which grid, where to — while a
/// generic entity is enough everywhere else.
fn arguments_for(
    params: &[Type],
    namespace: &str,
    grid_roles: [sindri_core::EntityId; 3],
    spare: sindri_core::EntityId,
) -> Vec<Value> {
    params
        .iter()
        .enumerate()
        .map(|(index, ty)| match ty {
            Type::String => Value::String("Space".to_owned()),
            Type::Bool => Value::Bool(true),
            Type::Named(named) if named == ENTITY && namespace == super::GRID => {
                Value::Reference(grid_roles[index.min(2)].to_bits())
            }
            Type::Named(named) if named == ENTITY => Value::Reference(spare.to_bits()),
            _ => Value::Number(1.0),
        })
        .collect()
}

/// Expands a described member into every complete path under it, with the
/// type each one ends in.
fn walk(
    environment: &Environment,
    parts: Vec<String>,
    symbol: &ExternalSymbol,
) -> Vec<(Vec<String>, Type)> {
    let ExternalSymbol::Value(ty) = symbol else {
        return Vec::new();
    };
    let Type::Named(name) = ty else {
        return vec![(parts, ty.clone())];
    };
    let described = environment
        .get_type(name)
        .unwrap_or_else(|| panic!("`{name}` is named by the surface but never described"));
    described
        .members()
        .flat_map(|(field, symbol)| {
            let mut parts = parts.clone();
            parts.push(field.to_owned());
            walk(environment, parts, symbol)
        })
        .collect()
}

/// Every bare function the analyzer offers is one the host performs.
#[test]
fn every_described_function_is_one_the_host_performs() {
    let (mut world, entity) = world();
    let input = InputState::default();
    let mut board = blackboard();
    let mut audio = Vec::new();
    let environment = environment();

    for (name, symbol) in environment.globals() {
        let ExternalSymbol::Function(signature) = symbol else {
            continue;
        };
        let args = vec![Value::Number(1.0); signature.params.len()];
        let mut host = WorldHost::new(&mut world, entity, context(&input), &mut board, &mut audio);
        assert!(
            matches!(
                host.call(None, &Path(vec![name.to_owned()]), &args),
                Ok(Some(_))
            ),
            "the host does not perform `{name}`, which the environment offers"
        );
    }
}
