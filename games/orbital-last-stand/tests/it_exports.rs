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
    let authored = std::fs::read_dir(orbital_last_stand::project().join("assets/prefabs"))
        .expect("the project's prefabs")
        .filter_map(Result::ok)
        .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "json"))
        .count();
    assert_eq!(prefabs.len(), authored, "{prefabs:?}");
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
    // Counted from the project rather than written down. A literal here was a
    // number that had to be edited every time a script was added, which makes
    // it a chore rather than a check -- and a chore gets bumped to whatever the
    // run reported, which is exactly how a missing script would have got in.
    let authored: Vec<String> =
        std::fs::read_dir(orbital_last_stand::project().join("assets/scripts"))
            .expect("the project's scripts")
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .filter(|name| name.ends_with(".decay"))
            .collect();
    assert_eq!(
        scripts.len(),
        authored.len(),
        "every authored script ships: authored {authored:?}, shipped {scripts:?}"
    );
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

/// 10 and 11 together, minus the browser: the build that would be served is
/// opened the way a browser opens it — every asset by the logical ID the
/// manifest names, out of the hashed directory it names — and played.
///
/// This is what catches a build that is complete by the hashes and still not a
/// game. A missing script is bytes nobody asked for; here it is a run that
/// does not start.
#[test]
fn the_exported_build_plays() {
    let (scratch, _) = exported("plays");
    let mut run = orbital_last_stand::Run::open_export(&scratch.0).expect("the build opens");

    for _ in 0..6 {
        let notes = run.step(1.0 / 60.0);
        assert!(notes.is_empty(), "{notes:#?}");
    }
    assert_eq!(run.board("run_state"), 0.0, "the title should be showing");

    run.click("TitleStart");
    assert_eq!(run.board("run_state"), 1.0, "START should start the run");

    // The exported build must preserve the same continuous pressure as the
    // source project. The old batch director had already dropped roughly eight
    // enemies by this point; one-at-a-time pressure has only produced one or two.
    let notes = run.step(1.0 / 60.0);
    assert!(notes.is_empty(), "{notes:#?}");
    for _ in 0..66 {
        let notes = run.step(1.0 / 60.0);
        assert!(notes.is_empty(), "{notes:#?}");
    }
    let arrivals = run.count("enemy") + run.board("kills") as usize;
    assert!(
        (1..=2).contains(&arrivals),
        "exported build batched {arrivals} early enemies"
    );

    for _ in 0..900 {
        let notes = run.step(1.0 / 60.0);
        assert!(notes.is_empty(), "{notes:#?}");
    }
    // Spawned from prefabs that only reached this directory because the export
    // followed a script's declared field types into them.
    assert!(run.board("kills") > 0.0, "nothing died in the build");
}
