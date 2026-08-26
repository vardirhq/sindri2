//! Loading a scene and saving it back as the same file.

use crate::{EntityData, SceneEntityId, Transform3D, World, WorldError};

use super::support::authored_scene;

#[test]
fn saving_a_loaded_world_reproduces_the_canonical_scene() {
    let authored = authored_scene();
    let loaded = World::from_scene(&authored).unwrap();
    let saved = loaded.world.to_scene().unwrap();
    assert_eq!(saved, authored.canonicalized());
    assert!(saved.is_canonical());
    assert_eq!(saved.metadata, authored.metadata);
    assert_eq!(
        saved
            .entity(&SceneEntityId::new("child").unwrap())
            .unwrap()
            .parent,
        Some(SceneEntityId::new("root").unwrap())
    );
}

#[test]
fn editing_a_transform_survives_a_save_and_reload() {
    let authored = authored_scene();
    let loaded = World::from_scene(&authored).unwrap();
    let mut world = loaded.world;
    let child = loaded.entity_map[&SceneEntityId::new("child").unwrap()];
    world.get_mut(child).unwrap().transform_3d = Some(Transform3D {
        position: [4.0, 8.0, -1.5],
        rotation: [0.0, 0.0, 0.247_404, 0.968_912],
        scale: [2.0, 2.0, 1.0],
        ..Transform3D::default()
    });

    let saved = world.to_scene().unwrap();
    let reloaded = World::from_scene(&saved).unwrap();
    let reloaded_child = reloaded.entity_map[&SceneEntityId::new("child").unwrap()];
    assert_eq!(
        reloaded.world.get(reloaded_child).unwrap().transform_3d,
        Some(Transform3D {
            position: [4.0, 8.0, -1.5],
            rotation: [0.0, 0.0, 0.247_404, 0.968_912],
            scale: [2.0, 2.0, 1.0],
            ..Transform3D::default()
        })
    );
    assert_eq!(reloaded.world.to_scene().unwrap(), saved);
}

#[test]
fn reparenting_is_preserved_without_losing_stable_ids() {
    let authored = authored_scene();
    let loaded = World::from_scene(&authored).unwrap();
    let mut world = loaded.world;
    let child = loaded.entity_map[&SceneEntityId::new("child").unwrap()];
    world.set_parent(child, None).unwrap();

    let saved = world.to_scene().unwrap();
    assert_eq!(
        saved
            .entity(&SceneEntityId::new("child").unwrap())
            .unwrap()
            .parent,
        None
    );
    assert_eq!(saved.entities.len(), 2);
}

#[test]
fn runtime_entities_without_stable_ids_cannot_be_saved_silently() {
    let mut world = World::default();
    let spawned = world.spawn(EntityData::default());
    assert_eq!(world.to_scene(), Err(WorldError::UnstableEntity(spawned)));

    let assigned = world.assign_missing_source_ids("entity").unwrap();
    assert_eq!(assigned.len(), 1);
    assert_eq!(assigned[0].1.as_str(), "entity-1");
    assert_eq!(world.to_scene().unwrap().entities.len(), 1);
}

#[test]
fn assigned_ids_skip_identities_already_in_use() {
    let mut world = World::default();
    world.spawn(EntityData {
        source_id: Some(SceneEntityId::new("entity-1").unwrap()),
        ..EntityData::default()
    });
    world.spawn(EntityData::default());
    world.spawn(EntityData::default());

    let assigned = world.assign_missing_source_ids("entity").unwrap();
    let minted: Vec<_> = assigned
        .iter()
        .map(|(_, source_id)| source_id.as_str())
        .collect();
    assert_eq!(minted, ["entity-2", "entity-3"]);
    world.to_scene().unwrap().validate().unwrap();
}

#[test]
fn saving_survives_slot_reuse_after_despawn() {
    let authored = authored_scene();
    let loaded = World::from_scene(&authored).unwrap();
    let mut world = loaded.world;
    let child = loaded.entity_map[&SceneEntityId::new("child").unwrap()];
    world.despawn_recursive(child).unwrap();
    world.spawn(EntityData {
        source_id: Some(SceneEntityId::new("alpha").unwrap()),
        ..EntityData::default()
    });

    // The new entity reuses the despawned slot, so document order would
    // otherwise depend on allocation history rather than identity.
    let saved = world.to_scene().unwrap();
    assert_eq!(
        saved
            .entities
            .iter()
            .map(|entity| entity.id.as_str())
            .collect::<Vec<_>>(),
        ["alpha", "root"]
    );
}
