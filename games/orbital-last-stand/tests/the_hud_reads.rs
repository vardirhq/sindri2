//! The HUD says what it is for, in the place it was laid out.
//!
//! Both of these guard the same class of mistake: a meter whose parts drift
//! apart, or a bar that moves too little to notice. Neither fails loudly on its
//! own -- a bar with its track somewhere else still draws, and a bar that grows
//! three percent still grows -- so they are asserted rather than looked at.

use orbital_last_stand::Run;
use sindri_core::EntityId;

const STEP: f32 = 1.0 / 60.0;

fn started() -> Run {
    let mut run = Run::open().expect("the project opens");
    for _ in 0..6 {
        run.step(STEP);
    }
    run.click("TitleStart");
    for _ in 0..30 {
        run.step(STEP);
    }
    run
}

fn named(run: &Run, name: &str) -> EntityId {
    run.find(name)
        .unwrap_or_else(|| panic!("the HUD has no {name}"))
}

fn position(run: &Run, entity: EntityId) -> [f32; 3] {
    run.world
        .get(entity)
        .and_then(|data| data.transform_3d.as_ref())
        .expect("the element has a transform")
        .position
}

/// A meter is two elements: the part that grows and the track showing how far
/// it has to grow. They only read as one thing while they are in one place.
///
/// They were not. `hud.decay` set three hardcoded `position.y` values in
/// `start`, moving both fills and the health label to the top of the screen
/// while their backs stayed where the scene put them -- so the health bar had no
/// track behind it, the label sat on top of the sector name, and filling the
/// cores bar showed nothing, because the growing part and the part that gives it
/// a scale were at opposite ends of the screen.
#[test]
fn every_meter_sits_on_its_own_track() {
    let run = started();
    for (fill, back) in [("Health", "HealthBack"), ("Cores", "CoresBack")] {
        let fill_at = position(&run, named(&run, fill));
        let back_at = position(&run, named(&run, back));
        assert!(
            (fill_at[0] - back_at[0]).abs() < 0.001 && (fill_at[1] - back_at[1]).abs() < 0.001,
            "{fill} is at {fill_at:?} and {back} at {back_at:?}: a bar and its \
             track have to be in the same place to read as one meter"
        );
    }
}

/// A core is worth one and a level costs thirty-five, so a pickup moves the
/// cores bar by about a thirtieth of its width. That is real progress and far
/// too little to see mid-fight, so the bar swells when it is fed; without that
/// the first several cores of every level look like nothing happening.
#[test]
fn taking_a_core_shows_on_the_bar_immediately() {
    let mut run = started();
    let cores = named(&run, "Cores");
    let resting = run
        .world
        .get(cores)
        .and_then(|data| data.transform_3d.as_ref())
        .expect("the cores bar has a transform")
        .scale[1];

    run.set_board("cores", run.board("cores") + 1.0);
    run.step(STEP);

    let flashed = run
        .world
        .get(cores)
        .and_then(|data| data.transform_3d.as_ref())
        .expect("the cores bar survives")
        .scale[1];
    assert!(
        flashed > resting * 1.2,
        "a core moved the bar from {resting:.4} to {flashed:.4}: nothing a \
         player would notice"
    );

    // And it settles, rather than staying swollen for the rest of the run.
    for _ in 0..40 {
        run.step(STEP);
    }
    let settled = run
        .world
        .get(cores)
        .and_then(|data| data.transform_3d.as_ref())
        .expect("the cores bar survives")
        .scale[1];
    assert!(
        (settled - resting).abs() < 0.001,
        "the bar stayed swollen at {settled:.4} instead of returning to {resting:.4}"
    );
}
