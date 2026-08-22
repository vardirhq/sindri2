from pathlib import Path

# One editor test asks which components are offered when a font exists but no
# sprite/grid exists. The pathfinding-aware helper now needs that grid context.
p = Path('editor/src/native.rs')
text = p.read_text()
old = '''        let offered: Vec<String> = addable_components(
            extractor.components(),
            &present,
            Some("fonts/Inter.ttf"),
            None,
        )'''
new = '''        let offered: Vec<String> = addable_components(
            extractor.components(),
            &present,
            Some("fonts/Inter.ttf"),
            None,
            None,
        )'''
if old not in text:
    raise SystemExit('missing editor addable-components test call')
text = text.replace(old, new, 1)
p.write_text(text)

# The generic surface contract gives every Entity argument the same spare
# entity. Make that spare a self-contained one-cell grid occupant as well as a
# tilemap, so Grid.can_reach/step_toward exercise a valid navigation call rather
# than failing because the test fixture omitted the components they require.
p = Path('crates/sindri-decay/src/surface.rs')
text = p.read_text()
old = '''                let spare = world.spawn(EntityData {
                    transform_3d: Some(Transform3D::default()),
                    components: [(
                        super::TILEMAP_COMPONENT.to_owned(),
                        serde_json::json!({
                            "columns": 1,
                            "rows": 1,
                            "space": "world",
                            "texture": "tiles.png",
                            "palette": ["tile"],
                            "tiles": [0]
                        }),
                    )]
                    .into_iter()
                    .collect(),
                    ..EntityData::default()
                });'''
new = '''                let spare = world.spawn(EntityData {
                    source_id: Some("surface-grid".parse().expect("stable test id")),
                    transform_3d: Some(Transform3D::default()),
                    components: [
                        (
                            super::TILEMAP_COMPONENT.to_owned(),
                            serde_json::json!({
                                "columns": 1,
                                "rows": 1,
                                "space": "world",
                                "texture": "tiles.png",
                                "palette": ["tile"],
                                "tiles": [0]
                            }),
                        ),
                        (
                            "sindri.grid_navigation".to_owned(),
                            serde_json::json!({ "walls": [] }),
                        ),
                        (
                            "sindri.grid_occupant".to_owned(),
                            serde_json::json!({
                                "grid": "surface-grid",
                                "footprint": [[0, 0]]
                            }),
                        ),
                    ]
                    .into_iter()
                    .collect(),
                    ..EntityData::default()
                });'''
if old not in text:
    raise SystemExit('missing Decay surface spare fixture')
text = text.replace(old, new, 1)
p.write_text(text)

# Put new Grid calls in the checked API table, not in prose the contract test ignores.
p = Path('docs/scripting.md')
text = p.read_text()
text = text.replace(
    '| `Grid.place(entity, grid, x, y)` | nothing |\n',
    '| `Grid.place(entity, grid, x, y)` | nothing |\n'
    '| `Grid.can_reach(mover, grid, target)` | `bool` |\n'
    '| `Grid.step_toward(mover, grid, target)` | `bool` |\n',
    1,
)
text = text.replace(
    '\n### Pathfinding\n\n`Grid.can_reach(mover, grid, target)` asks whether the mover\'s authored footprint has a route to the target\'s current cell. `Grid.step_toward(mover, grid, target)` performs one deterministic cardinal A* step and returns whether it moved. Both use `sindri.grid_navigation` walls and `sindri.grid_occupant` footprints from the scene.\n',
    '\n',
)
p.write_text(text)

# Prove Gather uses the authored wall, rather than merely carrying the components.
p = Path('game/tests/the_game_holds_together.rs')
text = p.read_text()
text = text.replace(
    'use sindri_gather::{AUDIO, FONTS, extractor, sources, world};\n',
    'use sindri_gather::{AUDIO, FONTS, Session, extractor, sources, world};\n',
    1,
)
text = text.replace(
    'use sindri_grid::{GridPoint, GridSpace, PlanePoint};\n',
    'use sindri_grid::{GridCoord, GridPoint, GridSpace, PlanePoint};\n',
    1,
)
text = text.replace(
    'use sindri_scene::{SceneExtractor, TilemapComponent};\n',
    'use sindri_scene::{SceneExtractor, TilemapComponent, WorldGridNavigation};\n',
    1,
)
marker = '/// The game is playable: walking into an orb collects it, and collecting them\n'
if marker not in text:
    raise SystemExit('missing Gather test insertion marker')
test = '''/// The Wisp is real gameplay pathfinding: its first direct east edge is authored
/// as a wall, so deterministic cardinal A* must route south before approaching
/// the player. This catches a script that merely moves toward the target while
/// ignoring the scene navigation components.
#[test]
fn the_wisp_routes_around_the_authored_wall() {
    let mut world = world().expect("the scene loads");
    let extractor = extractor().expect("the schemas register");
    let floor = world
        .entities()
        .find(|(_, data)| data.source_id.as_ref().is_some_and(|id| id.as_str() == "floor"))
        .map(|(entity, _)| entity)
        .expect("the game has a floor");
    let wisp = world
        .entities()
        .find(|(_, data)| data.source_id.as_ref().is_some_and(|id| id.as_str() == "wisp"))
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
        .step(&mut world, &InputState::default(), 0.33)
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

'''
text = text.replace(marker, test + marker, 1)
p.write_text(text)
