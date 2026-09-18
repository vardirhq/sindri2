//! What a builder script reads about the block under the pointer.

use serde_json::json;
use sindri_core::{
    ComponentSchemaRegistry, EntityData, EntityId, SceneComponent, Transform3D, World,
};
use sindri_decay::{ScriptComponent, ScriptFailure, ScriptFrame, ScriptSources, Scripts};
use sindri_platform::InputState;
use sindri_scene::voxel::VolumeAim;

fn registry() -> ComponentSchemaRegistry {
    let mut registry = ComponentSchemaRegistry::default();
    registry
        .register::<ScriptComponent>("Script")
        .expect("sindri.script registers");
    registry
}

/// One scripted entity that writes what it aims at into its own transform,
/// which is the cheapest thing a test can read back out.
fn scripted(script: &str) -> (World, EntityId, ScriptSources) {
    let mut world = World::default();
    let actor = world.spawn(EntityData {
        transform_3d: Some(Transform3D::default()),
        components: [(
            ScriptComponent::TYPE_NAME.to_owned(),
            json!({ "source": "aim.decay", "script": "Builder" }),
        )]
        .into_iter()
        .collect(),
        ..EntityData::default()
    });
    let mut sources = ScriptSources::new();
    sources.insert("aim.decay", script);
    (world, actor, sources)
}

fn run(world: &mut World, sources: &ScriptSources, aim: Option<VolumeAim>) -> Vec<ScriptFailure> {
    let input = InputState::default();
    let frame = ScriptFrame::new(sources, &input, 1.0 / 60.0);
    let frame = match aim {
        Some(aim) => frame.with_aim(aim),
        None => frame,
    };
    Scripts::new().advance(world, &registry(), frame).failures
}

fn aim_on(cell: [i32; 3], face: sindri_core::TileFace) -> VolumeAim {
    let hit = sindri_scene::voxel::VoxelHit {
        cell: sindri_grid::GridCoord3::new(cell[0], cell[1], cell[2]),
        face,
    };
    VolumeAim {
        grid: EntityId::from_bits(0),
        cell: hit.cell,
        face,
        against: hit.against(),
    }
}

const READS_THE_AIM: &str = r"
    script Builder {
        fn update(dt: f32) {
            if Aim.hit {
                this.transform.position.x = Aim.x;
                this.transform.position.y = Aim.y;
                this.transform.position.z = Aim.z;
                this.transform.scale.x = Aim.place_x;
                this.transform.scale.y = Aim.place_y;
                this.transform.scale.z = Aim.place_z;
            } else {
                this.transform.position.x = -1.0;
            }
        }
    }
";

#[test]
fn a_script_reads_the_block_under_the_pointer_and_the_one_beside_its_face() {
    // Both halves of a click, in one read. The cell is what a right-click
    // removes; the place is what a left-click fills, and they differ by the
    // face -- which is the reason a pick reports a side at all.
    let (mut world, actor, sources) = scripted(READS_THE_AIM);
    let failures = run(
        &mut world,
        &sources,
        Some(aim_on([2, 3, 1], sindri_core::TileFace::Top)),
    );
    assert!(failures.is_empty(), "{failures:?}");

    let transform = world
        .get(actor)
        .and_then(|data| data.transform_3d)
        .expect("the actor kept its transform");
    assert_eq!(
        transform.position.map(f32::to_bits),
        [2.0, 3.0, 1.0].map(f32::to_bits)
    );
    assert_eq!(
        transform.scale.map(f32::to_bits),
        [2.0, 3.0, 2.0].map(f32::to_bits),
        "a block attached to a top face goes one level up"
    );
}

#[test]
fn a_face_on_the_side_places_beside_the_block_rather_than_above_it() {
    // The same block, a different side, a different answer. Without this the
    // face could be ignored entirely and placing would always stack.
    let (mut world, actor, sources) = scripted(READS_THE_AIM);
    run(
        &mut world,
        &sources,
        Some(aim_on([2, 3, 1], sindri_core::TileFace::East)),
    );
    let scale = world
        .get(actor)
        .and_then(|data| data.transform_3d)
        .expect("the actor kept its transform")
        .scale;
    assert_eq!(
        scale.map(f32::to_bits),
        [3.0, 3.0, 1.0].map(f32::to_bits),
        "a block attached to an east face goes one column across"
    );
}

#[test]
fn pointing_at_nothing_is_false_rather_than_the_cell_at_the_origin() {
    // The trap this guards. Every cell value reads zero with no aim, and zero
    // is a real cell, so a host that picked nothing must not look to a script
    // like a person pointing at the corner of the world.
    let (mut world, actor, sources) = scripted(READS_THE_AIM);
    let failures = run(&mut world, &sources, None);
    assert!(failures.is_empty(), "{failures:?}");
    assert_eq!(
        world
            .get(actor)
            .and_then(|data| data.transform_3d)
            .expect("the actor kept its transform")
            .position[0]
            .to_bits(),
        (-1.0_f32).to_bits(),
        "a host with nothing under the pointer reports nothing, not cell zero"
    );
}
