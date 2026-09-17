use std::path::Path;

use serde_json::json;
use sindri_core::{
    CommandBuffer, CommandHistory, EntityData, PREFAB_FORMAT_VERSION, PrefabDocument, SceneEntity,
    SceneEntityId, World,
};

use super::{instantiate_into, load};

fn id(value: &str) -> SceneEntityId {
    SceneEntityId::new(value).expect("a valid ID")
}

/// A tree with a trunk under it, so parenting has something to get wrong.
fn oak() -> PrefabDocument {
    let mut root = SceneEntity::new(id("oak"));
    root.name = Some("Oak".to_owned());
    root.components
        .insert("sindri.sprite".to_owned(), json!({ "texture": "oak.png" }));

    let mut trunk = SceneEntity::new(id("oak-trunk"));
    trunk.name = Some("Trunk".to_owned());
    trunk.parent = Some(id("oak"));

    PrefabDocument {
        format_version: PREFAB_FORMAT_VERSION,
        metadata: sindri_core::SceneMetadata::default(),
        entities: vec![root, trunk],
    }
}

/// Applies an instantiation the way the editor does, through the history.
fn instantiate(world: &mut World, prefab: &PrefabDocument) -> sindri_core::EntityId {
    let mut rehearsal = world.clone();
    let mut buffer = CommandBuffer::new();
    let root = instantiate_into(&mut rehearsal, prefab, None, &mut buffer)
        .expect("a single-rooted prefab instantiates");
    let mut history = CommandHistory::default();
    history
        .apply(buffer.into_transaction("Add Oak"), world)
        .expect("the spawns apply");
    root
}

#[test]
fn a_prefab_arrives_with_its_children_parented() {
    let mut world = World::default();
    let root = instantiate(&mut world, &oak());

    let data = world.get(root).expect("the root exists");
    assert_eq!(data.name.as_deref(), Some("Oak"));
    assert_eq!(data.children.len(), 1, "the trunk is under the oak");
    assert!(
        data.components.contains_key("sindri.sprite"),
        "components come with it"
    );
    let trunk = world.get(data.children[0]).expect("the trunk exists");
    assert_eq!(trunk.name.as_deref(), Some("Trunk"));
    assert_eq!(trunk.parent, Some(root));
}

/// Every entity a scene saves needs an identity, which is the whole reason
/// this is not the runtime's spawn.
#[test]
fn every_instantiated_entity_has_a_stable_id() {
    let mut world = World::default();
    let root = instantiate(&mut world, &oak());
    let children = world.get(root).expect("the root exists").children.clone();
    for entity in std::iter::once(root).chain(children) {
        assert!(
            world
                .get(entity)
                .and_then(|data| data.source_id.clone())
                .is_some(),
            "an entity a scene has to write needs an identity"
        );
    }
}

/// Instantiating twice is the ordinary case, and two entities sharing a stable
/// ID is a scene that refuses to load.
#[test]
fn a_second_instance_does_not_collide_with_the_first() {
    let mut world = World::default();
    let prefab = oak();
    instantiate(&mut world, &prefab);
    instantiate(&mut world, &prefab);

    let ids: Vec<String> = world
        .entities()
        .filter_map(|(_, data)| data.source_id.as_ref().map(|id| id.as_str().to_owned()))
        .collect();
    let mut unique = ids.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(ids.len(), 4, "two instances of two entities");
    assert_eq!(
        unique.len(),
        ids.len(),
        "and no two share an identity: {ids:?}"
    );
}

/// Both instances of one transaction have to be rehearsed together, or they
/// are each told the same next handle and land on top of each other.
#[test]
fn two_instances_in_one_transaction_land_apart() {
    let mut world = World::default();
    let prefab = oak();
    let mut rehearsal = world.clone();
    let mut buffer = CommandBuffer::new();
    let first = instantiate_into(&mut rehearsal, &prefab, None, &mut buffer).unwrap();
    let second = instantiate_into(&mut rehearsal, &prefab, None, &mut buffer).unwrap();
    assert_ne!(first, second, "two instances are two entities");

    let mut history = CommandHistory::default();
    history
        .apply(buffer.into_transaction("Add two"), &mut world)
        .expect("the spawns apply");
    assert!(world.get(first).is_some() && world.get(second).is_some());
}

/// One undoable step, not four.
#[test]
fn undoing_takes_the_whole_prefab_back() {
    let mut world = World::default();
    let prefab = oak();
    let mut rehearsal = world.clone();
    let mut buffer = CommandBuffer::new();
    let root = instantiate_into(&mut rehearsal, &prefab, None, &mut buffer).unwrap();
    let mut history = CommandHistory::default();
    history
        .apply(buffer.into_transaction("Add Oak"), &mut world)
        .expect("the spawns apply");
    assert!(world.get(root).is_some());

    history.undo(&mut world).expect("one step undoes it");
    assert!(
        world.entities().next().is_none(),
        "the whole prefab goes back in one step, children included"
    );
}

/// A prefab can be put under something already in the scene.
#[test]
fn a_prefab_can_arrive_under_a_parent() {
    let mut world = World::default();
    let host = world.spawn(EntityData {
        source_id: Some(id("host")),
        ..EntityData::default()
    });

    let prefab = oak();
    let mut rehearsal = world.clone();
    let mut buffer = CommandBuffer::new();
    let root = instantiate_into(&mut rehearsal, &prefab, Some(host), &mut buffer).unwrap();
    let mut history = CommandHistory::default();
    history
        .apply(buffer.into_transaction("Add Oak"), &mut world)
        .expect("the spawns apply");

    assert_eq!(world.get(root).expect("the root exists").parent, Some(host));
}

/// A prefab with two roots is refused while it is still somebody's choice of
/// asset, rather than half-spawned into their scene.
#[test]
fn a_prefab_without_one_root_is_refused() {
    let prefab = PrefabDocument {
        format_version: PREFAB_FORMAT_VERSION,
        metadata: sindri_core::SceneMetadata::default(),
        entities: vec![SceneEntity::new(id("one")), SceneEntity::new(id("two"))],
    };
    let mut rehearsal = World::default();
    let mut buffer = CommandBuffer::new();
    assert!(
        instantiate_into(&mut rehearsal, &prefab, None, &mut buffer).is_err(),
        "two roots is not a prefab"
    );
}

#[test]
fn a_missing_prefab_says_so_rather_than_panicking() {
    let error = load(Some(Path::new("/nonexistent")), "nothing.prefab.json")
        .expect_err("a missing file is an error");
    assert!(error.contains("nothing.prefab.json"), "{error}");
}
