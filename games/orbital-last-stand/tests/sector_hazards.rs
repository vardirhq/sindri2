use orbital_last_stand::Run;

const STEP: f32 = 1.0 / 60.0;

fn step(run: &mut Run) {
    let notes = run.step(STEP);
    assert!(notes.is_empty(), "{notes:#?}");
}

#[test]
fn the_five_sectors_swap_real_hazards() {
    let mut run = Run::open().expect("the project opens");
    for _ in 0..6 {
        step(&mut run);
    }
    run.click("TitleStart");
    step(&mut run);
    run.set_board("hp", 1000.0);

    assert_eq!(run.board("sector"), 1.0);
    assert_eq!(run.count("hazard"), 7, "Outer Drift starts with seven rocks");

    let director = run.find("Director").expect("the campaign director exists");
    run.world
        .get_mut(director)
        .expect("the campaign director remains")
        .disabled = true;

    run.set_board("sector", 2.0);
    step(&mut run);
    assert_eq!(run.count("hazard"), 2, "Ember Belt owns two edge flares");

    run.set_board("sector", 3.0);
    step(&mut run);
    assert_eq!(run.count("hazard"), 3, "Violet Wake owns three gravity wells");

    run.set_board("sector", 4.0);
    step(&mut run);
    assert_eq!(run.count("hazard"), 1, "Null Lattice owns one roaming field");

    let field = run.find("Hazard Field").expect("the null field exists");
    let field_position = run
        .world
        .get(field)
        .and_then(|data| data.transform_3d)
        .expect("the null field has a transform")
        .position;
    let player = run.find("Player").expect("the player exists");
    run.world
        .get_mut(player)
        .and_then(|data| data.transform_3d.as_mut())
        .expect("the player has a transform")
        .position = field_position;
    step(&mut run);
    assert_eq!(run.board("nullified"), 1.0, "the field should suppress weapons");

    run.set_board("sector", 5.0);
    step(&mut run);
    assert_eq!(run.board("nullified"), 0.0, "leaving Null Lattice restores weapons");
    assert_eq!(run.count("hazard"), 1, "Core Approach immediately queues a beam");
}
