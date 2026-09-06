use orbital_last_stand::Run;
use sindri_core::EntityId;

const STEP: f32 = 1.0 / 60.0;

fn play(run: &mut Run, seconds: f32) {
    let steps = (seconds / STEP) as usize;
    for step in 0..steps {
        let notes = run.step(STEP);
        assert!(notes.is_empty(), "step {step}: {notes:#?}");
    }
}

fn isolated_run() -> Run {
    let mut run = Run::open().expect("the project opens");
    play(&mut run, 0.1);
    run.click("TitleStart");
    play(&mut run, STEP);
    run.set_board("hp", 1000.0);
    let director = run.find("Director").expect("the campaign director exists");
    run.world
        .get_mut(director)
        .expect("the campaign director remains")
        .disabled = true;
    run
}

fn position(run: &Run, entity: EntityId) -> [f32; 3] {
    run.world
        .get(entity)
        .and_then(|data| data.transform_3d)
        .expect("the entity has a transform")
        .position
}

fn angle(run: &Run, entity: EntityId) -> f32 {
    run.world
        .get(entity)
        .and_then(|data| data.transform_3d)
        .expect("the entity has a transform")
        .rotation_z_radians()
}

/// The camera is rectangular, especially on a phone. Movement must use that
/// same rectangle instead of an invisible circular arena: the complete ship
/// reaches every edge, cannot leave through a side, and does not bog down near
/// a portrait corner.
#[test]
fn the_ship_stays_inside_the_portrait_viewport_rectangle() {
    let mut run = isolated_run();
    run.viewport = (390.0, 844.0);
    let player = run.find("Player").expect("a ship");

    run.hold(sindri_platform::Key::D);
    play(&mut run, 1.5);
    run.let_go(sindri_platform::Key::D);
    let right = position(&run, player);
    assert!(
        (5.1..=5.23).contains(&right[0]),
        "the portrait side did not match the visible camera edge: {right:?}"
    );

    run.hold(sindri_platform::Key::S);
    play(&mut run, 2.8);
    run.let_go(sindri_platform::Key::S);
    let corner = position(&run, player);
    assert!(
        (-11.65..=-11.35).contains(&corner[1]),
        "the ship stopped short of the portrait bottom: {corner:?}"
    );
    assert!(
        (5.1..=5.23).contains(&corner[0]),
        "moving vertically let the ship escape or slide off the side: {corner:?}"
    );
}

/// An idle Strider keeps its last heading, while its independently animated
/// shield continues to turn around it.
#[test]
fn the_ship_keeps_its_heading_and_animated_shield() {
    let mut run = isolated_run();
    let player = run.find("Player").expect("a ship");
    let shield = run.find("Player Shield").expect("the Strider shield");

    run.hold(sindri_platform::Key::D);
    play(&mut run, 0.6);
    run.let_go(sindri_platform::Key::D);
    let moving_heading = angle(&run, player);
    let shield_before = angle(&run, shield);
    play(&mut run, 0.3);
    let idle_heading = angle(&run, player);
    let shield_after = angle(&run, shield);

    assert!(moving_heading < -1.0, "the ship did not turn right");
    assert!(
        (idle_heading - moving_heading).abs() < 0.02,
        "the idle ship forgot its heading: {moving_heading} -> {idle_heading}"
    );
    assert!(
        (shield_after - shield_before).abs() > 0.2,
        "the shield did not rotate independently"
    );
}
