//! The world as it is walked: where things draw, what stops you, and whether
//! the game can be finished.
//!
//! Split from `the_game_holds_together.rs`, which asks whether the scene's
//! assets exist and load. These ask what happens once they do, and they share
//! one thing that file does not need: the floor's own mapping from world
//! positions back to cells, so every check here is stated in the coordinates
//! the game is authored in.

use std::collections::BTreeSet;

use sindri_core::{Transform3D, World};
use sindri_decay::{AudioCommand, ScriptFrame, Scripts};
use sindri_gather::{AUDIO, Session, extractor, sources, world};
use sindri_grid::{GridCoord, GridPoint, GridSpace, PlanePoint};
use sindri_platform::InputState;
use sindri_scene::{SceneExtractor, SpriteComponent, TilemapComponent, WorldGridNavigation};

/// A grid coordinate as a transform holds it.
///
/// The grid works in `f64` and a transform in `f32`, so something has to
/// narrow. Named, so the narrowing reads as the intent it is.
#[allow(clippy::cast_possible_truncation)]
fn f64_to_f32(value: f64) -> f32 {
    value as f32
}

fn logical_position(grid: GridSpace, map: Transform3D, world: [f32; 3]) -> GridPoint {
    let (sin, cos) = map.rotation_z_radians().sin_cos();
    let x = world[0] - map.position[0];
    let y = world[1] - map.position[1];
    let local = PlanePoint::new(
        f64::from((cos * x + sin * y) / map.scale[0]),
        f64::from((-sin * x + cos * y) / map.scale[1]),
    );
    grid.unproject(local)
        .expect("the authored point unprojects")
}

fn floor_grid(world: &World, extractor: &SceneExtractor) -> (Transform3D, GridSpace) {
    let (floor, tilemap) = extractor
        .components()
        .query::<TilemapComponent>(world)
        .expect("the tilemap schema reads")
        .into_iter()
        .next()
        .expect("Gather has a floor");
    let map = world
        .get(floor)
        .and_then(|data| data.transform_3d)
        .unwrap_or_default();
    (
        map,
        tilemap.grid_space().expect("the floor has a valid grid"),
    )
}

/// Draw order follows where a thing stands, not what it is made of.
///
/// Sprites batch by layer *and texture*, and a frame's passes are ordered by
/// layer alone — so two different textures on one layer are drawn in whichever
/// order their textures happen to sort in, however far apart they stand. That
/// is why every world entity carries a layer derived from its isometric row
/// rather than a hand-picked one: before this, the orbs sat on layer 10 and the
/// player on 20, so both drew over the shrine from anywhere on the island.
///
/// Half rows, because a wall stands on the edge *between* two cells and so
/// falls on an exact half row; rounding that to a whole one decides by coin
/// toss whether the wall occludes what is behind it.
#[test]
fn every_world_sprite_layers_by_where_it_stands() {
    let (world, _scenes) = world().expect("the scene loads");
    let extractor = extractor().expect("the schemas register");
    let (map, grid) = floor_grid(&world, &extractor);

    let mut checked = 0;
    for (entity, sprite) in extractor
        .components()
        .query::<SpriteComponent>(&world)
        .expect("the sprite schema reads")
    {
        let Some(transform) = world.get(entity).and_then(|data| data.transform_3d) else {
            continue;
        };
        let at = logical_position(grid, map, transform.position);
        if !(-1.0_f64..=9.0).contains(&at.x) || !(-1.0_f64..=9.0).contains(&at.y) {
            continue;
        }

        // Kept in floating point rather than casting the row to an integer:
        // the cast would be the only lossy step in the comparison, and it is
        // not the thing under test. Both sides are whole numbers, so the
        // tolerance costs nothing and says so.
        let expected = 1.0 + (2.0 * (at.x + at.y)).round();
        assert!(
            (f64::from(sprite.layer) - expected).abs() < 1.0e-9,
            "{} stands on row {:.2} and belongs on layer {expected}, not {}",
            world
                .get(entity)
                .and_then(|data| data.source_id.as_ref().map(|id| id.as_str().to_owned()))
                .unwrap_or_default(),
            at.x + at.y,
            sprite.layer,
        );
        checked += 1;
    }

    assert!(
        checked >= 20,
        "expected the world's sprites, checked {checked}"
    );
}

/// Solid things are solid: walking at one stops rather than passing through.
///
/// Driven by holding a direction through the real scripts, which is what a
/// player does — not by asking the collision helper whether a point is free,
/// which would only test that the helper agrees with itself.
///
/// The player is put beside the tree rather than walked at whatever happens to
/// be west of its start: a collision test that depends on the layout fails
/// every time someone moves a tree, which is a test about composition wearing a
/// collision test's name.
#[test]
fn the_player_cannot_walk_through_solid_scenery() {
    use sindri_platform::{InputEvent, Key};

    let (mut world, _scenes) = world().expect("the scene loads");
    let extractor = extractor().expect("the schemas register");
    let sources = sources();
    let mut scripts = Scripts::new();
    let (map, grid) = floor_grid(&world, &extractor);

    let find = |world: &World, name: &str| {
        world
            .entities()
            .find(|(_, data)| {
                data.source_id
                    .as_ref()
                    .is_some_and(|id| id.as_str() == name)
            })
            .map(|(entity, _)| entity)
            .expect("the scene names this entity")
    };
    let player = find(&world, "player");
    let tree = find(&world, "tree-0");

    let cell = |world: &World, entity| {
        logical_position(
            grid,
            map,
            world
                .get(entity)
                .and_then(|data| data.transform_3d)
                .expect("a transform")
                .position,
        )
    };
    let blocker = cell(&world, tree);

    // Two cells due east of the tree, so holding one key walks straight at it.
    let start = GridPoint::new(blocker.x + 2.0, blocker.y);
    {
        let local = grid.project(start).expect("the start projects");
        // The grid works in f64 and a transform holds f32, so the narrowing is
        // the point of the conversion rather than an accident of it.
        let (local_x, local_y) = (local.x, local.y);
        let data = world.get_mut(player).expect("the player exists");
        let mut transform = data.transform_3d.expect("the player has a transform");
        transform.position[0] = map.position[0] + f64_to_f32(local_x);
        transform.position[1] = map.position[1] + f64_to_f32(local_y);
        data.transform_3d = Some(transform);
    }

    let mut held = InputState::default();
    held.apply(InputEvent::KeyPressed(Key::ArrowLeft));
    for _ in 0..240 {
        let report = scripts.advance(
            &mut world,
            extractor.components(),
            ScriptFrame::new(&sources, &held, 1.0 / 60.0),
        );
        assert!(report.failures.is_empty(), "{:?}", report.failures);
    }

    let ended = cell(&world, player);
    assert!(
        ended.x < start.x - 0.5,
        "the player did not move: it is still at {ended:?}"
    );
    assert!(
        ended.x > blocker.x + 0.4,
        "the player reached {ended:?}, walking into the tree at {blocker:?}"
    );
}

/// The walk cycle belongs to walking.
///
/// Gather's player has had a four-frame `bob` clip since the sheet was sliced,
/// and it played whenever the scene did — so standing still was a bob on the
/// spot. The clip was authored and always playing, and nothing could tell it
/// otherwise. This is what Decay reaching animation is *for*, and it is proved
/// here rather than only in the engine's own tests: the real script, the real
/// scene, and the real input a window feeds.
#[test]
fn the_player_only_bobs_while_walking() {
    use sindri_platform::{InputEvent, Key};

    let (mut world, _scenes) = world().expect("the scene loads");
    let extractor = extractor().expect("the schemas register");
    let sources = sources();
    let mut scripts = Scripts::new();
    let mut animations = sindri_scene::SpriteAnimations::new();

    let player = world
        .entities()
        .find(|(_, data)| {
            data.source_id
                .as_ref()
                .is_some_and(|id| id.as_str() == "player")
        })
        .map(|(entity, _)| entity)
        .expect("the scene names the player");

    let playing = |world: &World| -> Option<String> {
        world
            .get(player)?
            .components
            .get("sindri.animation.sprite")?
            .get("playing")?
            .as_str()
            .map(str::to_owned)
    };

    let step = |world: &mut World,
                scripts: &mut Scripts,
                animations: &mut sindri_scene::SpriteAnimations,
                held: &InputState| {
        let report = scripts.advance(
            world,
            extractor.components(),
            ScriptFrame::new(&sources, held, 1.0 / 60.0).with_animations(animations),
        );
        assert!(report.failures.is_empty(), "{:?}", report.failures);
    };

    // Standing still, with nothing held.
    let still = InputState::default();
    for _ in 0..4 {
        step(&mut world, &mut scripts, &mut animations, &still);
    }
    assert_eq!(
        playing(&world),
        None,
        "the player bobbed on the spot with nothing held"
    );

    // Walking.
    let mut held = InputState::default();
    held.apply(InputEvent::KeyPressed(Key::ArrowLeft));
    for _ in 0..4 {
        step(&mut world, &mut scripts, &mut animations, &held);
    }
    assert_eq!(
        playing(&world).as_deref(),
        Some("bob"),
        "the walk cycle did not start when the player walked"
    );

    // And stopping again.
    for _ in 0..4 {
        step(&mut world, &mut scripts, &mut animations, &still);
    }
    assert_eq!(
        playing(&world),
        None,
        "the walk cycle kept running after the player stopped"
    );
}

/// The Wisp is real gameplay pathfinding: its first direct east edge is authored
/// as a wall, so deterministic cardinal A* must route south before approaching
/// the player. This catches a script that merely moves toward the target while
/// ignoring the scene navigation components.
#[test]
fn the_wisp_routes_around_the_authored_wall() {
    let (mut world, _scenes) = world().expect("the scene loads");
    let extractor = extractor().expect("the schemas register");
    let floor = world
        .entities()
        .find(|(_, data)| {
            data.source_id
                .as_ref()
                .is_some_and(|id| id.as_str() == "floor")
        })
        .map(|(entity, _)| entity)
        .expect("the game has a floor");
    let wisp = world
        .entities()
        .find(|(_, data)| {
            data.source_id
                .as_ref()
                .is_some_and(|id| id.as_str() == "wisp")
        })
        .map(|(entity, _)| entity)
        .expect("the game has a wisp");

    let before = WorldGridNavigation::from_world(&world, floor)
        .expect("the authored navigation is valid")
        .placement(wisp)
        .expect("the wisp is an occupant")
        .anchor;
    assert_eq!(before, GridCoord::new(0, 0));

    let mut session = Session::new(extractor.components().clone());
    session
        .step(&mut world, &InputState::default(), (960.0, 600.0), 0.33)
        .expect("the pathfinding script steps");

    let after = WorldGridNavigation::from_world(&world, floor)
        .expect("navigation remains valid")
        .placement(wisp)
        .expect("the wisp remains an occupant")
        .anchor;
    assert_eq!(
        after,
        GridCoord::new(0, 1),
        "the wall from (0,0) to (1,0) forces the first A* step south"
    );
}

/// The game is playable: walking into an orb collects it, and collecting them
/// all wins.
///
/// Driven through the same scripts and the same input the window feeds, so this
/// is the game being played rather than a model of it — and steered toward each
/// orb in turn, which is what a player does.
#[test]
fn walking_into_the_orbs_wins_the_game() {
    use sindri_platform::{InputEvent, Key};

    let (mut world, _scenes) = world().expect("the scene loads");
    let extractor = extractor().expect("the schemas register");
    let sources = sources();
    let mut scripts = Scripts::new();
    // The island's doors ask for a scene change, so these frames need somewhere
    // to record one. A frame built without it is a host that plays one scene,
    // and `Scene.go` says so rather than accepting a request nothing performs.
    let mut scenes = sindri_decay::SceneChannel::playing("gather.scene.json");

    let (map, grid) = floor_grid(&world, &extractor);

    let orbs: Vec<GridPoint> = world
        .entities()
        .filter(|(_, data)| {
            data.source_id
                .as_ref()
                .is_some_and(|id| id.as_str().starts_with("orb-"))
        })
        .map(|(_, data)| {
            logical_position(grid, map, data.transform_3d.unwrap_or_default().position)
        })
        .collect();
    assert_eq!(orbs.len(), 5, "the game has five orbs");

    let player = world
        .entities()
        .find(|(_, data)| {
            data.source_id
                .as_ref()
                .is_some_and(|id| id.as_str() == "player")
        })
        .map(|(entity, _)| entity)
        .expect("the game has a player");

    let mut steps = 0;
    for orb in &orbs {
        // Walk toward this one until it is gathered, holding whichever keys
        // point that way — the same two axes the window reports.
        let target_score = scripts.blackboard().get("score", 0.0) + 1.0;
        while scripts.blackboard().get("score", 0.0) < target_score {
            let at = logical_position(
                grid,
                map,
                world
                    .get(player)
                    .and_then(|data| data.transform_3d)
                    .expect("the player kept its transform")
                    .position,
            );
            let mut held = InputState::default();
            if orb.x - at.x > 0.02 {
                held.apply(InputEvent::KeyPressed(Key::ArrowRight));
            } else if at.x - orb.x > 0.02 {
                held.apply(InputEvent::KeyPressed(Key::ArrowLeft));
            }
            if orb.y - at.y > 0.02 {
                held.apply(InputEvent::KeyPressed(Key::ArrowDown));
            } else if at.y - orb.y > 0.02 {
                held.apply(InputEvent::KeyPressed(Key::ArrowUp));
            }

            // The doors on the island ask for a scene change, so this frame
            // is given somewhere to record that. Without it `Scene.go` says
            // the host plays one scene -- which is the truth for a frame built
            // without one, and is what this used to be.
            let report = scripts.advance(
                &mut world,
                extractor.components(),
                ScriptFrame::new(&sources, &held, 1.0 / 60.0).with_scenes(&mut scenes),
            );
            assert!(report.failures.is_empty(), "{:?}", report.failures);

            steps += 1;
            assert!(
                steps < 4_000,
                "walking to {orb:?} never arrived; the player is at {at:?} with a score of {}",
                scripts.blackboard().get("score", 0.0)
            );
        }
    }

    assert!(
        (scripts.blackboard().get("score", 0.0) - 5.0).abs() < 1.0e-6,
        "every orb is gathered"
    );

    // And the banner, which is how the game says so, has faded in.
    for _ in 0..90 {
        scripts.advance(
            &mut world,
            extractor.components(),
            ScriptFrame::new(&sources, &InputState::default(), 1.0 / 60.0),
        );
    }
    let banner = world
        .entities()
        .find(|(_, data)| {
            data.source_id
                .as_ref()
                .is_some_and(|id| id.as_str() == "banner")
        })
        .map(|(_, data)| {
            data.components["sindri.ui.image"]["tint"][3]
                .as_f64()
                .unwrap_or(0.0)
        })
        .expect("the game has a banner");
    assert!(
        (banner - 1.0).abs() < 1.0e-6,
        "the banner shows once the game is won, and it is at {banner}"
    );

    // The exit gate for audio: a headless run with no sound device can still
    // prove what the game asked to play.
    assert_the_sounds_a_won_game_asked_for(&mut scripts, orbs.len());
}

/// Five orbs make five pickups, and winning plays the fanfare once rather than
/// every frame the banner shows.
fn assert_the_sounds_a_won_game_asked_for(scripts: &mut Scripts, orbs: usize) {
    let asked = scripts.take_audio_commands();
    let played = |wanted: &str| {
        asked
            .iter()
            .filter(|command| matches!(command, AudioCommand::Play { clip, .. } if clip == wanted))
            .count()
    };
    assert_eq!(
        played("audio/pickup.wav"),
        orbs,
        "one sound per orb: {asked:?}"
    );
    assert_eq!(
        played("audio/victory.wav"),
        1,
        "the fanfare plays once: {asked:?}"
    );
    assert!(
        scripts.take_audio_commands().is_empty(),
        "draining takes the requests, so a host cannot play them twice"
    );
}

/// Every sound the game can play is one it ships, and one that will decode.
///
/// The decode half is what a silent backend cannot tell you: it records the
/// name of a clip without ever looking at its bytes, so three clips at a sample
/// rate no browser accepts passed every test in the workspace and failed only
/// in a real browser, from a promise nothing was watching.
#[test]
fn every_sound_the_game_plays_is_shipped_and_decodes() {
    use sindri_assets::{AssetBytes, AssetDecoder, AudioAssetDecoder};
    use sindri_core::AssetId;
    use sindri_scene::AudioSourceComponent;

    let (world, _scenes) = world().expect("the scene loads");
    let extractor = extractor().expect("the schemas register");
    let shipped: BTreeSet<&str> = AUDIO.iter().map(|(id, _)| *id).collect();

    for (_, source) in extractor
        .components()
        .query::<AudioSourceComponent>(&world)
        .expect("the audio schema reads")
    {
        assert!(
            shipped.contains(source.clip.as_str()),
            "the scene plays '{}', which the binary does not carry",
            source.clip
        );
    }

    for (id, bytes) in AUDIO {
        AudioAssetDecoder
            .decode(AssetBytes::new(
                (*id).parse::<AssetId>().expect("asset id"),
                (*bytes).to_vec(),
            ))
            .unwrap_or_else(|error| panic!("{id} does not decode: {error}"));
    }
}

/// A fresh game starts at nothing, so playing again is playing again.
#[test]
fn starting_over_starts_at_nothing() {
    let (mut world, _scenes) = world().expect("the scene loads");
    let extractor = extractor().expect("the schemas register");
    let mut scripts = Scripts::new();
    scripts.advance(
        &mut world,
        extractor.components(),
        ScriptFrame::new(&sources(), &InputState::default(), 0.5),
    );
    scripts.clear();
    assert!(!scripts.blackboard().has("score"));
}
