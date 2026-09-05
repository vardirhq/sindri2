use orbital_last_stand::Run;
use sindri_platform::Key;

const STEP: f32 = 1.0 / 60.0;

#[track_caller]
fn near(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < 1.0e-5,
        "expected {expected}, got {actual}"
    );
}

fn step(run: &mut Run) {
    let notes = run.step(STEP);
    assert!(notes.is_empty(), "{notes:#?}");
}

#[test]
fn the_player_is_the_authored_strider_hull_not_a_regular_hexagon() {
    let mut run = Run::open().expect("the project opens");
    step(&mut run);

    let player = run.find("Player").expect("the player exists");
    let data = run.world.get(player).expect("the player remains");
    let shape = &data.components["sindri.shape"];
    let points = shape["points"]
        .as_array()
        .expect("the Strider has authored polygon points");
    let expected = [
        [0.0, 0.5],
        [0.304, -0.328],
        [0.1, -0.248],
        [0.0, -0.392],
        [-0.1, -0.248],
        [-0.304, -0.328],
    ];
    assert_eq!(points.len(), expected.len());
    for (point, expected) in points.iter().zip(expected) {
        let point = point.as_array().expect("a polygon point is a pair");
        near(point[0].as_f64().expect("point X is numeric"), expected[0]);
        near(point[1].as_f64().expect("point Y is numeric"), expected[1]);
    }

    let fill = shape["fill"].as_array().expect("the hull has a fill");
    near(fill[0].as_f64().expect("red"), 0.470_588);
    near(fill[1].as_f64().expect("green"), 0.921_569);
    near(fill[2].as_f64().expect("blue"), 1.0);
    let stroke = shape["stroke"].as_array().expect("the hull has a stroke");
    near(stroke[0].as_f64().expect("red"), 0.913_725);
    near(stroke[1].as_f64().expect("green"), 0.992_157);
    near(stroke[2].as_f64().expect("blue"), 1.0);
}

#[test]
fn the_strider_turns_toward_movement_instead_of_aiming_at_its_target() {
    let mut run = Run::open().expect("the project opens");
    step(&mut run);
    run.click("TitleStart");

    let player = run.find("Player").expect("the player exists");
    let before = run
        .world
        .get(player)
        .expect("the player remains")
        .transform_3d
        .expect("the player has a transform")
        .rotation;

    run.hold(Key::D);
    step(&mut run);
    let after = run
        .world
        .get(player)
        .expect("the player remains")
        .transform_3d
        .expect("the player has a transform")
        .rotation;
    run.let_go(Key::D);

    assert_ne!(after, before, "moving right must visibly turn the Strider");
    assert!(
        after[2] < 0.0,
        "rightward movement should turn the up-authored hull clockwise: {after:?}"
    );
}
