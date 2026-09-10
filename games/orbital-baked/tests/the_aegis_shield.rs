//! The Aegis can only be hurt through a hole in its own shield.
//!
//! Nothing in the script says so. A bullet spends itself on the first sensor it
//! enters and the ring orbits outside the hull's collider, so the gating is a
//! consequence of where things are rather than a rule anybody wrote. That is
//! the good version of this mechanic and also the fragile one: a collider layer
//! typo, a radius that tucks the ring inside the hull, or a plate that stops
//! being a sensor would all leave a boss that looks shielded and is not.

use orbital_baked::Run;
use serde_json::json;
use sindri_core::EntityId;

const STEP: f32 = 1.0 / 60.0;

fn step(run: &mut Run) {
    let notes = run.step(STEP);
    assert!(notes.is_empty(), "{notes:#?}");
}

fn isolated_run() -> Run {
    let mut run = Run::open().expect("the project opens");
    for _ in 0..6 {
        step(&mut run);
    }
    run.click("TitleStart");
    step(&mut run);
    run.set_board("hp", 1000.0);
    let director = run.find("Director").expect("the director exists");
    run.world.get_mut(director).expect("director").disabled = true;
    let enemies: Vec<_> = run
        .world
        .entities()
        .filter_map(|(entity, _)| {
            run.components
                .get::<sindri_core::TagsComponent>(&run.world, entity)
                .ok()
                .flatten()
                .is_some_and(|tags| tags.has("enemy"))
                .then_some(entity)
        })
        .collect();
    for enemy in enemies {
        run.world.despawn_recursive(enemy).expect("enemy despawns");
    }
    step(&mut run);
    run
}

/// Every entity carrying a tag, which the harness counts but does not hand back.
fn tagged(run: &Run, tag: &str) -> Vec<EntityId> {
    run.world
        .entities()
        .filter_map(|(entity, _)| {
            run.components
                .get::<sindri_core::TagsComponent>(&run.world, entity)
                .ok()
                .flatten()
                .is_some_and(|tags| tags.has(tag))
                .then_some(entity)
        })
        .collect()
}

fn spawn_aegis(run: &mut Run) -> EntityId {
    let document = run
        .prefabs
        .get("prefabs/challenger.prefab.json")
        .expect("the boss prefab ships");
    let entity = run.world.spawn_prefab(document).expect("boss spawns").root;
    let data = run.world.get_mut(entity).expect("boss remains");
    data.transform_3d.as_mut().expect("boss transform").position = [0.0, 3.5, 0.0];
    data.components
        .get_mut("sindri.script")
        .and_then(|script| script.get_mut("properties"))
        .and_then(serde_json::Value::as_object_mut)
        .expect("script properties")
        .insert("kind".to_owned(), json!(10.0));
    entity
}

#[test]
fn it_raises_a_ring_of_plates_once() {
    let mut run = isolated_run();
    spawn_aegis(&mut run);
    step(&mut run);
    step(&mut run);
    assert_eq!(run.count("shield"), 8, "the Aegis raises eight plates");

    // Built once, not once a frame. A ring rebuilt every tick would look right
    // in a screenshot and be unkillable in a game.
    for _ in 0..30 {
        step(&mut run);
    }
    assert_eq!(
        run.count("shield"),
        8,
        "the ring is built once rather than every frame"
    );
}

/// The plates orbit rather than sitting where they were spawned, which is what
/// makes the opening sweep and the fight about position.
#[test]
fn the_ring_turns() {
    let mut run = isolated_run();
    let boss = spawn_aegis(&mut run);
    step(&mut run);
    step(&mut run);

    let plate = *tagged(&run, "shield")
        .first()
        .expect("at least one plate exists");
    let before = run
        .world
        .get(plate)
        .and_then(|data| data.transform_3d.as_ref())
        .expect("a plate has a transform")
        .position;
    for _ in 0..40 {
        step(&mut run);
    }
    let after = run
        .world
        .get(plate)
        .and_then(|data| data.transform_3d.as_ref())
        .expect("the plate survives")
        .position;
    let moved = (after[0] - before[0]).hypot(after[1] - before[1]);
    assert!(
        moved > 0.15,
        "the plate barely moved ({moved:.3}); the ring is not turning"
    );

    // Still on its ring rather than drifting off it: the radius is what keeps
    // the plates outside the hull, which is the whole of the gating.
    // A child's transform is local to its parent, so this is the ring radius
    // directly: the plate rides at 1.55 from a hull that is itself moving.
    let radius = after[0].hypot(after[1]);
    assert!(
        (radius - 1.55).abs() < 0.2,
        "the plate left its ring (radius {radius:.3})"
    );
    let _ = boss;
}

/// The plates are what the player's fire lands on, which is the mechanic.
///
/// If a plate's collider were on the wrong layer, or the ring tucked inside the
/// hull, the shots would go straight past and the boss would be shielded in
/// appearance only. So this fires into the ring and checks the ring pays for it.
#[test]
fn fire_lands_on_the_plates_rather_than_the_hull() {
    let mut run = isolated_run();
    let boss = spawn_aegis(&mut run);
    step(&mut run);
    step(&mut run);
    assert_eq!(run.count("shield"), 8, "the ring is up");

    let bullet = run
        .prefabs
        .get("prefabs/bullet.prefab.json")
        .expect("the bullet prefab ships")
        .clone();

    // Fired from below the boss, straight up into the ring, over and over: the
    // ring turns, so this walks around it rather than drilling one plate.
    for _ in 0..90 {
        let shot = run.world.spawn_prefab(&bullet).expect("a shot spawns").root;
        if let Some(data) = run.world.get_mut(shot) {
            if let Some(transform) = data.transform_3d.as_mut() {
                transform.position = [0.0, 0.6, 0.0];
            }
            if let Some(properties) = data
                .components
                .get_mut("sindri.script")
                .and_then(|script| script.get_mut("properties"))
                .and_then(serde_json::Value::as_object_mut)
            {
                properties.insert("damage".to_owned(), json!(4.0));
                properties.insert("dir_x".to_owned(), json!(0.0));
                properties.insert("dir_y".to_owned(), json!(1.0));
                properties.insert("speed".to_owned(), json!(18.0));
            }
        }
        for _ in 0..4 {
            step(&mut run);
        }
    }

    let left = run.count("shield");
    assert!(
        left < 8,
        "ninety shots into the ring destroyed no plates: the fire is not \
         landing on them, so the shield is decoration rather than a mechanic"
    );
    assert!(run.world.get(boss).is_some(), "the boss is still alive");
}
