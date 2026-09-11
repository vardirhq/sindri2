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

/// Resolving a stable identity back to the entity it names.
///
/// The direction an authoring host needs: a proposal, a saved selection and a
/// prefab reference all name entities the way the file does, because a runtime
/// handle means nothing once the scene has been reloaded.
mod source_ids {
    use crate::{EntityData, SceneEntityId, World};

    fn identified(world: &mut World, id: &str) -> crate::EntityId {
        let source_id = SceneEntityId::new(id).expect("a valid stable id");
        world.spawn(EntityData {
            source_id: Some(source_id),
            ..EntityData::default()
        })
    }

    #[test]
    fn a_stable_id_resolves_to_its_entity() {
        let mut world = World::default();
        let first = identified(&mut world, "player");
        let second = identified(&mut world, "director");
        let player = SceneEntityId::new("player").expect("a valid stable id");
        let director = SceneEntityId::new("director").expect("a valid stable id");
        assert_eq!(world.entity_for_source_id(&player), Some(first));
        assert_eq!(world.entity_for_source_id(&director), Some(second));
    }

    /// "Resolve references without guessing" is the host's rule, so an identity
    /// nothing carries is an answer of none rather than a nearby entity.
    #[test]
    fn an_unknown_stable_id_resolves_to_nothing() {
        let mut world = World::default();
        identified(&mut world, "player");
        let absent = SceneEntityId::new("nobody").expect("a valid stable id");
        assert_eq!(world.entity_for_source_id(&absent), None);
    }

    /// An entity spawned at runtime carries no stable id until one is assigned,
    /// and must not be reachable through this before it has one.
    #[test]
    fn an_entity_with_no_stable_id_is_not_resolvable() {
        let mut world = World::default();
        let spawned = world.spawn(EntityData::default());
        assert!(
            world
                .source_id_map()
                .values()
                .all(|entity| *entity != spawned)
        );
    }

    #[test]
    fn a_despawned_entity_stops_resolving() {
        let mut world = World::default();
        let entity = identified(&mut world, "player");
        let player = SceneEntityId::new("player").expect("a valid stable id");
        assert_eq!(world.entity_for_source_id(&player), Some(entity));
        world
            .despawn_recursive(entity)
            .expect("an entity with no children despawns");
        assert_eq!(world.entity_for_source_id(&player), None);
    }

    /// The bulk form, for a caller resolving a dozen references at once.
    #[test]
    fn the_map_holds_every_identity_the_world_carries() {
        let mut world = World::default();
        let first = identified(&mut world, "player");
        let second = identified(&mut world, "director");
        world.spawn(EntityData::default());
        let map = world.source_id_map();
        assert_eq!(map.len(), 2, "only the entities carrying an identity");
        assert_eq!(
            map.get(&SceneEntityId::new("player").expect("a valid stable id")),
            Some(&first)
        );
        assert_eq!(
            map.get(&SceneEntityId::new("director").expect("a valid stable id")),
            Some(&second)
        );
    }

    /// The two forms must not be able to disagree.
    #[test]
    fn the_map_and_the_single_lookup_agree() {
        let mut world = World::default();
        identified(&mut world, "player");
        identified(&mut world, "director");
        for (source_id, entity) in world.source_id_map() {
            assert_eq!(world.entity_for_source_id(&source_id), Some(entity));
        }
    }
}
