//! Spine is now a segmented boss rather than a baked armour-state boss.
//!
//! Keep the old sheet-wiring check because the head still uses that baked art,
//! but prove the encounter's new identity at runtime: spawning Spine must build
//! its full independent body without Decay errors.

use orbital_baked::Run;
use serde_json::json;
use sindri_core::EntityId;
use sindri_decay::{ScriptComponent, ScriptValue};

const STEP: f32 = 1.0 / 60.0;

fn step(run: &mut Run) {
    let notes = run.step(STEP);
    assert!(notes.is_empty(), "{notes:#?}");
}

/// A run with the director off and the field cleared, so the only hostile
/// entities are the boss and the body it deliberately creates.
fn isolated_run() -> Run {
    let mut run = Run::open().expect("the project opens");
    for _ in 0..6 {
        step(&mut run);
    }
    run.click("TitleStart");
    step(&mut run);
    run.set_board("hp", 1000.0);
    let director = run.find("Director").expect("the director exists");
    run.world.get_mut(director).expect("director").disabled = true;
    let enemies: Vec<_> = run
        .world
        .entities()
        .filter_map(|(entity, _)| {
            run.components
                .get::<sindri_core::TagsComponent>(&run.world, entity)
                .ok()
                .flatten()
                .is_some_and(|tags| tags.has("enemy"))
                .then_some(entity)
        })
        .collect();
    for enemy in enemies {
        run.world.despawn_recursive(enemy).expect("enemy despawns");
    }
    step(&mut run);
    run
}

fn spawn_spine(run: &mut Run) -> EntityId {
    let document = run
        .prefabs
        .get("prefabs/spine.prefab.json")
        .expect("the Spine prefab ships");
    let entity = run.world.spawn_prefab(document).expect("boss spawns").root;
    let data = run.world.get_mut(entity).expect("boss remains");
    data.transform_3d.as_mut().expect("boss transform").position = [0.0, 3.5, 0.0];
    entity
}

fn tagged(run: &Run, tag: &str) -> Vec<EntityId> {
    run.world
        .entities()
        .filter_map(|(entity, _)| {
            run.components
                .get::<sindri_core::TagsComponent>(&run.world, entity)
                .ok()
                .flatten()
                .is_some_and(|tags| tags.has(tag))
                .then_some(entity)
        })
        .collect()
}

fn segment_at(run: &Run, wanted_slot: f64) -> EntityId {
    tagged(run, "spine_segment")
        .into_iter()
        .find(|entity| {
            run.components
                .get::<ScriptComponent>(&run.world, *entity)
                .ok()
                .flatten()
                .and_then(|script| {
                    script
                        .properties
                        .get("slot")
                        .and_then(serde_json::Value::as_f64)
                })
                == Some(wanted_slot)
        })
        .unwrap_or_else(|| panic!("Spine segment {wanted_slot} exists"))
}

#[test]
fn spine_builds_its_nine_segment_body_at_runtime() {
    let mut run = isolated_run();
    let boss = spawn_spine(&mut run);

    // One frame lets the head's start/update path construct the body; a few
    // more exercise the segment follower logic as real runtime entities. This
    // catches immutable exported chain state and similar errors that static
    // prefab validation cannot see.
    for _ in 0..6 {
        step(&mut run);
    }

    assert!(
        run.world.get(boss).is_some(),
        "Spine disappeared during startup"
    );
    let segments = tagged(&run, "spine_segment");
    assert_eq!(
        segments.len(),
        9,
        "Spine should enter the fight with nine independent body sections"
    );
}

#[test]
fn destroying_a_middle_segment_severs_and_promotes_the_rear_chain() {
    let mut run = isolated_run();
    spawn_spine(&mut run);
    for _ in 0..6 {
        step(&mut run);
    }

    let cut = segment_at(&run, 4.0);
    let new_head = segment_at(&run, 5.0);
    let target = run
        .world
        .get(cut)
        .and_then(|data| data.transform_3d.as_ref())
        .expect("the middle section has a transform")
        .position;
    let bullet = run
        .prefabs
        .get("prefabs/bullet.prefab.json")
        .expect("the bullet prefab ships")
        .clone();
    let shot = run.world.spawn_prefab(&bullet).expect("a shot spawns").root;
    let shot_data = run.world.get_mut(shot).expect("the shot remains");
    shot_data
        .transform_3d
        .as_mut()
        .expect("the shot has a transform")
        .position = target;
    let properties = shot_data
        .components
        .get_mut("sindri.script")
        .and_then(|script| script.get_mut("properties"))
        .and_then(serde_json::Value::as_object_mut)
        .expect("the shot has script properties");
    properties.insert("damage".to_owned(), json!(20.0));
    properties.insert("speed".to_owned(), json!(0.0));

    // The hit removes the selected body. The following physics pass retires
    // both joints attached to it, and the rear section promotes itself without
    // trying to mutate another running script's authored properties.
    for _ in 0..3 {
        step(&mut run);
    }

    assert!(
        run.world.get(cut).is_none(),
        "the struck section was destroyed"
    );
    assert_eq!(
        run.count("spine_segment"),
        8,
        "only the struck section is lost"
    );
    assert_eq!(
        run.physics.world().joint_count(),
        7,
        "the cut removes its two joints"
    );
    assert_eq!(
        run.scripts.field(new_head, "head"),
        Some(&ScriptValue::Number(1.0)),
        "the first surviving rear section becomes an autonomous head"
    );
}

/// The legacy armour clips still back the head's baked animation. Even though
/// health no longer selects them as armour tiers, every referenced frame must
/// remain valid so the existing Spine art cannot silently rot.
#[test]
fn every_armour_clip_names_frames_the_sheet_holds() {
    let run = Run::open().expect("the project opens");
    let prefab = run
        .prefabs
        .get("prefabs/spine.prefab.json")
        .expect("the boss prefab ships");
    let text =
        std::fs::read_to_string(orbital_baked::project().join("assets/textures/spine.sheet.json"))
            .expect("the sheet reads");
    let sheet: serde_json::Value = serde_json::from_str(&text).expect("the sheet parses");
    let names: Vec<&str> = sheet["grid"]["names"]
        .as_array()
        .expect("frame names")
        .iter()
        .filter_map(serde_json::Value::as_str)
        .collect();

    let json = serde_json::to_value(prefab).expect("the prefab serialises");
    let clips = json["entities"][0]["components"]["sindri.animation.sprite"]["clips"]
        .as_object()
        .expect("the boss has clips");
    assert_eq!(
        clips.len(),
        5,
        "the baked head still ships five clips: {clips:?}"
    );
    for (name, clip) in clips {
        let frames = clip["frames"].as_array().expect("frames");
        assert!(!frames.is_empty(), "clip {name} has no frames");
        for frame in frames {
            let frame = frame.as_str().expect("a frame name");
            assert!(
                names.contains(&frame),
                "clip {name} names frame {frame}, which the sheet does not hold"
            );
        }
    }
}
