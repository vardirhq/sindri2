use orbital_last_stand::Run;
use sindri_core::{EntityId, TagsComponent};

const STEP: f32 = 1.0 / 60.0;

fn step(run: &mut Run) {
    let notes = run.step(STEP);
    assert!(notes.is_empty(), "{notes:#?}");
}

fn tagged(run: &Run, tag: &str) -> Vec<EntityId> {
    run.world
        .entities()
        .filter_map(|(entity, _)| {
            run.components
                .get::<TagsComponent>(&run.world, entity)
                .ok()
                .flatten()
                .is_some_and(|tags| tags.has(tag))
                .then_some(entity)
        })
        .collect()
}

fn positions(run: &Run, entities: &[EntityId]) -> Vec<[f32; 3]> {
    entities
        .iter()
        .map(|entity| {
            run.world
                .get(*entity)
                .and_then(|data| data.transform_3d)
                .expect("the hazard has a transform")
                .position
        })
        .collect()
}

fn rotations(run: &Run, entities: &[EntityId]) -> Vec<f32> {
    entities
        .iter()
        .map(|entity| {
            run.world
                .get(*entity)
                .and_then(|data| data.transform_3d)
                .expect("the hazard has a transform")
                .rotation_z_radians()
        })
        .collect()
}

fn isolated_run() -> Run {
    let mut run = Run::open().expect("the project opens");
    for _ in 0..6 {
        step(&mut run);
    }
    run.click("TitleStart");
    step(&mut run);
    run.set_board("hp", 1000.0);
    let director = run.find("Director").expect("the campaign director exists");
    run.world
        .get_mut(director)
        .expect("the campaign director remains")
        .disabled = true;
    run
}

#[test]
fn the_five_sectors_swap_real_hazards() {
    let mut run = isolated_run();

    assert_eq!(run.board("sector"), 1.0);
    assert_eq!(
        run.count("hazard"),
        7,
        "Outer Drift starts with seven rocks"
    );

    run.set_board("sector", 2.0);
    step(&mut run);
    assert_eq!(run.count("hazard"), 2, "Ember Belt owns two edge flares");

    run.set_board("sector", 3.0);
    step(&mut run);
    assert_eq!(
        run.count("hazard"),
        3,
        "Violet Wake owns three gravity wells"
    );

    run.set_board("sector", 4.0);
    step(&mut run);
    assert_eq!(
        run.count("hazard"),
        1,
        "Null Lattice owns one roaming field"
    );

    let field = run.find("Hazard Field").expect("the null field exists");
    let player = run.find("Player").expect("the player exists");
    let player_position = run
        .world
        .get(player)
        .and_then(|data| data.transform_3d)
        .expect("the player has a transform")
        .position;

    // Physics owns the dynamic player's position, so move the non-physics
    // field onto the player to exercise suppression deterministically.
    run.world
        .get_mut(field)
        .and_then(|data| data.transform_3d.as_mut())
        .expect("the null field has a transform")
        .position = player_position;
    step(&mut run);
    assert_eq!(
        run.board("nullified"),
        1.0,
        "the field should suppress weapons"
    );

    run.set_board("sector", 5.0);
    step(&mut run);
    assert_eq!(
        run.board("nullified"),
        0.0,
        "leaving Null Lattice restores weapons"
    );
    assert_eq!(
        run.count("hazard"),
        1,
        "Core Approach immediately queues a beam"
    );
}

#[test]
fn null_suppression_is_preserved_until_the_overlay_closes() {
    let mut run = isolated_run();
    run.set_board("sector", 4.0);
    step(&mut run);
    let field = run.find("Hazard Field").expect("the null field exists");
    let player = run.find("Player").expect("the player exists");
    let player_position = run
        .world
        .get(player)
        .and_then(|data| data.transform_3d)
        .expect("the player has a transform")
        .position;
    run.world
        .get_mut(field)
        .and_then(|data| data.transform_3d.as_mut())
        .expect("the null field has a transform")
        .position = player_position;
    step(&mut run);
    assert_eq!(run.board("nullified"), 1.0);

    run.set_board("run_state", 2.0);
    for _ in 0..6 {
        step(&mut run);
    }
    assert_eq!(
        run.board("nullified"),
        1.0,
        "level-up briefly re-enabled weapons inside the null field"
    );
}

#[test]
fn hazard_layout_survives_level_up_and_pause_overlays() {
    let mut run = isolated_run();

    for sector in 1..=5 {
        run.set_board("sector", sector as f32);
        step(&mut run);
        let before = tagged(&run, "hazard");
        let before_rotations = rotations(&run, &before);
        assert!(!before.is_empty(), "sector {sector} created no hazard");

        // Physics runs before scripts, so the first paused frame may finish
        // one authored movement step. Everything must then remain frozen.
        run.set_board("run_state", 2.0);
        step(&mut run);
        let paused_ids = tagged(&run, "hazard");
        let paused_positions = positions(&run, &paused_ids);
        assert_eq!(paused_ids, before, "sector {sector} rebuilt for level-up");
        if sector == 1 {
            assert_eq!(
                rotations(&run, &paused_ids),
                before_rotations,
                "asteroids snapped to authored angles when level-up opened"
            );
        }
        for _ in 0..8 {
            step(&mut run);
        }
        assert_eq!(
            tagged(&run, "hazard"),
            paused_ids,
            "sector {sector} replaced hazards while paused"
        );
        assert_eq!(
            positions(&run, &paused_ids),
            paused_positions,
            "sector {sector} kept moving behind the overlay"
        );

        run.set_board("run_state", 1.0);
        step(&mut run);
        assert_eq!(
            tagged(&run, "hazard"),
            paused_ids,
            "sector {sector} regenerated hazards when play resumed"
        );

        run.set_board("run_state", 3.0);
        step(&mut run);
        let manual_pause_ids = tagged(&run, "hazard");
        for _ in 0..4 {
            step(&mut run);
        }
        assert_eq!(
            tagged(&run, "hazard"),
            manual_pause_ids,
            "sector {sector} regenerated hazards during manual pause"
        );
        run.set_board("run_state", 1.0);
        step(&mut run);
    }
}

#[test]
fn outer_drift_starts_clear_of_the_player() {
    let run = isolated_run();
    let player = run.find("Player").expect("the player exists");
    let player_position = run
        .world
        .get(player)
        .and_then(|data| data.transform_3d)
        .expect("the player has a transform")
        .position;

    for position in positions(&run, &tagged(&run, "hazard")) {
        let dx = position[0] - player_position[0];
        let dy = position[1] - player_position[1];
        assert!(
            (dx * dx + dy * dy).sqrt() > 3.7,
            "an asteroid appeared on top of the player at {position:?}"
        );
    }
}

#[test]
fn reference_hazard_timing_and_damage_are_authored() {
    let run = Run::open().expect("the project opens");
    let beam = run
        .prefabs
        .get("prefabs/hazard-beam.prefab.json")
        .expect("the beam prefab ships")
        .root()
        .expect("the beam has a root");
    let properties = &beam.components["sindri.script"]["properties"];
    for (name, expected) in [
        ("warning", 1.05),
        ("active_for", 0.55),
        ("player_dps", 4.5),
        ("enemy_dps", 3.4),
        ("flare_enemy_dps", 1.2),
    ] {
        let actual = properties[name]
            .as_f64()
            .expect("the hazard value is numeric");
        assert!((actual - expected).abs() < 1.0e-6, "{name}: {actual}");
    }

    for (prefab, health, impact, children) in [
        ("hazard-asteroid-large.prefab.json", 5.0, 0.7, 3.0),
        ("hazard-asteroid-medium.prefab.json", 2.0, 0.55, 2.0),
        ("hazard-asteroid-small.prefab.json", 1.0, 0.4, 0.0),
    ] {
        let asteroid = run
            .prefabs
            .get(&format!("prefabs/{prefab}"))
            .expect("the asteroid prefab ships")
            .root()
            .expect("the asteroid has a root");
        let properties = &asteroid.components["sindri.script"]["properties"];
        assert_eq!(properties["health"].as_f64(), Some(health));
        assert_eq!(properties["impact_damage"].as_f64(), Some(impact));
        assert_eq!(properties["child_count"].as_f64(), Some(children));
    }
}
