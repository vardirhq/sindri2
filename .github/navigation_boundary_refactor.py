from pathlib import Path

p = Path("crates/sindri-decay/src/surface.rs")
text = p.read_text()

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
if anchor not in text:
    raise SystemExit("missing OpenNavigation fixture anchor")

helper = anchor + '''
    fn grid_call_entities(
        world: &mut World,
        checked: usize,
    ) -> (EntityId, EntityId, EntityId) {
        let grid_name = format!("surface-grid-{checked}");
        let grid = world.spawn(EntityData {
            source_id: Some(
                sindri_core::SceneEntityId::new(grid_name.clone()).expect("stable test id"),
            ),
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
        });
        (mover, grid, target)
    }
'''
text = text.replace(anchor, helper, 1)

start_marker = '                let grid_name = format!("surface-grid-{checked}");\n'
end_marker = '                let mut host =\n'
start = text.find(start_marker)
if start < 0:
    raise SystemExit("missing grid fixture start")
end = text.find(end_marker, start)
if end < 0:
    raise SystemExit("missing grid fixture end")
text = (
    text[:start]
    + '                let (mover, grid, target) = grid_call_entities(&mut world, checked);\n'
    + text[end:]
)
p.write_text(text)
