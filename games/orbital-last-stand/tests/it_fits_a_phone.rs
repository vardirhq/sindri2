//! The same build, on the screens people actually hold.
//!
//! A game that is only ever looked at in one window is a game whose layout is
//! a coincidence. These play the project at several shapes and check that what
//! a person needs to touch is still on the screen.

use orbital_last_stand::Run;

const STEP: f32 = 1.0 / 60.0;

/// Every viewport worth being sure about, widest to narrowest.
const SCREENS: [(&str, f32, f32); 5] = [
    ("desktop 16:9", 1920.0, 1080.0),
    ("laptop 16:10", 1440.0, 900.0),
    ("tablet portrait", 1536.0, 2048.0),
    ("phone portrait", 1080.0, 2400.0),
    ("phone landscape", 2400.0, 1080.0),
];

/// The overlay is two tall and as wide as the aspect ratio, so this is where
/// the edge of the screen is in the units a scene is authored in.
fn half_width(width: f32, height: f32) -> f32 {
    width / height
}

#[test]
fn every_screen_can_reach_the_start_button() {
    for (name, width, height) in SCREENS {
        let mut run = Run::open().expect("the project opens");
        run.viewport = (width, height);
        for _ in 0..6 {
            let notes = run.step(STEP);
            assert!(notes.is_empty(), "{name}: {notes:#?}");
        }

        let start = run.find("TitleStart").expect("a start button");
        let rect = run
            .screen_ui
            .rect(start)
            .unwrap_or_else(|| panic!("{name}: START is not laid out"));
        let limit = half_width(width, height);
        assert!(
            rect.center[0].abs() + rect.size[0] * 0.5 <= limit + 1.0e-4,
            "{name}: START runs off the side — reaches {:.3} of {limit:.3}",
            rect.center[0].abs() + rect.size[0] * 0.5
        );
        assert!(
            rect.center[1].abs() + rect.size[1] * 0.5 <= 1.0 + 1.0e-4,
            "{name}: START runs off the top or bottom"
        );

        // And it is still a button: pressed where it is drawn, at this size.
        run.click("TitleStart");
        assert_eq!(
            run.board("run_state"),
            1.0,
            "{name}: START did not start a run"
        );
    }
}

/// Three upgrade cards have to fit, and a phone is where they do not if they
/// were laid out in a row.
#[test]
fn the_upgrade_cards_fit_a_phone() {
    let (name, width, height) = SCREENS[3];
    let mut run = Run::open().expect("the project opens");
    run.viewport = (width, height);
    for _ in 0..6 {
        run.step(STEP);
    }
    run.click("TitleStart");

    // This is a layout test, not a combat-balance test. Give the run exactly
    // the authored XP requirement so the chooser is reached deterministically
    // even when progression pacing changes.
    run.set_board("cores", run.board("next_level"));
    let notes = run.step(STEP);
    assert!(notes.is_empty(), "{name}: {notes:#?}");
    assert_eq!(
        run.board("run_state"),
        2.0,
        "{name}: forced level-up did not offer an upgrade"
    );
    // Two frames: the chooser switches the cards on during a script pass, and
    // the pass that lays elements out has already run by then.
    run.step(STEP);
    run.step(STEP);

    let limit = half_width(width, height);
    let offers = run.active_named("upgrade");
    assert_eq!(offers.len(), 3, "{name}: {offers:?}");
    for offer in &offers {
        let card = run.find(offer).expect("an offered card");
        let rect = run
            .screen_ui
            .rect(card)
            .unwrap_or_else(|| panic!("{name}: {offer} is not laid out"));
        assert!(
            rect.center[0].abs() + rect.size[0] * 0.5 <= limit + 1.0e-4,
            "{name}: {offer} runs off the side"
        );
        assert!(
            rect.center[1].abs() + rect.size[1] * 0.5 <= 1.0 + 1.0e-4,
            "{name}: {offer} runs off the top or bottom — a column of three has \
             to fit the height it is stacked in"
        );
    }
}

/// The arena is the game. A camera framed by height alone takes the sides off
/// it the moment a screen is taller than it is wide.
#[test]
fn the_arena_is_whole_on_every_screen() {
    let arena = 7.5_f32;
    for (name, width, height) in SCREENS {
        let aspect = width / height;
        // What the scene's camera frames, worked out the way the extractor
        // does: the size lands on the shorter axis.
        let half = 17.0_f32 * 0.5;
        let half_height = if aspect < 1.0 { half / aspect } else { half };
        let half_width = half_height * aspect;
        assert!(
            half_width >= arena && half_height >= arena,
            "{name}: the arena is cut off — {half_width:.1} by {half_height:.1} \
             of the {arena:.1} it needs"
        );
    }
}
