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
        .get("prefabs/aegis.prefab.json")
        .expect("the Aegis prefab ships");
    let entity = run.world.spawn_prefab(document).expect("boss spawns").root;
    let data = run.world.get_mut(entity).expect("boss remains");
    data.transform_3d.as_mut().expect("boss transform").position = [0.0, 3.5, 0.0];
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
    // the plates outside the hull, which is the whole of the gating. Measured
    // from the boss rather than from the origin, because that is the distance
    // the mechanic is about — and because measuring it from the origin is how
    // a ring that had come off its boss entirely once passed this test.
    let hull = run
        .world
        .get(boss)
        .and_then(|data| data.transform_3d.as_ref())
        .expect("the boss survives")
        .position;
    let radius = (after[0] - hull[0]).hypot(after[1] - hull[1]);
    assert!(
        (radius - 1.55).abs() < 0.2,
        "the plate left its ring (radius {radius:.3} from a hull at          {:.3},{:.3})",
        hull[0],
        hull[1]
    );
}

/// The plates are what the player's fire lands on, which is the mechanic.
///
/// If a plate's collider were on the wrong layer, or the ring tucked inside the
/// hull, the shots would go straight past and the boss would be shielded in
/// appearance only. So this fires at the boss and checks two things in order:
/// that a whole ring pays for the shots instead of the hull, and that once the
/// ring has holes in it the hull starts paying after all. Aimed at wherever the
/// boss actually is, because an earlier version fired at a fixed point and
/// passed while the ring was sitting at the origin nowhere near the boss.
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
    let full = run.board("boss_hp");

    // One shot, aimed from just outside the ring at the hull behind it.
    let fire = |run: &mut Run| {
        let Some(hull) = run
            .world
            .get(boss)
            .and_then(|data| data.transform_3d.as_ref())
            .map(|transform| transform.position)
        else {
            return;
        };
        let shot = run.world.spawn_prefab(&bullet).expect("a shot spawns").root;
        if let Some(data) = run.world.get_mut(shot) {
            if let Some(transform) = data.transform_3d.as_mut() {
                transform.position = [hull[0], hull[1] - 2.6, 0.0];
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
            step(run);
        }
    };

    // While the ring is whole the hull is unreachable, wherever it has got to.
    let mut whole = 0;
    while run.count("shield") == 8 && whole < 12 {
        fire(&mut run);
        whole += 1;
    }
    assert!(
        whole > 0,
        "no shot was fired while the ring was whole, so nothing was tested"
    );
    assert!(
        (run.board("boss_hp") - full).abs() < 0.001,
        "the hull lost {:.2} through an unbroken ring",
        full - run.board("boss_hp")
    );

    // Keep firing: the ring turns, so this walks around it rather than
    // drilling one plate, and the gaps it opens are how the hull is reached.
    // Stopped at the first hit on the hull, because the question is whether an
    // opening lets a shot through, not how long the boss survives one.
    let mut shots = 0;
    while shots < 90 && run.board("boss_hp") >= full {
        fire(&mut run);
        shots += 1;
    }
    let left = run.count("shield");
    assert!(
        left < 8,
        "ninety shots into the ring destroyed no plates: the fire is not \
         landing on them, so the shield is decoration rather than a mechanic"
    );
    assert!(
        run.board("boss_hp") < full,
        "the hull never took a shot through a ring with {} plates missing: \
         the opening is not an opening",
        8 - left
    );
}

/// The Aegis wears its own steel.
///
/// For a long time every boss but this one was a single orange gunship
/// recoloured by a script tint, and the Aegis was too: a blue asked for once at
/// configuration and overwritten every frame after by the cream its neighbours
/// shared. Neither colour was one anybody chose for it, and neither belonged
/// beside its own blue plates. Every boss wears its own hull now, so the guard
/// is two-sided — the right texture, and no tint over it.
#[test]
fn it_wears_its_own_hull_rather_than_a_tinted_gunship() {
    let mut run = isolated_run();
    let aegis = spawn_aegis(&mut run);
    for _ in 0..40 {
        step(&mut run);
    }

    let sprite = run
        .components
        .get::<sindri_scene::SpriteComponent>(&run.world, aegis)
        .expect("the sprite store answers")
        .expect("the Aegis draws a sprite")
        .clone();

    assert!(
        sprite.texture.starts_with("textures/aegis.png#"),
        "the Aegis draws its own sheet, not {}",
        sprite.texture
    );

    for (channel, name) in sprite.tint.iter().take(3).zip(["red", "green", "blue"]) {
        assert!(
            (channel - 1.0).abs() < 1e-3,
            "the {name} channel is {channel}: a tint would recolour a hull that \
             is already the colour it should be"
        );
    }
}

/// The ring goes where the boss goes.
///
/// The plates were spawned as children of the hull, which reads as the obvious
/// way to say "these belong to it" and was wrong: nothing composes a parent's
/// transform into a world sprite or a collider, so the ring drew and blocked
/// around the origin while the boss flew around somewhere else. On screen it
/// was a tidy shield in the middle of the arena guarding nothing, and every
/// shot at the boss went straight through. So this lets the hull travel and
/// checks the ring is still on it.
#[test]
fn the_ring_travels_with_the_hull() {
    let mut run = isolated_run();
    let boss = spawn_aegis(&mut run);
    step(&mut run);
    step(&mut run);

    // Long enough for the boss to have driven itself well away from where it
    // started, under its own movement rather than a position written over it:
    // physics owns a dynamic body's pose, so a transform set from here does
    // not survive the step that follows.
    for _ in 0..180 {
        step(&mut run);
    }

    let hull = run
        .world
        .get(boss)
        .and_then(|data| data.transform_3d.as_ref())
        .expect("the boss survives")
        .position;
    // Otherwise this proves nothing: a boss that never left the middle of the
    // arena is exactly where a ring stuck at the origin would be anyway.
    let travelled = hull[0].hypot(hull[1]);
    assert!(
        travelled > 2.0,
        "the boss only reached {travelled:.3} from the origin, which is too \
         close to tell a ring that follows it from one that does not"
    );

    let plates = tagged(&run, "shield");
    assert!(!plates.is_empty(), "the ring survives");
    for plate in plates {
        let at = run
            .world
            .get(plate)
            .and_then(|data| data.transform_3d.as_ref())
            .expect("a plate has a transform")
            .position;
        let radius = (at[0] - hull[0]).hypot(at[1] - hull[1]);
        assert!(
            (radius - 1.55).abs() < 0.25,
            "a plate sits {radius:.3} from a hull at {:.3},{:.3}: the ring is \
             not on the boss",
            hull[0],
            hull[1]
        );
    }
}

/// A plate does not outlive the boss it rings.
///
/// It used to be a child, so despawning the hull took the ring with it. Now
/// that they are siblings the plates have to notice, or a dead boss leaves
/// eight invisible walls in the arena that eat the player's fire forever.
#[test]
fn the_ring_goes_when_the_boss_does() {
    let mut run = isolated_run();
    let boss = spawn_aegis(&mut run);
    step(&mut run);
    step(&mut run);
    assert_eq!(run.count("shield"), 8, "the ring is up");

    run.world.despawn_recursive(boss).expect("the boss dies");
    for _ in 0..4 {
        step(&mut run);
    }
    assert_eq!(run.count("shield"), 0, "the ring went with it");
}
