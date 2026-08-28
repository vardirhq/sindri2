//! What the editor offers against the project the companion game actually is.
//!
//! Gather is the one project in the repository laid out the way a real one is:
//! its scene, art, scripts, fonts, and audio live under `assets/`, and its
//! Cargo manifest, `src/`, `tests/`, and web page live beside that directory
//! rather than in it. Every unit test around `ProjectTree` builds a project for
//! the occasion, and a project built for the occasion is exactly the one that
//! never caught this: rooting the browser at the project moved it away from the
//! directory the asset loader resolves against, so the inspector offered
//! `assets/textures/orb.png` for a scene that loads `textures/orb.png` — and
//! taking the offer made the sprite vanish.
//!
//! So this test opens the game's own project and asks the browser for the
//! references it would put in a picker, then checks them against the references
//! the scene file already names. Nothing here needs a GPU or a window.

use std::path::{Path, PathBuf};

use sindri_editor::project::{Project, ProjectTree};
use sindri_editor::tilemap::SpritePalette;

/// The companion game's directory, from this crate's own.
fn gather() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("the editor crate sits in the workspace")
        .join("game")
}

/// The browser as the editor builds it for an open scene: rooted at the
/// project, resolving at the scene's own directory.
fn browsing(scene: &Path) -> ProjectTree {
    let project = Project::open(&gather()).expect("Gather is a project");
    ProjectTree::rooted_as(project.root(), project.name()).resolving_at(scene.parent())
}

/// Every reference of one kind the scene file names, read as text rather than
/// through the schema: the point is what is written in the file.
fn named_in_scene(scene: &str, key: &str) -> Vec<String> {
    let needle = format!("\"{key}\": \"");
    let mut found: Vec<String> = scene
        .match_indices(&needle)
        .filter_map(|(at, _)| {
            let rest = &scene[at + needle.len()..];
            rest.find('"').map(|end| rest[..end].to_owned())
        })
        // A procedural reference is generated rather than loaded, and no
        // directory listing can offer one.
        .filter(|reference| !reference.starts_with("procedural:"))
        .collect();
    found.sort();
    found.dedup();
    found
}

#[test]
fn the_browser_offers_gather_the_references_gather_uses() {
    let scene = gather().join("assets/gather.scene.json");
    let text = std::fs::read_to_string(&scene).expect("the game's scene is there");
    let tree = browsing(&scene);

    for (key, offered) in [
        ("texture", tree.textures()),
        ("source", tree.scripts()),
        ("font", tree.fonts()),
        ("clip", tree.audio()),
    ] {
        let used = named_in_scene(&text, key);
        assert!(
            !used.is_empty(),
            "the game should name at least one {key}, or this proves nothing"
        );
        for reference in used {
            assert!(
                offered.contains(&reference),
                "the scene loads {key} {reference:?}, and the browser offers {offered:?}"
            );
        }
    }
}

/// The other half of the same fault: what the browser offers has to be what
/// loads, so nothing it offers may carry the project-root prefix that made the
/// sprite disappear.
#[test]
fn nothing_offered_is_spelled_from_the_project_root() {
    let scene = gather().join("assets/gather.scene.json");
    let tree = browsing(&scene);

    let offered: Vec<String> =
        [tree.textures(), tree.scripts(), tree.fonts(), tree.audio()].concat();
    assert!(!offered.is_empty(), "the game has assets to offer");
    for reference in offered {
        assert!(
            !reference.starts_with("assets/"),
            "{reference:?} is the path from the project root, not the one the scene resolves"
        );
    }
}

/// The browser's default listing, for a project shaped like this one: the
/// directory whose files a scene can name, with the rest of the project one
/// control away rather than two thirds of the rows.
#[test]
fn the_listing_starts_at_the_assets_and_the_rest_is_still_there() {
    let scene = gather().join("assets/gather.scene.json");
    let tree = browsing(&scene);

    assert_eq!(tree.assets_root(), Some(gather().join("assets").as_path()));
    assert!(
        tree.keeps_more_than_assets(),
        "Gather keeps a Cargo manifest, a src/, and a web page outside its assets"
    );

    let in_assets: Vec<&str> = tree
        .folders_in(tree.assets_root())
        .into_iter()
        .map(|(entry, _)| entry.name.as_str())
        .collect();
    assert_eq!(in_assets, ["audio", "fonts", "scripts", "textures"]);

    let whole_project: Vec<&str> = tree
        .folders_in(None)
        .into_iter()
        .map(|(entry, _)| entry.name.as_str())
        .collect();
    assert!(
        whole_project.contains(&"src") && whole_project.contains(&"tests"),
        "and asking for the whole project still shows what the game is made of: {whole_project:?}"
    );
}

/// The tile and sprite palettes read the file behind a reference by joining it
/// onto a directory, and it has to be the same directory the loader uses.
/// Joined onto the project root, Gather's `textures/tiles.png` names
/// `game/textures/tiles.png`, which is nothing: the tilemap inspector showed a
/// missing-file message instead of the tiles, and the animation preview showed
/// one instead of the sheet.
#[test]
fn the_palette_reads_a_reference_from_where_the_scene_resolves_it() {
    let assets = gather().join("assets");
    let mut palette = SpritePalette::default();

    palette.ensure(Some(&assets), "textures/tiles.png");
    assert_eq!(palette.problem(), None);
    assert!(
        !palette.sprites().is_empty(),
        "the game's tile sheet names its sprites, and the palette lists them"
    );

    let mut from_the_root = SpritePalette::default();
    from_the_root.ensure(Some(&gather()), "textures/tiles.png");
    assert!(
        from_the_root.problem().is_some(),
        "and the project root is not that directory, which is the whole point"
    );
}
