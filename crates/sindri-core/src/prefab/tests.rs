use serde_json::json;

use crate::{PrefabDocument, PrefabError, SceneEntity, SceneEntityId, SceneError, World};

fn id(name: &str) -> SceneEntityId {
    SceneEntityId::new(name).expect("a non-empty literal is a valid identity")
}

fn entity(name: &str, parent: Option<&str>) -> SceneEntity {
    SceneEntity {
        name: Some(name.to_owned()),
        parent: parent.map(id),
        ..SceneEntity::new(id(name))
    }
}

fn enemy() -> PrefabDocument {
    let mut root = entity("Enemy", None);
    root.components.insert(
        "sindri.sprite".to_owned(),
        json!({ "texture": "textures/enemy.png", "layer": 2 }),
    );
    PrefabDocument {
        entities: vec![
            root,
            entity("Muzzle", Some("Enemy")),
            entity("Shield", Some("Enemy")),
        ],
        ..PrefabDocument::default()
    }
}

#[test]
fn a_prefab_names_its_single_root() {
    let prefab = enemy();
    assert_eq!(prefab.root().expect("one root").id, id("Enemy"));
}

#[test]
fn several_roots_are_refused_rather_than_one_of_them_chosen() {
    let prefab = PrefabDocument {
        entities: vec![entity("First", None), entity("Second", None)],
        ..PrefabDocument::default()
    };
    assert_eq!(prefab.validate(), Err(PrefabError::SeveralRoots(2)));
}

#[test]
fn an_empty_prefab_describes_nothing_and_is_refused() {
    assert_eq!(
        PrefabDocument::default().validate(),
        Err(PrefabError::NoRoot)
    );
}

#[test]
fn a_prefab_from_a_newer_format_is_refused_rather_than_guessed_at() {
    let prefab = PrefabDocument {
        format_version: crate::PREFAB_FORMAT_VERSION + 1,
        ..enemy()
    };
    assert_eq!(
        prefab.validate(),
        Err(PrefabError::UnsupportedVersion {
            found: crate::PREFAB_FORMAT_VERSION + 1,
            supported: crate::PREFAB_FORMAT_VERSION,
        })
    );
}

#[test]
fn a_prefab_carries_the_entity_rules_a_scene_carries() {
    let prefab = PrefabDocument {
        entities: vec![entity("Root", None), entity("Child", Some("Absent"))],
        ..PrefabDocument::default()
    };
    assert_eq!(
        prefab.validate(),
        Err(PrefabError::Entities(SceneError::MissingParent {
            entity: id("Child"),
            parent: id("Absent"),
        }))
    );
}

#[test]
fn canonical_serialization_is_a_fixed_point() {
    let once = enemy().to_canonical_json().expect("a valid prefab writes");
    let twice = PrefabDocument::from_json(&once)
        .expect("what was written reads back")
        .to_canonical_json()
        .expect("and writes again");
    assert_eq!(once, twice);
    assert!(once.ends_with('\n'));
}

#[test]
fn spawning_puts_the_whole_subtree_in_the_world_under_one_root() {
    let mut world = World::default();
    let spawned = world.spawn_prefab(&enemy()).expect("a valid prefab spawns");

    assert_eq!(world.len(), 3);
    assert_eq!(spawned.entities.len(), 3);
    let root = world.get(spawned.root).expect("the root is in the world");
    assert_eq!(root.name.as_deref(), Some("Enemy"));
    assert_eq!(root.children.len(), 2);
    assert_eq!(
        root.components["sindri.sprite"]["texture"],
        json!("textures/enemy.png")
    );
    for child in root.children.clone() {
        assert_eq!(
            world.get(child).expect("a child").parent,
            Some(spawned.root)
        );
    }
}

#[test]
fn two_instances_share_an_authored_identity_and_nothing_else() {
    let mut world = World::default();
    let prefab = enemy();
    let first = world.spawn_prefab(&prefab).expect("spawns");
    let second = world.spawn_prefab(&prefab).expect("spawns again");

    assert_ne!(first.root, second.root);
    assert_eq!(world.len(), 6);
    // No stable identity, because a prefab's identities name entities inside
    // the prefab: two instances carrying them would collide on every one.
    for spawned in [&first, &second] {
        for entity in &spawned.entities {
            assert!(world.get(*entity).expect("spawned").source_id.is_none());
        }
    }
    assert_eq!(first.by_source_id[&id("Muzzle")], first.entities[1]);
}

#[test]
fn a_prefab_with_several_roots_reaches_the_world_not_at_all() {
    let mut world = World::default();
    let broken = PrefabDocument {
        entities: vec![entity("First", None), entity("Second", None)],
        ..PrefabDocument::default()
    };
    assert!(world.spawn_prefab(&broken).is_err());
    assert!(world.is_empty(), "a refused prefab spawns nothing at all");
}
