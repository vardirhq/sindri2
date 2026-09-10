//! A long fight still earns the quiet minute after it.
//!
//! The gap between bosses is measured from the end of the last fight. It used to
//! advance by one interval per boss sent, which only works while fights are
//! shorter than the interval -- and when they are not, the clock is already
//! overdue at the moment the boss dies and the next one lands on top of it.
//! Nothing errors: you simply never get the break the numbers say you get.

use orbital_last_stand::Run;

const STEP: f32 = 1.0 / 60.0;

fn step(run: &mut Run) {
    let notes = run.step(STEP);
    assert!(notes.is_empty(), "{notes:#?}");
    // Killing a boss offers an upgrade, and the chooser holds the run still
    // until something is picked. A test about elapsed time has to keep time
    // moving, so it takes whatever is on offer.
    let offers = run.active_named("upgrade");
    if let Some(first) = offers.first() {
        run.click(first);
    }
}

/// A run whose first boss is due almost immediately and whose gap is short, so
/// the schedule can be watched in seconds rather than minutes.
fn quick_boss_run(interval: f32) -> Run {
    let mut run = Run::open().expect("the project opens");
    let director = run.find("Director").expect("the Director");
    let properties = run
        .world
        .get_mut(director)
        .and_then(|data| data.components.get_mut("sindri.script"))
        .and_then(|script| script.get_mut("properties"))
        .and_then(serde_json::Value::as_object_mut)
        .expect("the Director's authored script properties");
    properties.insert("first_boss_at".to_owned(), serde_json::Value::from(2.0));
    properties.insert(
        "boss_interval".to_owned(),
        serde_json::Value::from(f64::from(interval)),
    );

    for _ in 0..6 {
        step(&mut run);
    }
    run.click("TitleStart");
    step(&mut run);
    run.set_board("hp", 100_000.0);
    run
}

/// Play until a boss is counted, then hold the fight open for a while and end
/// it.
///
/// The boss entity is removed and `boss_hp` held high by hand for the duration.
/// What is under test is the schedule the director keeps, which reads exactly
/// that board -- and an actually-fought twenty second boss kills the player,
/// which ends the run and proves nothing about pacing.
fn fight_and_kill(run: &mut Run, fight_seconds: f32) {
    let mut waited = 0;
    while run.board("boss_count") < 1.0 && waited < 3600 {
        step(run);
        waited += 1;
    }
    assert!(run.board("boss_count") >= 1.0, "no boss ever arrived");

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

    // A deliberately long fight: far longer than the interval, which is the
    // case the old schedule could not survive.
    for _ in 0..((fight_seconds * 60.0) as usize) {
        run.set_board("boss_hp", 100.0);
        step(run);
    }
    run.set_board("boss_hp", 0.0);
    step(run);
}

#[test]
fn a_long_fight_still_earns_the_quiet_after_it() {
    let interval = 6.0_f32;
    let mut run = quick_boss_run(interval);
    let first = run.board("boss_count");

    // Twenty seconds of fighting against a six second interval: more than three
    // intervals of overrun, which is the shape of a real boss fight against the
    // authored minute.
    fight_and_kill(&mut run, 20.0);
    let after_kill = run.board("boss_count");

    // Half the interval later there must still be no new boss.
    for _ in 0..((interval * 0.5 * 60.0) as usize) {
        step(&mut run);
    }
    assert_eq!(
        run.board("boss_count"),
        after_kill,
        "a new boss arrived within half an interval of the last one dying: the \
         overrun was never repaid"
    );
    assert!(
        after_kill > first,
        "the first boss was never counted ({first} -> {after_kill})"
    );

    // And past the interval, the next one does come: the gap is a delay, not a
    // stall.
    for _ in 0..((interval * 1.2 * 60.0) as usize) {
        step(&mut run);
        if run.board("boss_count") > after_kill {
            return;
        }
    }
    panic!(
        "no boss arrived: elapsed={:.1} boss_count={} run_state={} boss_hp={} \
         after_kill={after_kill}",
        run.board("elapsed"),
        run.board("boss_count"),
        run.board("run_state"),
        run.board("boss_hp")
    );
}
