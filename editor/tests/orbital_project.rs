//! What the editor loads when it opens the acceptance project.
//!
//! The lesson `gather_project.rs` records, one layer down: every unit test
//! around script loading builds a scene for the occasion, and a scene built for
//! the occasion is one whose scripts are named directly by entities in it.
//! Orbital Last Stand is not that. Its director spawns enemies from prefabs it
//! names in Decay, its screens are switched off until something shows them, and
//! opening it in the editor produced two hundred errors -- no prefab loaded, no
//! script for the HUD -- while the exported build of the same project played
//! perfectly.
//!
//! So this opens the project the way the editor does and asks what arrived.
//! Nothing here needs a GPU or a window.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use sindri_core::{ComponentSchemaRegistry, World};
use sindri_editor::native::scene_extractor;
use sindri_editor::scene_file::SceneFile;
use sindri_editor::scripts::SceneScripts;

const REQUIRED_PREFABS: [&str; 3] = [
    "prefabs/drifter.prefab.json",
    "prefabs/charger.prefab.json",
    "prefabs/splitter.prefab.json",
];

/// The acceptance project's scene, from this crate's own directory.
fn scene_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("the editor crate sits in the workspace")
        .join("games/orbital-last-stand/assets/orbital.scene.json")
}

/// Opens the scene and lets the loader settle, as an editor frame loop does.
///
/// Bounded by wall clock rather than by a fixed number of turns: the loader is
/// asynchronous, and a test that gave up after N frames would be measuring this
/// machine rather than the engine.
fn opened() -> (SceneScripts, World, ComponentSchemaRegistry) {
    let file = SceneFile::open(scene_path()).expect("the acceptance project opens");
    let extractor = scene_extractor();
    let world = World::from_scene(file.document())
        .expect("the acceptance scene loads")
        .world;
    let components = extractor.components().clone();
    let mut scripts = SceneScripts::for_scene(Some(&scene_path()));

    // Kept turning after the scripts compile, because that is when the
    // prefabs first become askable: an ask, a load, and a compile each happen
    // on a different turn, and stopping at the first clean compile stops one
    // turn before the prefabs are visible.
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        scripts.request(&world, &components);
        scripts.poll();
        let settled = scripts.compile(&world, &components).is_empty()
            && REQUIRED_PREFABS
                .iter()
                .all(|prefab| scripts.has_prefab(prefab));
        if settled {
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    (scripts, world, components)
}

#[test]
fn every_script_the_scene_names_is_loaded() {
    let (mut scripts, world, components) = opened();
    let failures = scripts.compile(&world, &components);
    assert!(
        failures.is_empty(),
        "the editor could not compile the project's scripts: {failures:#?}"
    );
}

/// Why the editor found none of them.
///
/// A prefab is named by the declared type of a compiled script's export, so it
/// cannot be asked for until the script that names it has been loaded *and*
/// compiled -- and both happen on later turns. One ask finds nothing, however
/// correct that ask is.
///
/// The editor made exactly one, because the call sat behind "has the world
/// changed since we last looked" and compiling a script changes no world. This
/// states the property that gate broke; it cannot reach the gate itself, which
/// lives in a frame loop needing eframe and a GPU.
#[test]
fn one_ask_is_never_enough_to_find_a_prefab() {
    let file = SceneFile::open(scene_path()).expect("the acceptance project opens");
    let extractor = scene_extractor();
    let world = World::from_scene(file.document())
        .expect("the acceptance scene loads")
        .world;
    let components = extractor.components().clone();
    let mut scripts = SceneScripts::for_scene(Some(&scene_path()));

    scripts.request(&world, &components);
    scripts.poll();
    assert!(
        !scripts.has_prefab("prefabs/drifter.prefab.json"),
        "if one ask were enough, the gate that allowed only one would not have \
         mattered, and this test would be guarding nothing"
    );
}

#[test]
fn every_prefab_the_scripts_spawn_is_loaded() {
    // The director spawns these by name. A prefab that is not loaded is an
    // enemy that never arrives, which is what the editor showed.
    let (scripts, _world, _components) = opened();
    for prefab in REQUIRED_PREFABS {
        assert!(
            scripts.has_prefab(prefab),
            "the editor never loaded {prefab}"
        );
    }
}
