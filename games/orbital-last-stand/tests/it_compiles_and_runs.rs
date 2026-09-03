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

/// Every label has to be able to draw.
///
/// A screen element's position and scale are in the overlay's units — two tall,
/// centred on the origin — and its font size is not: that is in pixels. This
/// whole scene was authored in overlay units, so every label was set to a
/// fraction of a pixel, and the game shipped to a page that drew its buttons
/// and none of its words. Nothing failed; there was simply nothing there.
#[test]
fn every_label_is_big_enough_to_appear() {
    let text =
        std::fs::read_to_string(orbital_last_stand::project().join("assets/orbital.scene.json"))
            .expect("the scene reads");
    let scene: serde_json::Value = serde_json::from_str(&text).expect("the scene parses");
    let entities = scene["entities"].as_array().expect("entities");

    let mut labels = 0;
    for entity in entities {
        let Some(label) = entity["components"].get("sindri.ui.text") else {
            continue;
        };
        labels += 1;
        let id = entity["id"].as_str().unwrap_or("?");
        // The engine refuses anything under a pixel; a label a person is meant
        // to read on a phone needs considerably more than one.
        for field in ["font_size", "line_height"] {
            let value = label[field].as_f64().unwrap_or(0.0);
            assert!(
                value >= 12.0,
                "{id}: {field} is {value}, which is pixels — overlay units are what \
                 the transform takes, not this"
            );
        }
    }
    assert!(
        labels > 20,
        "only {labels} labels; the scene lost most of them"
    );
}
