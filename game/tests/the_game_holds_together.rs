//! The game is the engine's exit gate, so what would break it is checked here.
//!
//! Not a rendering test — the offscreen capture covers that. These are the
//! things that would leave the game unopenable or unplayable while every other
//! test in the workspace still passed: a scene that names a texture nobody
//! ships, a script that stopped compiling, a property set on a field that was
//! renamed.

use std::collections::BTreeSet;

use sindri_core::{SceneComponent, SceneDocument, Transform3D, UnknownComponentPolicy};
use sindri_decay::{ScriptComponent, ScriptSources, Scripts};
use sindri_gather::{FONTS, extractor, sources, world};
use sindri_grid::{GridPoint, GridSpace, PlanePoint};
use sindri_platform::InputState;
use sindri_scene::TilemapComponent;

const SCENE: &str = include_str!("../assets/gather.scene.json");

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

/// Every texture the scene names is one the binary carries.
///
/// A missing one draws the magenta checker rather than failing, so nothing
/// else would notice — the game would just look wrong.
#[test]
fn every_texture_the_scene_names_is_shipped() {
    let world = world().expect("the scene loads");
    let referenced: BTreeSet<String> = sindri_scene::referenced_textures(&world)
        .into_iter()
        .collect();
    let shipped: BTreeSet<String> = ["tiles", "orb", "player", "pip", "banner"]
        .into_iter()
        .map(|name| format!("textures/{name}.png"))
        .collect();
    assert_eq!(referenced, shipped);
}

/// Every font is embedded too; an absent font deliberately draws no text
/// rather than falling back to a machine-dependent face.
#[test]
fn every_font_the_scene_names_is_shipped() {
    let world = world().expect("the scene loads");
    let referenced = sindri_scene::referenced_fonts(&world);
    let shipped = FONTS
        .iter()
        .map(|(id, _)| (*id).to_owned())
        .collect::<BTreeSet<_>>();
    assert_eq!(referenced, shipped);
}

/// Every script the scene names is one the binary carries, and it compiles.
#[test]
fn every_script_the_scene_names_compiles() {
    let world = world().expect("the scene loads");
    let extractor = extractor().expect("the schemas register");
    let named = sindri_decay::referenced_sources(&world, extractor.components());
    let sources = sources();
    for id in &named {
        assert!(sources.get(id).is_some(), "{id} is named but not shipped");
    }

    let failures = Scripts::new().compile(&world, extractor.components(), &sources);
    assert!(failures.is_empty(), "{failures:?}");
}

/// The scene uses only components the game understands, so nothing in it is
/// carried along doing nothing.
#[test]
fn the_scene_holds_no_component_the_game_cannot_run() {
    let document = SceneDocument::from_json(SCENE).expect("the scene parses");
    extractor()
        .expect("the schemas register")
        .validate(&document, UnknownComponentPolicy::Reject)
        .expect("every component is one the game runs");
}

/// The authored properties reach fields the scripts actually declare.
///
/// A property naming a renamed field is refused at runtime rather than ignored,
/// so this would show up as a game that reports an error every frame — worth
/// catching here instead.
#[test]
fn every_authored_property_names_a_field_its_script_exports() {
    let world = world().expect("the scene loads");
    let extractor = extractor().expect("the schemas register");
    let sources = sources();
    let mut scripts = Scripts::new();
    scripts.compile(&world, extractor.components(), &sources);

    let scripted = extractor
        .components()
        .query::<ScriptComponent>(&world)
        .expect("sindri.script is registered");
    assert!(!scripted.is_empty(), "the game has scripts");

    for (_, component) in scripted {
        let exports = scripts
            .exports(&component.source, &component.script)
            .unwrap_or_else(|| panic!("{} did not compile", component.source));
        let declared: BTreeSet<&str> = exports.iter().map(|export| export.name.as_str()).collect();
        for name in component.properties.keys() {
            assert!(
                declared.contains(name.as_str()),
                "{}'s {} sets `{name}`, which it does not @export -- declared: {declared:?}",
                component.source,
                component.script,
            );
        }
    }
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

    let mut world = world().expect("the scene loads");
    let extractor = extractor().expect("the schemas register");
    let sources = sources();
    let mut scripts = Scripts::new();

    let (floor, tilemap) = extractor
        .components()
        .query::<TilemapComponent>(&world)
        .expect("the tilemap schema reads")
        .into_iter()
        .next()
        .expect("Gather has a floor");
    let map = world
        .get(floor)
        .and_then(|data| data.transform_3d)
        .unwrap_or_default();
    let grid = tilemap.grid_space().expect("the floor has a valid grid");

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

            let report = scripts.advance(
                &mut world,
                extractor.components(),
                &sources,
                &held,
                1.0 / 60.0,
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
            &sources,
            &InputState::default(),
            1.0 / 60.0,
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
            data.components["sindri.sprite"]["tint"][3]
                .as_f64()
                .unwrap_or(0.0)
        })
        .expect("the game has a banner");
    assert!(
        (banner - 1.0).abs() < 1.0e-6,
        "the banner shows once the game is won, and it is at {banner}"
    );
}

/// A fresh game starts at nothing, so playing again is playing again.
#[test]
fn starting_over_starts_at_nothing() {
    let mut world = world().expect("the scene loads");
    let extractor = extractor().expect("the schemas register");
    let mut scripts = Scripts::new();
    scripts.advance(
        &mut world,
        extractor.components(),
        &sources(),
        &InputState::default(),
        0.5,
    );
    scripts.clear();
    assert!(!scripts.blackboard().has("score"));
}

fn _sources_are_used(_: &ScriptSources) {}
const _: fn() = || {
    let _ = ScriptComponent::TYPE_NAME;
};

/// The scene ships in canonical form, so editing it in the editor and saving
/// produces the file that is already committed rather than a whole-file diff.
///
/// Regenerate deliberately with
/// `SINDRI_UPDATE_GATHER_SCENE=1 cargo test --package sindri-gather`.
#[test]
fn the_scene_file_is_canonical() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("assets")
        .join("gather.scene.json");
    let stored = std::fs::read_to_string(&path).expect("the scene is readable");
    let canonical = SceneDocument::from_json(&stored)
        .expect("the scene parses")
        .to_canonical_json()
        .expect("the scene serializes");
    if std::env::var_os("SINDRI_UPDATE_GATHER_SCENE").is_some() {
        std::fs::write(&path, &canonical).expect("the scene is writable");
        return;
    }
    assert_eq!(
        stored, canonical,
        "gather.scene.json is not canonical; rerun with SINDRI_UPDATE_GATHER_SCENE=1"
    );
}
