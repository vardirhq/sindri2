//! Combat Lab proves attack primitives outside any boss script.
//! A boss should compose these entities, not reimplement their behavior.

use orbital_baked::Run;
use sindri_core::SceneDocument;

const STEP: f32 = 1.0 / 60.0;

fn step(run: &mut Run) {
    let notes = run.step(STEP);
    assert!(notes.is_empty(), "{notes:#?}");
}

fn lab_run() -> Run {
    let mut run = Run::open().expect("the project opens");
    for _ in 0..6 { step(&mut run); }
    run.click("TitleStart");
    step(&mut run);

    // The lab owns the arena while a test is using it. Normal spawning would
    // make counts nondeterministic and, more importantly, hide whether an attack
    // works on its own.
    let director = run.find("Director").expect("the director exists");
    run.world.get_mut(director).expect("director remains").disabled = true;

    let document = run
        .prefabs
        .get("prefabs/combat-lab.prefab.json")
        .expect("the combat lab prefab ships")
        .clone();
    run.world.spawn_prefab(&document).expect("lab spawns");
    step(&mut run);
    run.set_board("lab_autoplay", 0.0);
    run.set_board("lab_clear", 1.0);
    step(&mut run);
    run
}

#[test]
fn the_standalone_lab_scene_is_a_real_scene() {
    let text = std::fs::read_to_string(
        orbital_baked::project().join("assets/combat-lab.scene.json"),
    )
    .expect("the lab scene reads");
    let scene: SceneDocument = serde_json::from_str(&text).expect("the lab scene parses");
    scene.validate().expect("the lab scene validates");
}

#[test]
fn every_attack_can_be_spawned_independently() {
    let mut run = lab_run();
    for (kind, tag) in [
        (1.0, "attack_gas"),
        (2.0, "attack_mine"),
        (3.0, "attack_shockwave"),
        (4.0, "attack_gravity"),
    ] {
        run.set_board("lab_spawn", kind);
        step(&mut run);
        assert_eq!(run.count(tag), 1, "attack kind {kind} did not spawn");
        run.set_board("lab_clear", 1.0);
        step(&mut run);
        assert_eq!(run.count("lab_attack"), 0, "clear left attack kind {kind}");
    }
}

#[test]
fn combo_presets_use_the_same_attack_entities() {
    let mut run = lab_run();

    run.set_board("lab_combo", 1.0);
    step(&mut run);
    assert_eq!(run.count("attack_gas"), 1);
    assert_eq!(run.count("attack_gravity"), 1);

    run.set_board("lab_combo", 2.0);
    step(&mut run);
    assert_eq!(run.count("attack_mine"), 3);
    assert_eq!(run.count("attack_shockwave"), 1);

    run.set_board("lab_combo", 3.0);
    step(&mut run);
    assert_eq!(run.count("attack_gas"), 1);
    assert_eq!(run.count("attack_gravity"), 1);
    assert_eq!(run.count("attack_mine"), 2);
    assert_eq!(run.count("attack_shockwave"), 1);
    assert_eq!(run.count("lab_attack"), 5);

    // Let the combination actually run. A preset that merely spawns but starts
    // throwing script/physics errors a frame later is not a usable playground.
    for _ in 0..120 { step(&mut run); }
}
