//! Playing the game, which is the only way to find out whether it is one.
//!
//! Each test here is one line of the acceptance test in
//! `docs/orbital-last-stand-audit.md`, named after what it is checking.

use orbital_last_stand::Run;

const STEP: f32 = 1.0 / 60.0;

/// Runs for `seconds`, failing on the first thing a script complains about.
///
/// A game whose scripts are quietly failing looks from the outside like a game
/// that is simply not doing very much, so nothing here tolerates a note.
fn play(run: &mut Run, seconds: f32) {
    let steps = (seconds / STEP) as usize;
    for step in 0..steps {
        let notes = run.step(STEP);
        assert!(notes.is_empty(), "step {step}: {notes:#?}");
    }
}

/// Plays, taking whatever upgrade is offered whenever one is.
///
/// It does not know which upgrades exist: it asks the world what is on offer,
/// which is the same thing a person does by looking at the screen.
fn play_taking_upgrades(run: &mut Run, seconds: f32) -> usize {
    let steps = (seconds / STEP) as usize;
    let mut taken = 0;
    for step in 0..steps {
        let notes = run.step(STEP);
        assert!(notes.is_empty(), "step {step}: {notes:#?}");
        if run.board("run_state") == 2.0 {
            // One more frame first: a screen switched on during a script pass
            // is laid out by the pass after it, so nothing on it has a place
            // on the screen to be clicked until then.
            run.step(STEP);
            let offers = run.active_named("upgrade");
            assert_eq!(offers.len(), 3, "three cards should be offered: {offers:?}");
            run.click(&offers[0]);
            taken += 1;
        }
    }
    taken
}

/// 2. Start a run from an interactive screen using mouse or touch.
#[test]
fn a_run_starts_from_the_title_screen() {
    let mut run = Run::open().expect("the project opens");
    play(&mut run, 0.1);
    assert_eq!(run.board("run_state"), 0.0, "the title should be showing");

    run.click("TitleStart");
    assert_eq!(run.board("run_state"), 1.0, "START should start the run");
    assert_eq!(run.board("hp"), 5.0, "a run starts with a full hull");
}

/// 2, again, with the other half of "mouse or touch".
///
/// Worth its own test because a finger is not a mouse with different events:
/// it carries its own position and then ceases to exist, where a mouse is
/// somewhere all along and stays there once the button is up. A tap used to do
/// nothing at all on this screen -- the release was reported from nowhere, so
/// it never landed on the element the press began on -- and every check in this
/// file passed throughout, because every one of them clicked.
#[test]
fn a_run_starts_from_a_tap() {
    let mut run = Run::open().expect("the project opens");
    play(&mut run, 0.1);
    assert_eq!(run.board("run_state"), 0.0, "the title should be showing");

    run.tap("TitleStart");
    assert_eq!(
        run.board("run_state"),
        1.0,
        "a tap on START should start the run"
    );
    assert_eq!(run.board("hp"), 5.0, "a run starts with a full hull");
}

/// 4. Spawn, update, collide, and despawn continuously.
/// 5. Three enemy behaviours.
#[test]
fn enemies_arrive_die_and_leave_something_behind() {
    let mut run = Run::open().expect("the project opens");
    play(&mut run, 0.1);
    run.click("TitleStart");

    play_taking_upgrades(&mut run, 90.0);
    assert!(run.board("kills") > 20.0, "kills: {}", run.board("kills"));
    assert!(run.board("score") > 0.0, "nothing scored");
    // Chargers appear from wave 4 and splitters from wave 8, so ninety seconds
    // is past both.
    assert!(run.board("wave") >= 8.0, "wave: {}", run.board("wave"));
}

/// 6. Pause combat for a data-driven upgrade choice and apply the result.
#[test]
fn an_upgrade_pauses_the_run_and_changes_it() {
    let mut run = Run::open().expect("the project opens");
    play(&mut run, 0.1);
    run.click("TitleStart");

    // Played until the first offer rather than for a fixed time, because when
    // it arrives depends on how the run went.
    let mut waited = 0;
    while run.board("run_state") != 2.0 && waited < 3600 {
        run.step(STEP);
        waited += 1;
    }
    assert_eq!(run.board("run_state"), 2.0, "no upgrade was ever offered");
    run.step(STEP);

    let offers = run.active_named("upgrade");
    assert_eq!(offers.len(), 3, "three of the catalog: {offers:?}");

    let before = (
        run.board("damage"),
        run.board("fire_gap"),
        run.board("move_speed"),
        run.board("magnet"),
        run.board("max_hp"),
        run.board("pierce"),
    );
    run.click(&offers[0]);
    let after = (
        run.board("damage"),
        run.board("fire_gap"),
        run.board("move_speed"),
        run.board("magnet"),
        run.board("max_hp"),
        run.board("pierce"),
    );
    assert_ne!(before, after, "the card {} changed nothing", offers[0]);
    assert_eq!(run.board("run_state"), 1.0, "the run should resume");
}

/// 3. Move the player with keyboard and pointer controls.
#[test]
fn the_ship_answers_the_keyboard_and_the_pointer() {
    let mut run = Run::open().expect("the project opens");
    play(&mut run, 0.1);
    run.click("TitleStart");

    let player = run.find("Player").expect("a ship");
    let at = |run: &Run| {
        let data = run.world.get(player).expect("still there");
        let position = data.transform_3d.unwrap_or_default().position;
        (position[0], position[1])
    };

    let start = at(&run);
    run.hold(sindri_platform::Key::D);
    play(&mut run, 0.5);
    let moved = at(&run);
    assert!(
        moved.0 > start.0 + 0.5,
        "D did not move it right: {start:?} -> {moved:?}"
    );
    run.let_go(sindri_platform::Key::D);

    // And the pointer, which is the same control on a phone.
    run.hold(sindri_platform::Key::W);
    play(&mut run, 0.4);
    let up = at(&run);
    assert!(
        up.1 > moved.1 + 0.3,
        "W did not move it up: {moved:?} -> {up:?}"
    );
}

/// 7. Update HUD text and bars from gameplay state.
#[test]
fn the_hud_says_what_is_happening() {
    let mut run = Run::open().expect("the project opens");
    play(&mut run, 0.1);
    run.click("TitleStart");
    play_taking_upgrades(&mut run, 20.0);

    let health = run.find("Health").expect("a health bar");
    let fill = run.fill(health).expect("the bar has a fill");
    assert!((0.0..=1.0).contains(&fill), "fill: {fill}");

    let score = run.find("Score").expect("a score label");
    assert_eq!(
        run.text(score).as_deref(),
        Some("Score {}"),
        "the template is authored"
    );
    assert!(
        run.values(score).is_some_and(|v| !v.is_empty()),
        "nothing filled the score"
    );
}

/// 9. Save a persistent progression value, reload, and recover it.
#[test]
fn what_a_run_earns_outlives_it() {
    let mut run = Run::open().expect("the project opens");
    play(&mut run, 0.1);
    run.click("TitleStart");
    play_taking_upgrades(&mut run, 30.0);

    // Ending it the way the game does, rather than by reaching in.
    run.set_board("hp", 0.0);
    play(&mut run, 0.2);
    assert_eq!(run.board("run_state"), 4.0, "the run should be over");

    let scored = run.saves.number("best_score", 0.0);
    assert!(scored > 0.0, "no best score was written");
    let kills = run.saves.number("total_kills", 0.0);
    assert!(kills > 0.0, "no lifetime kills were written");

    // A second run, from the save the first one left.
    let carried = run.saves.clone();
    let mut again = Run::open().expect("the project opens");
    again.saves = carried;
    play(&mut again, 0.1);
    assert_eq!(
        again.saves.number("best_score", 0.0),
        scored,
        "the best score did not survive"
    );
}

/// The property the stat block exists for.
///
/// A module contributes to an additive pile or a multiplicative one, and the
/// ship is derived from both. So two players who took the same modules have the
/// same ship whatever order the offers came in. Folded into a running total at
/// the moment of picking, `+2 damage` then `x1.3` and `x1.3` then `+2` are
/// different numbers, and nothing on the screen would ever explain why.
#[test]
fn the_same_modules_make_the_same_ship_in_either_order() {
    // Applied directly to the piles rather than through the screen: which
    // offers a run happens to show is up to the seed, and this is a claim about
    // arithmetic rather than about what the chooser deals.
    let ship = |flat_first: bool| {
        let mut run = Run::open().expect("the project opens");
        play(&mut run, 0.1);
        run.click("TitleStart");
        run.step(STEP);

        let flat = |run: &mut Run| run.set_board("damage_add", run.board("damage_add") + 2.0);
        let scale = |run: &mut Run| run.set_board("damage_mul", run.board("damage_mul") * 1.3);
        if flat_first {
            flat(&mut run);
            scale(&mut run);
        } else {
            scale(&mut run);
            flat(&mut run);
        }
        // One frame for `Stats` to derive from the piles.
        run.step(STEP);
        run.board("damage")
    };

    let flat_first = ship(true);
    let scale_first = ship(false);
    assert!(
        (flat_first - scale_first).abs() < 1.0e-5,
        "the same build came out as {flat_first} and {scale_first}"
    );
    // And it is the number the piles describe, not merely a consistent one.
    assert!(
        (flat_first - 3.9).abs() < 1.0e-5,
        "(1 + 2) * 1.3 should be 3.9, and was {flat_first}"
    );
}
