//! The demo's asset manifest, kept honest.
//!
//! `assets/sindri.manifest.json` says what this project ships and what each file
//! is. It is committed, because a manifest generated at deploy time describes
//! whatever happened to be on the machine that ran the deploy, which is not a
//! promise about anything. Committing it makes the promise reviewable and makes
//! a stale one a failing test rather than a picture somebody notices.
//!
//! Regenerate with `SINDRI_UPDATE_ASSET_MANIFEST=1 cargo test -p sindri-cube`.

use std::{
    fs,
    path::{Path, PathBuf},
};

use sindri_assets::{AssetManifest, MANIFEST_FILE_NAME, MANIFEST_FORMAT_VERSION};
use sindri_core::AssetId;

const UPDATE_ENV: &str = "SINDRI_UPDATE_ASSET_MANIFEST";

fn assets() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("assets")
}

/// The manifest describes the assets that are actually there.
///
/// This is the check that costs nothing until it saves something: editing an
/// asset without regenerating the manifest would ship a promise about the file
/// that used to be there, and every load of it would fail verification.
#[test]
fn the_committed_manifest_matches_the_assets() {
    let built = AssetManifest::of_directory(&assets()).expect("the demo's assets read");
    let text = built.to_canonical_json().expect("a manifest serializes");
    let path = assets().join(MANIFEST_FILE_NAME);

    if std::env::var_os(UPDATE_ENV).is_some() {
        fs::write(&path, &text).expect("the manifest is writable");
        return;
    }

    let committed = fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!(
            "{} is missing ({error}); set {UPDATE_ENV} to write it",
            path.display()
        )
    });
    assert_eq!(
        committed,
        text,
        "{} is out of date; regenerate it with {UPDATE_ENV}=1",
        path.display()
    );
}

/// It is a manifest this build understands, and it describes the files the demo
/// scene actually names.
#[test]
fn the_manifest_covers_what_the_scene_references() {
    let path = assets().join(MANIFEST_FILE_NAME);
    let text = fs::read_to_string(&path).expect("the manifest is committed");
    let manifest = AssetManifest::from_json(&text).expect("the manifest parses");
    assert_eq!(manifest.format_version(), MANIFEST_FORMAT_VERSION);

    for name in ["demo.scene.json", "textures/badge.png"] {
        let id = AssetId::new(name).expect("a valid asset ID");
        let entry = manifest
            .get(&id)
            .unwrap_or_else(|| panic!("{name} is not in the manifest"));
        let bytes = fs::read(assets().join(name)).expect("the asset is there");
        assert_eq!(entry.bytes, bytes.len() as u64, "{name}");
        assert_eq!(manifest.verify(&id, &bytes), Ok(()), "{name}");
    }
}
