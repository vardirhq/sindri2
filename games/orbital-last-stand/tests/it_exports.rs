//! 11. Export to a static directory, carrying everything the game needs.
//!
//! Here rather than beside the export's own tests because this is the project
//! that has the shapes those tests could not: prefabs that spawn prefabs, and
//! screens that are switched off until something shows them.

use sindri_assets::{AssetKind, AssetManifest};
use sindri_export::ProjectExport;

struct Scratch(std::path::PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn exported(name: &str) -> (Scratch, AssetManifest) {
    let path = std::env::temp_dir().join(format!("ols-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&path);
    let project = ProjectExport::gather(&orbital_last_stand::project()).expect("gathers");
    project.write(&path, "/sindri2/").expect("exports");
    let text =
        std::fs::read_to_string(path.join("assets/sindri.manifest.json")).expect("a manifest");
    let manifest = AssetManifest::from_json(&text).expect("it reads");
    (Scratch(path), manifest)
}

fn ids(manifest: &AssetManifest, kind: AssetKind) -> Vec<String> {
    manifest
        .ids_of(kind)
        .map(|id| id.as_str().to_owned())
        .collect()
}

/// A prefab is only ever named by a script's declared field type, so an export
/// that did not compile the scripts would ship a game that could not spawn.
#[test]
fn every_prefab_ships() {
    let (_scratch, manifest) = exported("prefabs");
    let prefabs = ids(&manifest, AssetKind::Prefab);
    assert_eq!(prefabs.len(), 8, "{prefabs:?}");
}

/// The splitter's shards and the Warden's shots are named by *prefabs*, not by
/// the scene: nothing in the scene mentions either. They ship because the walk
/// follows a prefab's own scripts into the prefabs those can spawn.
#[test]
fn a_prefab_only_another_prefab_names_ships() {
    let (_scratch, manifest) = exported("nested");
    let prefabs = ids(&manifest, AssetKind::Prefab);
    assert!(
        prefabs.iter().any(|id| id.contains("shard")),
        "the splitter's shards were left behind: {prefabs:?}"
    );
    assert!(
        prefabs.iter().any(|id| id.contains("enemy-bullet")),
        "the Warden's shots were left behind: {prefabs:?}"
    );
}

/// Every screen in this game is switched off until something shows it, and the
/// runtime's walks are active-only. An export asking the runtime's question
/// shipped a game with no pause screen.
#[test]
fn what_is_switched_off_still_ships() {
    let (_scratch, manifest) = exported("hidden");
    let scripts = ids(&manifest, AssetKind::Script);
    for hidden in ["scripts/hud.decay", "scripts/result.decay"] {
        assert!(
            scripts.iter().any(|id| id == hidden),
            "{hidden} was left behind: {scripts:?}"
        );
    }
    assert_eq!(scripts.len(), 15, "{scripts:?}");
}

/// Every file the manifest promises is where it says, and is what it says.
#[test]
fn the_whole_build_is_there() {
    let (scratch, manifest) = exported("whole");
    let root = scratch.0.join("assets").join(manifest.content_root());
    for (id, entry) in manifest.assets() {
        let bytes = std::fs::read(root.join(id.as_str()))
            .unwrap_or_else(|error| panic!("{}: {error}", id.as_str()));
        assert_eq!(bytes.len() as u64, entry.bytes, "{}", id.as_str());
        manifest
            .verify(id, &bytes)
            .unwrap_or_else(|error| panic!("{}: {error}", id.as_str()));
    }
    let page = std::fs::read_to_string(scratch.0.join("index.html")).expect("a page");
    assert!(page.contains(r#"<base href="/sindri2/">"#), "{page}");
    assert!(page.contains("Orbital Last Stand"), "the game is not named");
}
