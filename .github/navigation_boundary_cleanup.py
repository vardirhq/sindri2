from pathlib import Path


def replace(path: str, old: str, new: str, count: int = 1) -> None:
    p = Path(path)
    text = p.read_text()
    if old not in text:
        raise SystemExit(f"missing cleanup anchor in {path}: {old[:100]!r}")
    p.write_text(text.replace(old, new, count))


# Editor now implements the navigation bridge directly, so its dependency must
# be explicit rather than arriving transitively through sindri-scene.
replace(
    "editor/Cargo.toml",
    'sindri-core = { path = "../crates/sindri-core" }\n',
    'sindri-core = { path = "../crates/sindri-core" }\n'
    'sindri-grid = { path = "../crates/sindri-grid" }\n',
)

# Once the public audio host owns the backwards-compatible no-navigation
# constructor, the inner world host only needs the explicit constructor.
p = Path("crates/sindri-decay/src/host.rs")
text = p.read_text()
old = '''    pub fn new(
        world: &'a mut World,
        entity: EntityId,
        context: ScriptContext<'a>,
        blackboard: &'a mut Blackboard,
    ) -> Self {
        Self::with_navigation(world, entity, context, blackboard, None)
    }

'''
if old not in text:
    raise SystemExit("missing now-unused inner WorldHost::new")
text = text.replace(old, "", 1)
p.write_text(text)

# The surface contract intentionally executes every analyzer-visible host call.
# After navigation becomes injected, every WorldHost created by this contract
# needs the fixture provider too. The first boundary script rewrites one host;
# rewrite any remaining constructors here rather than depending on source order.
p = Path("crates/sindri-decay/src/surface.rs")
text = p.read_text()
old = 'WorldHost::new(&mut world, entity, context(&input), &mut board, &mut audio);'
new = '''WorldHost::with_navigation(
                    &mut world,
                    entity,
                    context(&input),
                    &mut board,
                    &mut audio,
                    Some(&OpenNavigation),
                );'''
if old not in text:
    raise SystemExit("missing remaining surface contract WorldHost::new")
text = text.replace(old, new)

# Keep the exhaustive analyzer/runtime contract readable enough for strict
# pedantic Clippy by moving its navigation-specific world setup out of the test
# body. This helper still creates real entities and the test still executes the
# actual host calls; it merely stops one test function doing all the plumbing.
anchor = '''    impl GridNavigationHost for OpenNavigation {
        fn find_path(
            &self,
            _world: &World,
            _mover: EntityId,
            _grid: EntityId,
            goal: GridCoord,
        ) -> Result<Option<Vec<GridCoord>>, String> {
            Ok(Some(vec![GridCoord::new(0, 0), goal]))
        }
    }
'''
helper = anchor + '''
    fn grid_call_entities(world: &mut World) -> (EntityId, EntityId, EntityId) {
        let grid_name = sindri_core::SceneEntityId::new("surface-grid").expect("stable test id");
        let grid = world.spawn(EntityData {
            source_id: Some(grid_name.clone()),
            transform_3d: Some(Transform3D::default()),
            components: [
                (
                    super::TILEMAP_COMPONENT.to_owned(),
                    serde_json::json!({
                        "columns": 2,
                        "rows": 1,
                        "space": "world",
                        "texture": "tiles.png",
                        "palette": ["tile"],
                        "tiles": [0, 0]
                    }),
                ),
                (
                    "sindri.grid_navigation".to_owned(),
                    serde_json::json!({ "walls": [] }),
                ),
            ]
            .into_iter()
            .collect(),
            ..EntityData::default()
        });
        let mover = world.spawn(EntityData {
            transform_3d: Some(Transform3D {
                position: [0.5, -0.5, 0.0],
                ..Transform3D::default()
            }),
            components: [(
                "sindri.grid_occupant".to_owned(),
                serde_json::json!({
                    "grid": grid_name.as_str(),
                    "footprint": [[0, 0]]
                }),
            )]
            .into_iter()
            .collect(),
            ..EntityData::default()
        });
        let target = world.spawn(EntityData {
            transform_3d: Some(Transform3D {
                position: [1.5, -0.5, 0.0],
                ..Transform3D::default()
            }),
            ..EntityData::default()
        });
        (mover, grid, target)
    }
'''
if anchor not in text:
    raise SystemExit("missing OpenNavigation fixture anchor")
text = text.replace(anchor, helper, 1)

old_fixture = '''                let grid_name =
                    sindri_core::SceneEntityId::new("surface-grid").expect("stable test id");
                let grid = world.spawn(EntityData {
                    source_id: Some(grid_name.clone()),
                    transform_3d: Some(Transform3D::default()),
                    components: [
                        (
                            super::TILEMAP_COMPONENT.to_owned(),
                            serde_json::json!({
                                "columns": 2,
                                "rows": 1,
                                "space": "world",
                                "texture": "tiles.png",
                                "palette": ["tile"],
                                "tiles": [0, 0]
                            }),
                        ),
                        (
                            "sindri.grid_navigation".to_owned(),
                            serde_json::json!({ "walls": [] }),
                        ),
                    ]
                    .into_iter()
                    .collect(),
                    ..EntityData::default()
                });
                let mover = world.spawn(EntityData {
                    transform_3d: Some(Transform3D {
                        position: [0.5, -0.5, 0.0],
                        ..Transform3D::default()
                    }),
                    components: [(
                        "sindri.grid_occupant".to_owned(),
                        serde_json::json!({
                            "grid": grid_name,
                            "footprint": [[0, 0]]
                        }),
                    )]
                    .into_iter()
                    .collect(),
                    ..EntityData::default()
                });
                let target = world.spawn(EntityData {
                    transform_3d: Some(Transform3D {
                        position: [1.5, -0.5, 0.0],
                        ..Transform3D::default()
                    }),
                    ..EntityData::default()
                });'''
if old_fixture not in text:
    raise SystemExit("missing inline grid contract fixture")
text = text.replace(old_fixture, '                let (mover, grid, target) = grid_call_entities(&mut world);', 1)
p.write_text(text)

# Rust 1.95's pedantic Clippy quite reasonably dislikes closures whose entire
# career is calling one method. Keep the strict CI clean in both host bridges.
for path in ("game/src/lib.rs", "editor/src/scripts.rs"):
    p = Path(path)
    text = p.read_text()
    old = '.map(|path| path.map(|path| path.into_nodes()))'
    if old not in text:
        raise SystemExit(f"missing route conversion in {path}")
    p.write_text(text.replace(old, '.map(|path| path.map(sindri_grid::GridPath::into_nodes))', 1))

# Gather's broad playthrough test predates the Wisp and drove Scripts directly.
# It must provide the same navigation service as Session now that every scene
# script, including the Wisp, is deliberately part of that end-to-end run.
p = Path("game/tests/the_game_holds_together.rs")
text = p.read_text()
text = text.replace(
    'use sindri_decay::{AudioCommand, ScriptComponent, ScriptSources, Scripts};\n',
    'use sindri_decay::{\n    AudioCommand, GridNavigationHost, ScriptComponent, ScriptSources, Scripts,\n};\n',
    1,
)
text = text.replace(
    'use sindri_grid::{GridCoord, GridPoint, GridSpace, PlanePoint};\n',
    'use sindri_grid::{GridCoord, GridPathfinder, GridPoint, GridSpace, PlanePoint};\n',
    1,
)
anchor = 'const SCENE: &str = include_str!("../assets/gather.scene.json");\n'
provider = anchor + '''
struct TestNavigation;

impl GridNavigationHost for TestNavigation {
    fn find_path(
        &self,
        world: &World,
        mover: sindri_core::EntityId,
        grid: sindri_core::EntityId,
        goal: GridCoord,
    ) -> Result<Option<Vec<GridCoord>>, String> {
        WorldGridNavigation::from_world(world, grid)
            .map_err(|error| error.to_string())?
            .find_path(GridPathfinder::default(), mover, goal)
            .map(|path| path.map(sindri_grid::GridPath::into_nodes))
            .map_err(|error| error.to_string())
    }
}
'''
if anchor not in text:
    raise SystemExit("missing Gather integration-test scene anchor")
text = text.replace(anchor, provider, 1)
old = '''            let report = scripts.advance(
                &mut world,
                extractor.components(),
                &sources,
                &held,
                1.0 / 60.0,
            );'''
new = '''            let report = scripts.advance_with_navigation(
                &mut world,
                extractor.components(),
                &sources,
                &held,
                1.0 / 60.0,
                Some(&TestNavigation),
            );'''
if old not in text:
    raise SystemExit("missing Gather walking harness advance")
text = text.replace(old, new, 1)
old = '''        scripts.advance(
            &mut world,
            extractor.components(),
            &sources,
            &InputState::default(),
            1.0 / 60.0,
        );'''
new = '''        scripts.advance_with_navigation(
            &mut world,
            extractor.components(),
            &sources,
            &InputState::default(),
            1.0 / 60.0,
            Some(&TestNavigation),
        );'''
if old not in text:
    raise SystemExit("missing Gather banner harness advance")
text = text.replace(old, new, 1)
p.write_text(text)
