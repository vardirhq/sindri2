//! What `tools/isometric-baker` writes has to be what Sindri would write.
//!
//! The baker is a Node tool that generates prefabs and sheet documents outside
//! this workspace, which means it reimplements two things Sindri owns: the
//! canonical serialization a document is written in, and the shortest decimal
//! an `f32` is written as. Both are the kind of agreement that holds until it
//! quietly does not — the symptom being an editor that rewrites numbers nobody
//! touched, the first time someone opens a generated prefab and saves it.
//!
//! So the agreement is checked here, against the real implementation, rather
//! than asserted in the tool's own tests. This reaches out of the crate to do
//! it, which is unusual and deliberate: `sindri-core` is what defines canonical
//! form, so this is the only place the claim can actually be tested.

use std::{
    fs,
    path::{Path, PathBuf},
};

use sindri_core::{PrefabDocument, SpriteSheetDocument};

/// The miniature project the baker's fixture bakes into.
fn baked_project() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/isometric-baker/fixtures/project")
}

fn read(relative: &str) -> String {
    let path = baked_project().join(relative);
    fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!(
            "{} is readable ({error}); re-bake it with \
             `node tools/isometric-baker/src/cli.ts \
             tools/isometric-baker/fixtures/project/prefabs/standing-stone.isobake.json \
             --out tools/isometric-baker/fixtures/project`",
            path.display()
        )
    })
}

/// A generated prefab is already at the canonical fixed point.
///
/// Reading it and writing it again has to produce the identical bytes. That is
/// the whole claim: key order, indentation, which scalar arrays fold onto one
/// line, and the decimal an `f32` scale is spelled with.
#[test]
fn a_generated_prefab_is_written_the_way_sindri_writes_one() {
    let source = read("prefabs/standing-stone.prefab.json");
    let document = PrefabDocument::from_json(&source).expect("the generated prefab parses");
    let rewritten = document
        .to_canonical_json()
        .expect("a parsed prefab serializes");

    assert_eq!(
        source, rewritten,
        "the baker's prefab is not canonical, so opening and saving it in the editor \
         would rewrite it; the difference is what the baker's canonical writer got wrong"
    );
}

/// The prefab says exactly one thing about the world, and it is a sprite.
///
/// Guards the boundary the baker is written to hold: a generated asset carries
/// what any renderer needs and nothing a particular game invented.
#[test]
fn a_generated_prefab_is_one_sprite_on_one_root() {
    let source = read("prefabs/standing-stone.prefab.json");
    let document = PrefabDocument::from_json(&source).expect("the generated prefab parses");
    let root = document.root().expect("a prefab has exactly one root");

    assert_eq!(
        root.components.keys().collect::<Vec<_>>(),
        vec!["sindri.sprite"],
        "a generated prefab should carry only the components it was asked for"
    );

    let sprite = &root.components["sindri.sprite"];
    assert_eq!(
        sprite["texture"], "textures/standing-stone.png#south",
        "the prefab draws its default direction out of the generated sheet"
    );

    // The generation record is editor-only state, which runtimes ignore and a
    // spawn drops — so recording provenance cannot change what the asset does.
    assert!(
        root.editor.contains_key("sindri.isometric-baker"),
        "the prefab records what generated it"
    );
    assert!(
        root.editor["sindri.isometric-baker"]["recipe"].is_string(),
        "the generation record points back at the recipe that can rebuild it"
    );
}

/// The generated sheet is a sheet this runtime can actually cut.
#[test]
fn a_generated_sheet_slices_into_the_frames_it_names() {
    let source = read("textures/standing-stone.sheet.json");
    let document = SpriteSheetDocument::from_json(&source).expect("the generated sheet parses");
    let rects = document.rects().expect("the generated sheet slices");

    let mut names: Vec<&String> = rects.keys().collect();
    names.sort();
    assert_eq!(
        names,
        vec!["east", "north", "south", "west"],
        "a four-direction bake names one sprite per compass direction"
    );

    // Uniform frames are the whole reason the baker does not crop: every
    // direction has to be the same size, or a centred sprite would slide around
    // its tile as it turned.
    let widths: Vec<f32> = rects.values().map(|rect| rect[2]).collect();
    assert!(
        widths
            .windows(2)
            .all(|pair| (pair[0] - pair[1]).abs() < 1.0e-6),
        "every frame of a bake must be the same width, got {widths:?}"
    );
}
