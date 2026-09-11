//! Taking a module must not make the run start reporting errors.
//!
//! This exists because White Noise did. It calls `Effects.burst(this.player)`
//! every four seconds while held, and the player entity authored no burst -- so
//! the module worked, purged what it was meant to purge, and filled the script
//! log with `entity authors no burst` for the rest of the run. Nothing about
//! that is visible while playing: the effect it was asking for simply never
//! appeared, and the error went somewhere nobody was looking.

use orbital_last_stand::Run;

const STEP: f32 = 1.0 / 60.0;

/// Every board key a module writes, taken from `module-catalog.profile.json`,
/// switched on at once. If any of them asks the engine for something the scene
/// does not author, this is where it says so.
#[test]
fn holding_every_special_at_once_reports_nothing() {
    let mut run = Run::open().expect("the project opens");
    for _ in 0..6 {
        run.step(STEP);
    }
    run.click("TitleStart");
    run.step(STEP);

    let catalog = std::fs::read_to_string(
        orbital_last_stand::project().join("assets/profiles/module-catalog.profile.json"),
    )
    .expect("the module catalog ships");
    let catalog: serde_json::Value =
        serde_json::from_str(&catalog).expect("the module catalog parses");

    let mut switched = 0;
    for key in collect_board_keys(&catalog) {
        run.set_board(&key, 1.0);
        switched += 1;
    }
    assert!(switched > 5, "only {switched} module keys found");
    run.set_board("hp", 100_000.0);

    // Long enough for the slowest of them to come round: White Noise is on a
    // four second cycle, so ten seconds sees it several times.
    for tick in 0..600 {
        let notes = run.step(STEP);
        assert!(notes.is_empty(), "tick {tick}: {notes:#?}");
    }
}

/// Every `special_*` and `synergy_*` key the catalog mentions.
fn collect_board_keys(value: &serde_json::Value) -> Vec<String> {
    let mut found = Vec::new();
    fn walk(value: &serde_json::Value, found: &mut Vec<String>) {
        match value {
            serde_json::Value::String(text)
                if text.starts_with("special_") || text.starts_with("synergy_") =>
            {
                found.push(text.clone());
            }
            serde_json::Value::Array(items) => items.iter().for_each(|i| walk(i, found)),
            serde_json::Value::Object(map) => {
                for (key, item) in map {
                    if key.starts_with("special_") || key.starts_with("synergy_") {
                        found.push(key.clone());
                    }
                    walk(item, found);
                }
            }
            _ => {}
        }
    }
    walk(value, &mut found);
    found.sort();
    found.dedup();
    found
}
