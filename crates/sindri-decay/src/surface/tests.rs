//! The one test that keeps the two readers of this surface in step.
//!
//! Walking the description and asserting the host answers every path in it
//! is what stops the analyzer and the host being shipped disagreeing.

use decay_ir::Path;
use decay_runtime::{Host, Value};
use decay_semantic::{Environment, ExternalSymbol, Type};
use sindri_core::{EntityData, ProfileDocument, Transform3D, World};
use sindri_platform::InputState;

use crate::surface::names;
use crate::{
    PrefabSources, ScriptContext, Spawning, WorldHost, environment,
    surface::{ENTITY, PREFAB, PROFILE},
};

/// The prefab this module's calls spawn, and the document behind it.
///
/// A real prefab rather than an absent one, because the assertion below is that
/// the host *performs* every described call — and a spawn of a prefab nobody
/// loaded is refused, which would prove nothing about whether the host knows
/// the path.
const SPARE_PREFAB: &str = "prefabs/spare.prefab.json";
const SPARE_PROFILE: &str = "profiles/spare.profile.json";

fn prefabs() -> PrefabSources {
    let mut prefabs = PrefabSources::new();
    prefabs.insert(
        SPARE_PREFAB,
        sindri_core::PrefabDocument::single(sindri_core::SceneEntity::new(
            sindri_core::SceneEntityId::new("spare").expect("a literal identity"),
        )),
    );
    prefabs
}

fn profiles() -> crate::ProfileSources {
    let mut profiles = crate::ProfileSources::new();
    let mut profile = ProfileDocument {
        name: "Spare".to_owned(),
        profile_type: "surface_test".to_owned(),
        ..ProfileDocument::default()
    };
    profile
        .values
        .insert("Space".to_owned(), serde_json::json!([{"Space": 1.0}]));
    profiles.insert(SPARE_PROFILE, profile);
    profiles
}

/// The entities one described call is exercised against.
///
/// A fresh set per call, because one of these calls removes whatever it is
/// given and the rest still need a world. The spare carries a script component
/// because one described call authors a property on one and an entity with no
/// script is refused; the other three exist because the grid namespace needs
/// three distinct roles — who moves, which grid, where to.
fn subjects(world: &mut World, nth: usize) -> [sindri_core::EntityId; 4] {
    let spare = world.spawn(EntityData {
        transform_3d: Some(Transform3D::default()),
        // The spare carries a screen element as well as a script, because the
        // `Ui.*` calls act on one and a host that refused for want of a
        // component would look like a host that does not know the call.
        components: [
            (
                "sindri.script".to_owned(),
                serde_json::json!({ "source": "scripts/spare.decay", "script": "Spare" }),
            ),
            (
                "sindri.ui.text".to_owned(),
                serde_json::json!({ "text": "{}", "font": "font.ttf" }),
            ),
        ]
        .into(),
        ..EntityData::default()
    });
    let mover = world.spawn(EntityData {
        transform_3d: Some(Transform3D::default()),
        ..EntityData::default()
    });
    let grid = world.spawn(EntityData {
        transform_3d: Some(Transform3D::default()),
        components: [(
            "sindri.grid".to_owned(),
            serde_json::json!({ "columns": 2, "rows": 2, "cell_size": [1.0, 1.0] }),
        )]
        .into(),
        ..EntityData::default()
    });
    let target = world.spawn(EntityData {
        transform_3d: Some(Transform3D {
            position: [1.0 + nth as f32 * 0.01, 1.0, 0.0],
            ..Transform3D::default()
        }),
        ..EntityData::default()
    });
    [spare, mover, grid, target]
}

fn value_for(ty: &Type, subjects: [sindri_core::EntityId; 4]) -> Value {
    match ty {
        Type::Bool => Value::Bool(false),
        Type::F32 => Value::F32(1.0),
        Type::String => Value::String("Space".to_owned()),
        Type::Entity => Value::Entity(subjects[0]),
        Type::Prefab => Value::Prefab(SPARE_PREFAB.to_owned()),
        Type::Profile => Value::Profile(SPARE_PROFILE.to_owned()),
        Type::Vec2 => Value::Vec2([1.0, 1.0]),
        Type::Vec3 => Value::Vec3([1.0, 1.0, 1.0]),
        Type::Color => Value::Color([1.0, 1.0, 1.0, 1.0]),
        Type::EntityCollection => Value::EntityCollection(subjects.to_vec()),
        other => panic!("the surface test has no sample for {other:?}"),
    }
}

fn arguments(parameters: &[Type], subjects: [sindri_core::EntityId; 4]) -> Vec<Value> {
    let mut entities = 0;
    parameters
        .iter()
        .map(|ty| {
            if *ty == Type::Entity {
                let value = Value::Entity(subjects[entities.min(subjects.len() - 1)]);
                entities += 1;
                value
            } else {
                value_for(ty, subjects)
            }
        })
        .collect()
}

#[test]
fn every_described_call_is_implemented_by_the_host() {
    let environment = environment();
    let prefabs = prefabs();
    let profiles = profiles();
    let mut world = World::default();
    let input = InputState::default();
    let mut blackboard = crate::Blackboard::default();
    let mut spawning = Spawning::default();
    let mut audio = crate::SilentAudio::default();

    let calls = environment
        .symbols()
        .filter_map(|(path, symbol)| match symbol {
            ExternalSymbol::Function(signature) => Some((path.clone(), signature.clone())),
            _ => None,
        })
        .collect::<Vec<_>>();

    for (nth, (path, signature)) in calls.into_iter().enumerate() {
        let subjects = subjects(&mut world, nth);
        let mut host = WorldHost::new(
            &mut world,
            ScriptContext::new(&input, &mut blackboard, 1.0 / 60.0)
                .with_prefabs(&prefabs)
                .with_profiles(&profiles)
                .with_spawning(&mut spawning)
                .with_audio(&mut audio),
        );
        let args = arguments(&signature.parameters, subjects);
        host.call(&path, &args).unwrap_or_else(|error| {
            panic!("described surface call {path} was not executable: {error}")
        });
    }
}

#[test]
fn asset_types_are_described_as_asset_types() {
    let environment = environment();
    assert_eq!(environment.symbol(&Path::new(PREFAB)), Some(&ExternalSymbol::Type(Type::Prefab)));
    assert_eq!(environment.symbol(&Path::new(PROFILE)), Some(&ExternalSymbol::Type(Type::Profile)));
    assert_eq!(environment.symbol(&Path::new(ENTITY)), Some(&ExternalSymbol::Type(Type::Entity)));
}

#[test]
fn the_namespace_names_are_stable() {
    assert_eq!(names::WORLD, "World");
    assert_eq!(names::PROFILES, "Profiles");
}
