//! Boss rush: the mode that exists so the bosses can actually be seen.
//!
//! A normal run shows four bosses in its first four minutes, chosen at random
//! from a pool that only opens up with the clock. Somebody who wants to look at
//! the Aegis has no way to ask for it. This mode walks the whole roster in
//! order, from whichever boss the title screen is showing.

use orbital_baked::Run;

const STEP: f32 = 1.0 / 60.0;

fn step(run: &mut Run) {
    let notes = run.step(STEP);
    assert!(notes.is_empty(), "{notes:#?}");
    // The chooser holds the run still, and a rush is mostly kills.
    let offers = run.active_named("upgrade");
    if let Some(first) = offers.first() {
        run.click(first);
    }
}

fn at_the_title() -> Run {
    let mut run = Run::open().expect("the project opens");
    for _ in 0..6 {
        step(&mut run);
    }
    run
}

/// Run until a boss is on the field, then report which one.
fn await_boss(run: &mut Run) -> f32 {
    for _ in 0..1200 {
        step(run);
        if run.board("boss_hp") > 0.0 {
            return run.board("boss_kind");
        }
    }
    panic!("no boss arrived");
}

fn clear_the_boss(run: &mut Run) {
    let bosses: Vec<_> = run
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
    for boss in bosses {
        run.world.despawn_recursive(boss).expect("boss despawns");
    }
    run.set_board("boss_hp", 0.0);
    step(run);
}

/// The whole point: press it and the roster arrives, in order.
#[test]
fn a_rush_walks_the_roster_in_order() {
    let mut run = at_the_title();
    run.click("TitleRush");
    run.set_board("hp", 100_000.0);

    let mut seen: Vec<f32> = Vec::new();
    for _ in 0..4 {
        seen.push(await_boss(&mut run));
        run.set_board("hp", 100_000.0);
        clear_the_boss(&mut run);
    }
    assert_eq!(
        seen,
        vec![0.0, 1.0, 2.0, 3.0],
        "a rush should open on the Warden and walk forward, not shuffle"
    );
}

/// And it starts where the picker was left, so a particular boss can be asked
/// for rather than waited for.
#[test]
fn the_picker_chooses_where_the_rush_opens() {
    let mut run = at_the_title();
    // Eleven presses from the Warden is the Aegis, the last of the roster.
    for _ in 0..11 {
        run.click("TitlePick");
        step(&mut run);
    }
    assert_eq!(
        run.board("rush_from"),
        11.0,
        "the picker walked to the Aegis"
    );

    run.click("TitleRush");
    run.set_board("hp", 100_000.0);
    assert_eq!(
        await_boss(&mut run),
        11.0,
        "the rush should open on the boss the title was showing"
    );
}

/// The picker wraps, so it is never a dead end.
#[test]
fn the_picker_wraps() {
    let mut run = at_the_title();
    for _ in 0..12 {
        run.click("TitlePick");
        step(&mut run);
    }
    assert_eq!(
        run.board("rush_from"),
        0.0,
        "twelve presses is a full circle"
    );
}

/// A rush is bosses. Ordinary enemies would only be in the way of looking at
/// one, and the director is normally very willing to provide them.
#[test]
fn a_rush_does_not_fill_the_field_with_ordinary_enemies() {
    let mut run = at_the_title();
    run.click("TitleRush");
    run.set_board("hp", 100_000.0);
    await_boss(&mut run);

    let mut worst = 0;
    for _ in 0..600 {
        step(&mut run);
        worst = worst.max(run.count("enemy"));
    }
    assert!(
        worst <= 4,
        "a rush had {worst} enemies on the field at once: the ordinary spawner \
         is still running"
    );
}

/// START is an ordinary run; the rush button is the only thing that asks for a
/// rush. Checked from the title rather than by returning to it, because the
/// director hides the title screen while a run is going and a hidden button
/// cannot be pressed.
#[test]
fn start_asks_for_an_ordinary_run_and_rush_asks_for_a_rush() {
    let mut run = at_the_title();
    run.click("TitleStart");
    step(&mut run);
    assert_eq!(run.board("mode"), 0.0, "START is an ordinary run");

    let mut other = at_the_title();
    other.click("TitleRush");
    step(&mut other);
    assert_eq!(other.board("mode"), 1.0, "BOSS RUSH asks for a rush");
}
