use std::{fs, path::Path};

#[test]
fn export_refactor_sources_for_ci_artifact() {
    let editor = Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = editor
        .parent()
        .expect("editor crate lives under repository root");
    let out = root.join("target/render-artifacts");
    fs::create_dir_all(&out).expect("create refactor artifact directory");

    fs::copy(
        editor.join("src/native/app.rs"),
        out.join("browser-cube.png"),
    )
    .expect("copy native editor source");
    fs::copy(
        root.join("crates/sindri-scene/tests/extraction.rs"),
        out.join("browser-gather.png"),
    )
    .expect("copy extraction test source");
}
