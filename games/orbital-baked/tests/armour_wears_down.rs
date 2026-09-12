//! The boss visibly loses armour as it is damaged.
//!
//! The vector game shed a polygon side per damage tier, driven by
//! `this.shape.count`. A sprite sheet has fixed geometry, so the tiers are baked
//! as five sets of frames and chosen by clip. That is a mechanic hiding in an
//! asset pipeline: nothing about a sheet, a prefab or a script fails visibly if
//! the wiring is wrong, and the boss simply wears its full armour to the grave.
//! So the wiring is checked here rather than looked at.

use orbital_baked::Run;
use sindri_core::EntityId;

const STEP: f32 = 1.0 / 60.0;

fn step(run: &mut Run) {
    let notes = run.step(STEP);
    assert!(notes.is_empty(), "{notes:#?}");
}

/// A run with the director off and the field cleared, so the only thing alive
/// is the boss this test spawns.
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
    // The Spine is the one boss whose armour thins as it is damaged, and it is
    // now its own prefab rather than a `kind` selected out of a shared one.
    let document = run
        .prefabs
        .get("prefabs/spine.prefab.json")
        .expect("the Spine prefab ships");
    let entity = run.world.spawn_prefab(document).expect("boss spawns").root;
    let data = run.world.get_mut(entity).expect("boss remains");
    data.transform_3d.as_mut().expect("boss transform").position = [0.0, 3.5, 0.0];
    entity
}

/// Which clip the boss is playing, which is which armour tier it is wearing.
fn clip(run: &Run, entity: EntityId) -> String {
    run.world
        .get(entity)
        .and_then(|data| data.components.get("sindri.animation.sprite"))
        .and_then(|animation| animation.get("playing"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_owned()
}

#[test]
fn the_boss_sheds_armour_as_its_health_falls() {
    let mut run = isolated_run();
    let boss = spawn_spine(&mut run);
    step(&mut run);
    step(&mut run);

    assert_eq!(
        clip(&run, boss),
        "armour10",
        "a fresh boss wears its full ring of plates"
    );

    // Every threshold the vector outline used, in the order it used them.
    //
    // Driven through `boss_max` rather than by damaging the boss. The script
    // reads `share` as its own health over that board, and sets the board once
    // at spawn, so raising it is the same arithmetic the boss would see after
    // taking a beating -- and it does not depend on a weapon, a hit box, or how
    // long a fight takes. Writing the script's `hp` property directly does not
    // work at all: the value is copied into script state at start, and the
    // property is not read again.
    let full = run.board("boss_max");
    assert!(full > 0.0, "the boss reports no maximum health");
    for (share, expected) in [
        (0.70, "armour9"),
        (0.50, "armour8"),
        (0.30, "armour7"),
        (0.10, "armour5"),
    ] {
        run.set_board("boss_max", full / share);
        step(&mut run);
        assert_eq!(
            clip(&run, boss),
            expected,
            "at {share} of its health the boss should be wearing {expected}"
        );
    }

    // And back up: the tier follows health rather than only ever falling, so a
    // boss that heals wears its plates again.
    run.set_board("boss_max", full);
    step(&mut run);
    assert_eq!(
        clip(&run, boss),
        "armour10",
        "a healed boss is armoured again"
    );
}

/// The clips have to exist, and name frames the sheet actually holds. A clip
/// naming a missing frame is the failure this pipeline makes easiest to write
/// and hardest to see.
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
    assert_eq!(clips.len(), 5, "one clip per armour tier: {clips:?}");
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
