//! Exporting a real project into a directory a static host could serve.

use std::path::{Path, PathBuf};

use sindri_assets::{AssetKind, AssetManifest};
use sindri_export::ProjectExport;

/// Gather, which is the one complete project in the repository.
fn gather() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../game")
        .canonicalize()
        .expect("the companion game is beside the engine")
}

struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let path =
            std::env::temp_dir().join(format!("sindri-export-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        Self(path)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn exported(name: &str, base: &str) -> (Scratch, AssetManifest) {
    let scratch = Scratch::new(name);
    let project = ProjectExport::gather(&gather()).expect("Gather gathers");
    project.write(&scratch.0, base).expect("Gather exports");
    let text = std::fs::read_to_string(scratch.0.join("assets/manifest.json"))
        .expect("a manifest is written");
    let manifest = AssetManifest::from_json(&text).expect("the manifest reads");
    (scratch, manifest)
}

/// Every kind a scene can reference has to arrive, or a game ships with a
/// silent sound or an invisible sprite.
#[test]
fn every_kind_of_asset_the_project_uses_is_carried() {
    let (_scratch, manifest) = exported("kinds", "/");
    for kind in [
        AssetKind::Scene,
        AssetKind::Script,
        AssetKind::Texture,
        AssetKind::Font,
        AssetKind::Audio,
        AssetKind::Sheet,
    ] {
        assert!(
            manifest.ids_of(kind).next().is_some(),
            "nothing was carried for {}",
            kind.as_str()
        );
    }
}

/// A script names its sounds at run time, and no walk of a scene can see a
/// string inside a program.
#[test]
fn an_asset_only_a_script_names_is_carried_because_the_project_said_so() {
    let (_scratch, manifest) = exported("include", "/");
    let audio: Vec<&str> = manifest
        .ids_of(AssetKind::Audio)
        .map(sindri_core::AssetId::as_str)
        .collect();
    assert!(audio.contains(&"audio/pickup.wav"), "{audio:?}");
}

/// Every file the manifest promises has to be where it says.
#[test]
fn every_promised_file_is_there_and_is_what_was_promised() {
    let (scratch, manifest) = exported("files", "/");
    let root = scratch.0.join("assets").join(manifest.content_root());
    assert!(!manifest.content_root().is_empty(), "no content root");
    for (id, entry) in manifest.assets() {
        let path = root.join(id.as_str());
        let bytes = std::fs::read(&path).unwrap_or_else(|_| panic!("{} is missing", id.as_str()));
        assert_eq!(bytes.len() as u64, entry.bytes, "{}", id.as_str());
        manifest
            .verify(id, &bytes)
            .unwrap_or_else(|error| panic!("{}: {error}", id.as_str()));
    }
}

/// The manifest is the only file that must not be cached, so it must not be
/// inside the directory whose name is what makes caching safe.
#[test]
fn the_manifest_sits_outside_the_hashed_directory() {
    let (scratch, manifest) = exported("layout", "/");
    assert!(scratch.0.join("assets/manifest.json").is_file());
    assert!(
        !scratch
            .0
            .join("assets")
            .join(manifest.content_root())
            .join("manifest.json")
            .exists()
    );
    assert!(scratch.0.join("index.html").is_file());
}

/// A changed asset must not land in a directory anyone has already cached.
#[test]
fn the_directory_name_follows_the_contents() {
    let project = ProjectExport::gather(&gather()).expect("Gather gathers");
    let before = project.content_hash();

    let mut changed = ProjectExport::gather(&gather()).expect("Gather gathers");
    changed.assets[1].bytes.push(b'!');
    assert_ne!(
        before,
        changed.content_hash(),
        "a changed build kept its name"
    );

    // And an unchanged one keeps it, or every deploy would re-download
    // everything.
    let again = ProjectExport::gather(&gather()).expect("Gather gathers");
    assert_eq!(before, again.content_hash());
}

/// Exporting twice must not leave the old build beside the new one.
#[test]
fn a_second_export_replaces_the_first() {
    let scratch = Scratch::new("twice");
    let mut project = ProjectExport::gather(&gather()).expect("Gather gathers");
    project.write(&scratch.0, "/").expect("exports");
    project.assets[1].bytes.push(b'!');
    let second = project.write(&scratch.0, "/").expect("exports again");

    let directories: Vec<String> = std::fs::read_dir(scratch.0.join("assets"))
        .expect("an assets directory")
        .filter_map(Result::ok)
        .filter(|entry| entry.path().is_dir())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(directories, vec![second.content_root], "{directories:?}");
}

/// A page served from a project subpath has to resolve its own host.
#[test]
fn the_page_carries_the_base_path_it_was_given() {
    let (scratch, _) = exported("base", "/sindri2/");
    let page = std::fs::read_to_string(scratch.0.join("index.html")).expect("a page");
    assert!(page.contains(r#"<base href="/sindri2/">"#), "{page}");
}
