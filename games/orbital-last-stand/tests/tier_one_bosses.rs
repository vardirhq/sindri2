use orbital_last_stand::Run;
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

fn spawn_challenger(run: &mut Run, kind: f32, y: f32) -> EntityId {
    let document = run
        .prefabs
        .get("prefabs/challenger.prefab.json")
        .expect("the tier-one boss prefab ships");
    let entity = run.world.spawn_prefab(document).expect("boss spawns").root;
    let data = run.world.get_mut(entity).expect("boss remains");
    data.transform_3d.as_mut().expect("boss transform").position = [0.0, y, 0.0];
    data.components["sindri.script"]["properties"]
        .as_object_mut()
        .expect("script properties")
        .insert("kind".to_owned(), json!(kind));
    entity
}

#[test]
fn every_tier_one_challenger_initializes_and_attacks() {
    for kind in 0..3 {
        let mut run = isolated_run();
        spawn_challenger(&mut run, kind as f32, 3.5);
        for _ in 0..(5.0 / STEP) as usize {
            step(&mut run);
        }
        assert_eq!(run.board("boss_kind"), kind as f32 + 1.0);
        assert!(run.board("boss_max") > 0.0, "kind {kind} has no health");
        assert!(
            run.count("hostile_shot") > 0,
            "kind {kind} produced no recognizable attack"
        );
    }
}

#[test]
fn an_offscreen_challenger_only_approaches() {
    let mut run = isolated_run();
    spawn_challenger(&mut run, 1.0, 20.0);
    for _ in 0..(2.0 / STEP) as usize {
        step(&mut run);
    }
    assert_eq!(
        run.count("hostile_shot"),
        0,
        "off-screen bosses must not attack or target the player"
    );
}

#[test]
fn the_director_authors_both_tier_one_prefabs() {
    let run = Run::open().expect("the project opens");
    let director = run.find("Director").expect("the director exists");
    let properties = &run.world.get(director).expect("director").components["sindri.script"]
        ["properties"];
    assert_eq!(
        properties["warden"].as_str(),
        Some("prefabs/warden.prefab.json")
    );
    assert_eq!(
        properties["challenger"].as_str(),
        Some("prefabs/challenger.prefab.json")
    );
}
