//! The acceptance test the audit asks for: ten minutes, played through.
//!
//! `docs/orbital-last-stand-audit.md` says the workload should be comparable to
//! the reference game's rather than a quiet demo, so nothing here stands still:
//! the run is played to the boss and past it, taking every upgrade offered.

use std::time::Instant;

use orbital_last_stand::Run;

const STEP: f32 = 1.0 / 60.0;

/// One run, played, with what it cost recorded as it went.
struct Played {
    run: Run,
    /// The worst single step, in milliseconds.
    worst_step_ms: f64,
    /// The mean step, in milliseconds.
    mean_step_ms: f64,
    /// The most entities alive at once.
    peak_entities: usize,
    /// The most flecks alive at once.
    peak_flecks: usize,
    upgrades_taken: usize,
    steps: usize,
}

fn play_a_run(seconds: f32) -> Played {
    let mut run = Run::open().expect("the project opens");
    for _ in 0..6 {
        assert!(run.step(STEP).is_empty());
    }
    run.click("TitleStart");
    assert_eq!(run.board("run_state"), 1.0);

    let steps = (seconds / STEP) as usize;
    let mut worst = 0.0_f64;
    let mut total = 0.0_f64;
    let mut peak_entities = 0;
    let mut peak_flecks = 0;
    let mut upgrades = 0;

    for step in 0..steps {
        let began = Instant::now();
        let notes = run.step(STEP);
        let took = began.elapsed().as_secs_f64() * 1000.0;
        assert!(
            notes.is_empty(),
            "step {step} at {:.1}s: {notes:#?}",
            run.elapsed
        );

        worst = worst.max(took);
        total += took;
        peak_entities = peak_entities.max(run.world.len());
        peak_flecks = peak_flecks.max(run.effects.live());

        if run.board("run_state") == 2.0 {
            run.step(STEP);
            let offers = run.active_named("upgrade");
            assert_eq!(
                offers.len(),
                3,
                "three cards at {:.1}s: {offers:?}",
                run.elapsed
            );
            run.click(&offers[upgrades % offers.len()]);
            upgrades += 1;
        }
        // A run that ends early is not a ten-minute run, and the harness keeps
        // the ship alive rather than pretending it survived: the acceptance
        // test is about the engine carrying the workload, not about whether
        // this balance is beatable. Topped up every step rather than at a
        // threshold, because a late wave can take a full hull in one frame.
        run.set_board("hp", run.board("max_hp"));
    }

    Played {
        run,
        worst_step_ms: worst,
        mean_step_ms: total / steps as f64,
        peak_entities,
        peak_flecks,
        upgrades_taken: upgrades,
        steps,
    }
}

/// 4, 5, 8 and 12 together: ten minutes of churn, a boss, and the evidence.
#[test]
#[ignore = "ten simulated minutes; run it with --ignored"]
fn ten_minutes_hold_together() {
    let played = play_a_run(600.0);

    assert!(
        played.run.board("kills") > 300.0,
        "kills: {}",
        played.run.board("kills")
    );
    assert!(
        played.upgrades_taken >= 5,
        "upgrades: {}",
        played.upgrades_taken
    );
    assert!(
        played.run.saves.flag("beat_warden", false),
        "the boss was never beaten in ten minutes"
    );

    println!(
        "\n  ten-minute run\n\
         \x20   steps            {}\n\
         \x20   kills            {}\n\
         \x20   score            {}\n\
         \x20   waves            {}\n\
         \x20   upgrades taken   {}\n\
         \x20   peak entities    {}\n\
         \x20   peak flecks      {}\n\
         \x20   mean step        {:.3} ms\n\
         \x20   worst step       {:.3} ms\n",
        played.steps,
        played.run.board("kills"),
        played.run.board("score"),
        played.run.board("wave"),
        played.upgrades_taken,
        played.peak_entities,
        played.peak_flecks,
        played.mean_step_ms,
        played.worst_step_ms,
    );

    // A fixed step is 16.67 ms. A mean above it means the simulation cannot
    // keep up with itself, which is the only threshold here that is a fact
    // rather than a measurement of this machine.
    assert!(
        played.mean_step_ms < 16.6,
        "the simulation cannot keep up: {:.3} ms a step",
        played.mean_step_ms
    );
}

/// 5. One multi-phase boss, reached and fought.
#[test]
fn the_warden_arrives_and_changes_as_it_is_fought() {
    let mut run = Run::open().expect("the project opens");
    for _ in 0..6 {
        run.step(STEP);
    }
    run.click("TitleStart");

    // Straight to the boss rather than waiting three minutes for it: the
    // director sends it on a clock, and what is being checked is the fight.
    let mut seen_phases = Vec::new();
    let mut fought = false;
    for _ in 0..(200.0 / STEP) as usize {
        let notes = run.step(STEP);
        assert!(notes.is_empty(), "{notes:#?}");
        if run.board("run_state") == 2.0 {
            run.step(STEP);
            let offers = run.active_named("upgrade");
            run.click(&offers[0]);
        }
        if run.board("hp") <= 1.0 {
            run.set_board("hp", run.board("max_hp"));
        }
        let max = run.board("boss_max");
        if max > 0.0 {
            fought = true;
            let share = run.board("boss_hp") / max;
            let phase = if share <= 0.34 {
                3
            } else if share <= 0.67 {
                2
            } else {
                1
            };
            if seen_phases.last() != Some(&phase) {
                seen_phases.push(phase);
            }
        }
    }

    assert!(fought, "the Warden never arrived");
    assert!(
        seen_phases.len() >= 2,
        "the fight never changed phase: {seen_phases:?}"
    );
}
