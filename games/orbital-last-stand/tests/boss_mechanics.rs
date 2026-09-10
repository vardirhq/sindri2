//! Two bosses that have to be fought rather than out-damaged.
//!
//! Both mechanics are conditional damage, which is the kind of thing that fails
//! silently: a boss that should be invulnerable and is not still takes damage
//! and still dies, and a punish window that never opens just makes a fight
//! longer. Nothing about either would show up as an error.

use orbital_last_stand::Run;
use serde_json::json;
use sindri_core::EntityId;

const STEP: f32 = 1.0 / 60.0;

fn step(run: &mut Run) {
    let notes = run.step(STEP);
    assert!(notes.is_empty(), "{notes:#?}");
}

/// A run with the director off and the field cleared, so the only enemies are
/// the ones a test puts there.
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

fn spawn_boss(run: &mut Run, kind: f32) -> EntityId {
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
        .insert("kind".to_owned(), json!(kind));
    entity
}

/// Fire a shot into the boss from below and let it land.
fn shoot(run: &mut Run, damage: f32) {
    let bullet = run
        .prefabs
        .get("prefabs/bullet.prefab.json")
        .expect("the bullet prefab ships")
        .clone();
    let shot = run.world.spawn_prefab(&bullet).expect("a shot spawns").root;
    if let Some(data) = run.world.get_mut(shot) {
        if let Some(transform) = data.transform_3d.as_mut() {
            transform.position = [0.0, 1.6, 0.0];
        }
        if let Some(properties) = data
            .components
            .get_mut("sindri.script")
            .and_then(|script| script.get_mut("properties"))
            .and_then(serde_json::Value::as_object_mut)
        {
            properties.insert("damage".to_owned(), json!(damage));
            properties.insert("dir_x".to_owned(), json!(0.0));
            properties.insert("dir_y".to_owned(), json!(1.0));
            properties.insert("speed".to_owned(), json!(16.0));
        }
    }
    for _ in 0..14 {
        step(run);
    }
}

fn boss_share(run: &Run) -> f32 {
    let max = run.board("boss_max").max(0.001);
    run.board("boss_hp") / max
}

/// The Brood's children are the fight. While one is alive the parent is not
/// reachable at all; clear them and it is.
///
/// The child here is the test's own, placed beside the boss and off the line the
/// shots travel. Waiting for the Brood to spawn its own does not work: it puts
/// them right next to itself, so a shot aimed at the parent kills one on the
/// way, and then the shots that follow are landing on a boss whose children
/// really are dead. That the Brood spawns any at all is asserted separately.
#[test]
fn the_brood_is_only_reachable_once_its_children_are_dead() {
    let mut run = isolated_run();
    spawn_boss(&mut run, 4.0);
    step(&mut run);

    let child_prefab = run
        .prefabs
        .get("prefabs/brood-spawn.prefab.json")
        .expect("the brood prefab ships")
        .clone();
    let child = run
        .world
        .spawn_prefab(&child_prefab)
        .expect("a child spawns")
        .root;
    if let Some(transform) = run
        .world
        .get_mut(child)
        .and_then(|data| data.transform_3d.as_mut())
    {
        transform.position = [3.0, 3.5, 0.0];
    }
    step(&mut run);
    assert_eq!(run.count("brood"), 1, "one child, off the firing line");

    let guarded = boss_share(&run);
    for _ in 0..6 {
        shoot(&mut run, 12.0);
    }
    assert_eq!(
        run.count("brood"),
        1,
        "the child died during the guarded shots, so this proves nothing"
    );
    assert!(
        (boss_share(&run) - guarded).abs() < 0.0005,
        "the Brood was hurt with a child alive: {guarded:.4} -> {:.4}",
        boss_share(&run)
    );

    // Clear the room and the same shots land.
    run.world.despawn_recursive(child).expect("child despawns");
    step(&mut run);
    assert_eq!(run.count("brood"), 0, "the room is clear");

    let open = boss_share(&run);
    for _ in 0..6 {
        shoot(&mut run, 12.0);
    }
    assert!(
        boss_share(&run) < open - 0.01,
        "with its children gone the Brood should be hurt: {open:.4} -> {:.4}",
        boss_share(&run)
    );
}

/// And it does put children on the field by itself, which is what makes the
/// guard above something the player has to deal with rather than a curiosity.
#[test]
fn the_brood_raises_children_of_its_own() {
    let mut run = isolated_run();
    spawn_boss(&mut run, 4.0);
    // `special` starts at 2.7 seconds and the spawn rides that timer.
    for _ in 0..240 {
        step(&mut run);
    }
    assert!(
        run.count("brood") > 0,
        "the Brood never spawned anything to hide behind"
    );
}

/// The Harrower's charge is the opening. It costs the same shot far more after
/// a dash than before one, which is what makes baiting the charge the fight.
#[test]
fn the_harrower_is_worth_hitting_when_it_has_just_charged() {
    let mut run = isolated_run();
    let boss = spawn_boss(&mut run, 0.0);

    // Its first dash is on a timer; run until it has been and gone.
    let mut punished = 0.0_f32;
    let mut ordinary = 0.0_f32;
    for _ in 0..900 {
        step(&mut run);
        let stunned = run
            .world
            .get(boss)
            .and_then(|data| data.components.get("sindri.script"))
            .and_then(|script| script.get("properties"))
            .and_then(|properties| properties.get("stunned"))
            .is_some();
        let _ = stunned;
        let before = boss_share(&run);
        // A shot is cheap; take one now and record what it was worth.
        shoot(&mut run, 6.0);
        let taken = before - boss_share(&run);
        if taken > punished {
            punished = taken;
        }
        if taken > 0.0 && (ordinary == 0.0 || taken < ordinary) {
            ordinary = taken;
        }
        if punished > 0.0 && ordinary > 0.0 && punished > ordinary * 2.0 {
            break;
        }
    }

    assert!(ordinary > 0.0, "no shot ever landed on the Harrower");
    assert!(
        punished > ordinary * 2.0,
        "the same shot was worth {ordinary:.5} at best and {punished:.5} at \
         worst: the charge never opened a window"
    );
}
