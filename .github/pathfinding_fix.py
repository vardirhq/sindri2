from pathlib import Path

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
test = '''/// The Wisp is real gameplay pathfinding: its first direct east edge is authored\n/// as a wall, so deterministic cardinal A* must route south before approaching\n/// the player. This catches a script that merely moves toward the target while\n/// ignoring the scene navigation components.\n#[test]\nfn the_wisp_routes_around_the_authored_wall() {\n    let mut world = world().expect("the scene loads");\n    let extractor = extractor().expect("the schemas register");\n    let floor = world\n        .entities()\n        .find(|(_, data)| data.source_id.as_ref().is_some_and(|id| id.as_str() == "floor"))\n        .map(|(entity, _)| entity)\n        .expect("the game has a floor");\n    let wisp = world\n        .entities()\n        .find(|(_, data)| data.source_id.as_ref().is_some_and(|id| id.as_str() == "wisp"))\n        .map(|(entity, _)| entity)\n        .expect("the game has a wisp");\n\n    let before = WorldGridNavigation::from_world(&world, floor)\n        .expect("the authored navigation is valid")\n        .placement(wisp)\n        .expect("the wisp is an occupant")\n        .anchor;\n    assert_eq!(before, GridCoord::new(0, 0));\n\n    let mut session = Session::new(extractor.components().clone());\n    session\n        .step(&mut world, &InputState::default(), 0.33)\n        .expect("the pathfinding script steps");\n\n    let after = WorldGridNavigation::from_world(&world, floor)\n        .expect("navigation remains valid")\n        .placement(wisp)\n        .expect("the wisp remains an occupant")\n        .anchor;\n    assert_eq!(\n        after,\n        GridCoord::new(0, 1),\n        "the wall from (0,0) to (1,0) forces the first A* step south"\n    );\n}\n\n'''
text = text.replace(marker, test + marker, 1)
p.write_text(text)
