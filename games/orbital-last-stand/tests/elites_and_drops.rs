use orbital_last_stand::Run;
use serde_json::json;
use sindri_core::EntityId;

const STEP: f32 = 1.0 / 60.0;

fn step(run: &mut Run) {
    let notes = run.step(STEP);
    assert!(notes.is_empty(), "{notes:#?}");
}

fn set_script_number(run: &mut Run, entity: EntityId, name: &str, value: f32) {
    run.world
        .get_mut(entity)
        .and_then(|data| data.components.get_mut("sindri.script"))
        .and_then(|script| script.get_mut("properties"))
        .and_then(serde_json::Value::as_object_mut)
        .expect("the entity has authored script properties")
        .insert(name.to_owned(), json!(value));
}

fn spawn_at(run: &mut Run, prefab: &str, position: [f32; 3]) -> EntityId {
    let document = run.prefabs.get(prefab).expect("the prefab is loaded");
    let entity = run
        .world
        .spawn_prefab(document)
        .expect("the prefab spawns")
        .root;
    run.world
        .get_mut(entity)
        .and_then(|data| data.transform_3d.as_mut())
        .expect("the prefab has a transform")
        .position = position;
    entity
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
    run.world
        .get_mut(director)
        .expect("the director remains")
        .disabled = true;

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
        run.world
            .despawn_recursive(enemy)
            .expect("the opening enemy despawns");
    }
    step(&mut run);
    run
}

#[test]
fn elite_timing_and_normal_drop_values_are_authored_from_the_reference() {
    let run = Run::open().expect("the project opens");
    let director = run.find("Director").expect("the director exists");
    let elite = run
        .world
        .get(director)
        .expect("the director remains")
        .components["sindri.script"]["properties"]
        .as_object()
        .expect("the director has properties");
    for (name, expected) in [
        ("elite_after", 105.0),
        ("elite_base", 0.025),
        ("elite_time_divisor", 3000.0),
        ("elite_bonus_scale", 0.65),
        ("elite_cap", 0.32),
    ] {
        let actual = elite[name].as_f64().expect("the elite value is numeric");
        assert!((actual - expected).abs() < 1.0e-6, "{name}: {actual}");
    }

    let resolver = run
        .prefabs
        .get("prefabs/drop-roll.prefab.json")
        .expect("the drop resolver ships");
    let properties = &resolver.root().expect("the resolver has a root").components["sindri.script"]
        ["properties"];
    for (name, expected) in [
        ("regular_chance", 0.058),
        ("elite_chance", 0.26),
        ("boss_chance", 1.0),
        ("low_health_bonus", 0.055),
        ("repair_weight", 0.44),
        ("repair_bias", 0.26),
        ("pulse_weight", 0.24),
        ("pity_kills", 120.0),
    ] {
        let actual = properties[name]
            .as_f64()
            .expect("the drop value is numeric");
        assert!((actual - expected).abs() < 1.0e-6, "{name}: {actual}");
    }
}

#[test]
fn a_forced_elite_receives_one_complete_reference_trait() {
    let mut run = Run::open().expect("the project opens");
    let director = run.find("Director").expect("the director exists");
    set_script_number(&mut run, director, "first_boss_at", 1000.0);
    set_script_number(&mut run, director, "elite_after", -1.0);
    set_script_number(&mut run, director, "elite_base", 1.0);
    set_script_number(&mut run, director, "elite_cap", 1.0);
    for _ in 0..6 {
        step(&mut run);
    }
    run.click("TitleStart");
    step(&mut run);

    assert_eq!(run.board("elite_spawned"), 1.0);
    let enemy = run
        .world
        .entities()
        .find(|(entity, _)| {
            run.components
                .get::<sindri_core::TagsComponent>(&run.world, *entity)
                .ok()
                .flatten()
                .is_some_and(|tags| tags.has("enemy"))
        })
        .map(|(_, data)| data)
        .expect("the forced elite spawned");
    let properties = &enemy.components["sindri.script"]["properties"];
    let actual = (
        properties["health"].as_f64().unwrap_or_default(),
        properties["touch_damage"].as_f64().unwrap_or_default(),
        properties["speed"].as_f64().unwrap_or_default(),
        properties["worth"].as_f64().unwrap_or_default(),
    );
    let expected = [
        (4.70, 1.05, 1.32, 2.20),
        (3.10, 1.45, 2.07, 2.10),
        (3.30, 1.20, 1.62, 2.25),
        (3.70, 1.25, 1.59, 2.30),
        (3.40, 1.12, 1.68, 2.25),
    ];
    assert!(
        expected.iter().any(|candidate| {
            (actual.0 - candidate.0).abs() < 1.0e-5
                && (actual.1 - candidate.1).abs() < 1.0e-5
                && (actual.2 - candidate.2).abs() < 1.0e-5
                && (actual.3 - candidate.3).abs() < 1.0e-5
        }),
        "elite multipliers did not match a reference trait: {actual:?}"
    );
    assert!(
        enemy.components["sindri.shape"]["fill"][3]
            .as_f64()
            .is_some_and(|alpha| alpha > 0.0),
        "the elite has no visible treatment"
    );
}

#[test]
fn repair_pity_guarantees_and_collects_a_scaled_repair() {
    let mut run = isolated_run();
    run.set_board("hp", 1.0);
    run.set_board("kills_since_repair", 119.0);
    spawn_at(&mut run, "prefabs/drop-roll.prefab.json", [0.0, 0.0, 0.0]);

    step(&mut run);
    assert_eq!(run.count("powerup"), 1, "the pity roll did not drop");
    assert_eq!(run.board("kills_since_repair"), 0.0);
    step(&mut run);
    assert_eq!(run.count("powerup"), 0, "the repair was not collected");
    assert!(
        (run.board("hp") - 2.8).abs() < 1.0e-5,
        "hp: {}",
        run.board("hp")
    );
}

#[test]
fn pulse_clears_hostile_fire_and_overdrive_counts_down() {
    let mut run = isolated_run();
    spawn_at(
        &mut run,
        "prefabs/enemy-bullet.prefab.json",
        [3.0, 0.0, 0.0],
    );
    spawn_at(&mut run, "prefabs/drifter.prefab.json", [3.0, 0.0, 0.0]);
    let pulse = spawn_at(&mut run, "prefabs/powerup.prefab.json", [0.0, 0.0, 0.0]);
    set_script_number(&mut run, pulse, "kind", 1.0);

    step(&mut run);
    assert_eq!(run.count("hostile_shot"), 0, "pulse left hostile fire");
    step(&mut run);
    assert_eq!(run.count("enemy"), 0, "pulse did not damage the arena");

    let overdrive = spawn_at(&mut run, "prefabs/powerup.prefab.json", [0.0, 0.0, 0.0]);
    set_script_number(&mut run, overdrive, "kind", 2.0);
    step(&mut run);
    assert_eq!(run.board("overdrive"), 8.0);
    step(&mut run);
    assert!(run.board("overdrive") < 8.0, "overdrive did not count down");
}
