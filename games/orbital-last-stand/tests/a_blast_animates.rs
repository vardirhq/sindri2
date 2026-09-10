//! The one drawn thing in a game of drawn-by-arithmetic things.
//!
//! Orbital's visuals are `sindri.shape` driven by scripts, and for a shield
//! that breathes with your armour or an engine that stretches with thrust that
//! is the right tool: those are parameters of the run, and arithmetic is how
//! you say so. A detonation is the other kind of thing. It is not a parameter
//! of anything — it happens once, it is over, and its frames were drawn rather
//! than derived.
//!
//! So it is the piece of this game that wanted a sprite sheet, and the piece
//! that could not exist until a script could start a clip and be told when it
//! ended. Both halves are exercised here: the sheet the baker produced, and the
//! script that plays it and gets out of the way.

use orbital_last_stand::Run;

const STEP: f32 = 1.0 / 60.0;
const PREFAB: &str = "prefabs/detonation.prefab.json";

fn step(run: &mut Run) {
    let notes = run.step(STEP);
    assert!(notes.is_empty(), "{notes:#?}");
}

fn running() -> Run {
    let mut run = Run::open().expect("the project opens");
    for _ in 0..6 {
        step(&mut run);
    }
    run.click("TitleStart");
    step(&mut run);
    run
}

fn spawn_blast(run: &mut Run) -> sindri_core::EntityId {
    let document = run
        .prefabs
        .get(PREFAB)
        .expect("the detonation prefab is loaded");
    run.world
        .spawn_prefab(document)
        .expect("the detonation spawns")
        .root
}

/// The clip runs, and the frame showing changes as it does.
///
/// Read from the playback cursor rather than from the scene, which is the split
/// the whole surface is built around: the scene says *which* clip, the cursor
/// says where it has got to, and watching one play does not rewrite the other.
#[test]
fn a_spawned_blast_plays_its_clip() {
    let mut run = running();
    let blast = spawn_blast(&mut run);

    // The first step is the one the script starts the clip on, and the advance
    // in that same step is the one that begins it.
    step(&mut run);
    let first = run
        .animations
        .sprite(blast)
        .expect("the blast is showing a frame")
        .to_owned();

    let mut seen = std::collections::BTreeSet::new();
    seen.insert(first);
    for _ in 0..3 {
        step(&mut run);
        if let Some(sprite) = run.animations.sprite(blast) {
            seen.insert(sprite.to_owned());
        }
    }
    assert!(
        seen.len() > 1,
        "the blast held one frame for its whole life: {seen:?}"
    );
}

/// And it takes itself away when the clip ends.
///
/// This is the call the whole namespace was worth adding for. Before it, a
/// one-shot had to be timed by a countdown a script kept in step with the
/// clip's own frame times by hand — two numbers meaning one thing, which drift
/// the moment anybody re-times the animation.
#[test]
fn a_blast_despawns_itself_when_the_clip_ends() {
    let mut run = running();
    let blast = spawn_blast(&mut run);

    // Five frames at a twentieth of a second each, and a generous margin: the
    // assertion is that it goes, not exactly when.
    for _ in 0..40 {
        step(&mut run);
        if run.world.get(blast).is_none() {
            return;
        }
    }
    panic!("the blast was still there long after its clip had finished");
}

/// A clip may only name frames the sheet actually cuts.
///
/// A name no sheet places draws the whole image rather than failing, so this
/// would be a detonation that flashed the entire five-frame strip at once and
/// never said why.
#[test]
fn every_frame_the_clip_names_is_a_sprite_the_sheet_cuts() {
    let assets = orbital_last_stand::project().join("assets");
    let prefab: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(assets.join(PREFAB)).expect("the prefab reads"),
    )
    .expect("the prefab parses");
    let sheet: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(assets.join("textures/detonation.sheet.json"))
            .expect("the baked sheet reads"),
    )
    .expect("the sheet parses");

    let cut: Vec<&str> = sheet["grid"]["names"]
        .as_array()
        .expect("the sheet names its cells")
        .iter()
        .map(|name| name.as_str().expect("a cell name is text"))
        .collect();

    let clips = &prefab["entities"][0]["components"]["sindri.animation.sprite"]["clips"];
    let frames = clips["boom"]["frames"]
        .as_array()
        .expect("the clip lists its frames");
    assert!(!frames.is_empty(), "a clip with no frames is not a clip");
    for frame in frames {
        let frame = frame.as_str().expect("a frame is named with text");
        assert!(
            cut.contains(&frame),
            "the clip plays '{frame}', which the sheet does not cut: {cut:?}"
        );
    }
}
