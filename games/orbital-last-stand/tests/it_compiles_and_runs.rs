//! The first question: does the project load and do its scripts compile?

use orbital_last_stand::Run;

#[test]
fn the_project_opens_and_every_script_compiles() {
    let mut run = Run::open().expect("the project opens");
    let failures = run
        .scripts
        .compile(&run.world, &run.components, &run.sources);
    let reported: Vec<String> = failures.iter().map(ToString::to_string).collect();
    assert!(reported.is_empty(), "{reported:#?}");
}

#[test]
fn a_first_frame_reports_nothing() {
    let mut run = Run::open().expect("the project opens");
    let notes = run.step(1.0 / 60.0);
    assert!(notes.is_empty(), "{notes:#?}");
}
