use std::fs;

use sindri_core::{SceneDocument, SceneMigrator};

fn main() {
    let path = "examples/cube/assets/demo.scene.json";
    let raw = fs::read_to_string(path).expect("cube scene reads");
    let document = SceneDocument::from_json_migrated(&raw, &SceneMigrator::builtin())
        .expect("cube scene migrates");
    fs::write(path, document.to_canonical_json().expect("canonical scene"))
        .expect("cube scene writes");
}
