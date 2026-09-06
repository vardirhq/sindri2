use std::collections::HashSet;

use orbital_last_stand::Run;
use serde_json::json;
use sindri_core::ProfileDocument;

const STEP: f32 = 1.0 / 60.0;
const NAMES: &str = "FORKED GUIDANCE|CRITICAL CONDUCTION|MASS DRIVER|PHASE DISCHARGE|PRISMATIC PHASE|HUNTER'S SPARK|PHASE ESCORT|HEAVY ORBIT|VOID ECHO|STATIC AEGIS|SEEKING STORM|CRITICAL MASS|RAIL PRISM|RECURSIVE VIOLENCE|EVENT HORIZON|THUNDER CHOIR|PRISMATIC RAZOR|GUARDIAN SWARM|SINGULARITY COURT";

fn started_run() -> Run {
    let mut run = Run::open().expect("the project opens");
    for _ in 0..6 {
        assert!(run.step(STEP).is_empty());
    }
    run.click("TitleStart");
    assert!(run.step(STEP).is_empty());
    run
}

fn catalog() -> ProfileDocument {
    ProfileDocument::from_json(include_str!(
        "../assets/profiles/synergy-catalog.profile.json"
    ))
    .expect("the synergy catalog parses")
}

#[test]
fn the_reference_catalog_has_all_19_recipes() {
    let catalog = catalog();
    let synergies = catalog
        .value("synergies")
        .and_then(serde_json::Value::as_array)
        .expect("the catalog has synergies");
    let names: Vec<_> = NAMES.split('|').collect();
    let mut keys = HashSet::new();

    assert_eq!(synergies.len(), names.len());
    for (synergy, expected) in synergies.iter().zip(names) {
        assert_eq!(synergy["name"].as_str(), Some(expected));
        assert!(
            synergy["key"]
                .as_str()
                .is_some_and(|key| !key.is_empty() && keys.insert(key))
        );
        assert!(
            synergy["required_0"]
                .as_str()
                .is_some_and(|requirement| !requirement.is_empty())
        );
    }
}

#[test]
fn recipes_activate_and_deactivate_from_the_derived_build() {
    let mut run = started_run();
    run.set_board("shots_add", 1.0);
    run.set_board("missile", 1.0);
    assert!(run.step(STEP).is_empty());
    assert_eq!(run.board("synergy_forked_guidance"), 1.0);

    run.set_board("missile", 0.0);
    assert!(run.step(STEP).is_empty());
    assert_eq!(run.board("synergy_forked_guidance"), 0.0);
}

#[test]
fn apex_recipes_require_their_transform() {
    let mut run = started_run();
    run.set_board("shots_add", 1.0);
    run.set_board("pierce_add", 1.0);
    run.set_board("missile", 1.0);
    run.set_board("arc", 1.0);
    assert!(run.step(STEP).is_empty());
    assert_eq!(run.board("synergy_recursive_violence"), 0.0);

    run.set_board("transform_recursive", 1.0);
    assert!(run.step(STEP).is_empty());
    assert_eq!(run.board("synergy_recursive_violence"), 1.0);
}

#[test]
fn a_new_recipe_works_without_a_new_script_branch() {
    let mut run = started_run();
    let catalog = run
        .profiles
        .get_mut("profiles/synergy-catalog.profile.json")
        .expect("the catalog is loaded");
    catalog
        .values
        .get_mut("synergies")
        .and_then(serde_json::Value::as_array_mut)
        .expect("the synergies are a list")
        .push(json!({
            "key": "synergy_test_extension",
            "name": "TEST EXTENSION",
            "required_0": "extension_probe",
            "minimum_0": 7,
            "required_1": "",
            "minimum_1": 0,
            "required_2": "",
            "minimum_2": 0,
            "required_3": "",
            "minimum_3": 0,
            "required_4": "",
            "minimum_4": 0
        }));

    run.set_board("extension_probe", 7.0);
    assert!(run.step(STEP).is_empty());
    assert_eq!(run.board("synergy_test_extension"), 1.0);
    assert_eq!(run.board("synergy_count"), 1.0);
}
