use orbital_last_stand::Run;

const STEP: f32 = 1.0 / 60.0;

fn started_run() -> Run {
    let mut run = Run::open().expect("the project opens");
    for _ in 0..6 {
        assert!(run.step(STEP).is_empty());
    }
    run.click("TitleStart");
    assert!(run.step(STEP).is_empty());
    run
}

fn level_once(run: &mut Run) {
    run.set_board("cores", run.board("next_level"));
    let notes = run.step(STEP);
    assert!(notes.is_empty(), "{notes:#?}");
    assert_eq!(
        run.board("run_state"),
        2.0,
        "level-up did not pause the run"
    );
    assert_eq!(run.board("cores"), 0.0, "spent XP was not consumed");
    run.set_board("run_state", 1.0);
    assert!(run.step(STEP).is_empty());
}

#[test]
fn xp_thresholds_follow_the_reference_curve() {
    let mut run = started_run();
    assert_eq!(run.board("level"), 1.0);
    assert_eq!(run.board("next_level"), 35.0);

    let expected = [52.0, 74.0, 102.0, 138.0, 184.0, 243.0, 319.0, 416.0, 540.0];
    for (index, threshold) in expected.into_iter().enumerate() {
        level_once(&mut run);
        assert_eq!(run.board("level"), index as f32 + 2.0);
        assert_eq!(run.board("next_level"), threshold);
    }
}

#[test]
fn milestone_levels_publish_the_reference_module_pool() {
    let mut run = started_run();

    for level in 2..=14 {
        level_once(&mut run);
        let expected = if level % 7 == 0 {
            2.0
        } else if level % 5 == 0 {
            1.0
        } else {
            0.0
        };
        assert_eq!(
            run.board("module_pool"),
            expected,
            "wrong module pool at level {level}"
        );
    }
}
