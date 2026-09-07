//! The spectacle pass is authored gameplay, not a renderer special case.

use orbital_last_stand::Run;

const STEP: f32 = 1.0 / 60.0;

fn step(run: &mut Run) {
    let notes = run.step(STEP);
    assert!(notes.is_empty(), "{notes:#?}");
}

fn play(run: &mut Run, seconds: f32) {
    for _ in 0..(seconds / STEP) as usize {
        step(run);
    }
}

fn running() -> Run {
    let mut run = Run::open().expect("the project opens");
    play(&mut run, 0.1);
    run.click("TitleStart");
    step(&mut run);
    run.set_board("hp", 1000.0);
    run
}

fn disable(run: &mut Run, name: &str) {
    let entity = run.find(name).unwrap_or_else(|| panic!("{name} exists"));
    run.world
        .get_mut(entity)
        .unwrap_or_else(|| panic!("{name} remains"))
        .disabled = true;
}

fn spectacles(run: &Run) -> usize {
    run.world
        .entities()
        .filter(|(_, data)| data.name.as_deref() == Some("Spectacle"))
        .count()
}

#[test]
fn weapons_author_additive_flashes_and_pooled_trails() {
    let root = orbital_last_stand::project().join("assets");
    let prefab: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(root.join("prefabs/spectacle.prefab.json"))
            .expect("the spectacle prefab reads"),
    )
    .expect("the spectacle prefab parses");
    assert_eq!(
        prefab["entities"][0]["components"]["sindri.shape"]["blend"], "add",
        "flares need additive light rather than opaque geometry"
    );

    let scene: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(root.join("orbital.scene.json")).expect("the scene reads"),
    )
    .expect("the scene parses");
    let trails = scene["entities"]
        .as_array()
        .expect("entities")
        .iter()
        .filter(|entity| {
            entity["name"]
                .as_str()
                .is_some_and(|name| name.starts_with("Trail"))
        })
        .collect::<Vec<_>>();
    assert_eq!(trails.len(), 5, "each combat palette needs a pooled trail");
    assert!(
        trails.iter().all(|entity| {
            entity["components"]["sindri.effect.burst"]["count"]
                .as_u64()
                .is_some_and(|count| count <= 2)
        }),
        "a trail sample should stay tiny and cheap"
    );
}

#[test]
fn a_projectile_leaves_visuals_outside_the_entity_graph() {
    let mut run = running();
    disable(&mut run, "Director");
    disable(&mut run, "Player");
    run.effects.clear();
    let baseline = run.count("bullet");
    let document = run
        .prefabs
        .get("prefabs/bullet.prefab.json")
        .expect("the bullet prefab is loaded");
    run.world.spawn_prefab(document).expect("a bullet spawns");

    play(&mut run, 0.12);

    assert_eq!(run.count("bullet"), baseline + 1);
    assert!(
        run.effects.live() >= 2,
        "the projectile should leave pooled flecks behind"
    );
}

#[test]
fn camera_trauma_is_strong_then_gets_out_of_the_way() {
    let mut run = running();
    disable(&mut run, "Director");
    disable(&mut run, "Player");
    let camera = run.find("Camera").expect("the camera exists");

    run.set_board("screen_trauma", 0.8);
    step(&mut run);
    let shaken = run
        .world
        .get(camera)
        .unwrap()
        .transform_3d
        .unwrap()
        .position;
    assert!(shaken[0].abs() + shaken[1].abs() > 0.01);

    play(&mut run, 0.4);
    let settled = run
        .world
        .get(camera)
        .unwrap()
        .transform_3d
        .unwrap()
        .position;
    assert!(settled[0].abs() + settled[1].abs() < 0.001);
    assert_eq!(run.board("screen_trauma"), 0.0);
}

#[test]
fn unlocking_a_synergy_gets_a_three_ring_reveal() {
    let mut run = running();
    disable(&mut run, "Director");
    let before = spectacles(&run);

    run.set_board("shots_add", 1.0);
    run.set_board("missile", 1.0);
    step(&mut run);
    step(&mut run);

    assert_eq!(spectacles(&run), before + 3);
    assert!(run.board("screen_trauma") > 0.0);
}
