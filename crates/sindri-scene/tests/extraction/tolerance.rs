//! A tolerant extractor draws around an invalid environment or voxel world
//! instead of failing the frame, and says what it drew around.
//!
//! The failure this guards against: an editor dragging a value out of range
//! failed every frame after it, and its viewport kept showing the last good
//! image under gizmos that still moved, as if the scene had frozen.

use sindri_core::World;
use sindri_render::{FrameCommand, PreparedFrame, TextureId};
use sindri_scene::{CameraView, SceneExtractError, SceneExtractor, TextureBindings};

use crate::support::{VIEWPORT, scene, world_from};

/// A voxel world whose materials are `materials`, and whose generator uses
/// IDs 1, 2 and 3.
fn voxel_world(materials: &[u16]) -> World {
    let materials = materials
        .iter()
        .map(|voxel| {
            format!(
                r#"{{ "voxel": {voxel}, "top": "grass.png", "side": "grass.png",
                     "bottom": "grass.png" }}"#
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    world_from(&scene(&format!(
        r#",
        {{ "id": "terrain", "transform_3d": {{}}, "components": {{
          "sindri.voxel_world": {{
            "generator": {{
              "kind": "layered_terrain", "seed": 7,
              "base_height": 0, "height_variation": 0,
              "surface_voxel": 1, "subsurface_voxel": 2,
              "deep_voxel": 3, "subsurface_depth": 3
            }},
            "materials": [{materials}],
            "focus": [0, 0, 0], "render_radius": 0,
            "vertical_radius": 0, "layer": 0
          }}
        }} }}"#
    )))
}

fn environment(background: f32, bloom_intensity: f32) -> World {
    world_from(&scene(&format!(
        r#",
        {{ "id": "environment", "components": {{
          "sindri.environment": {{
            "background": [{background}, 0.0, 0.0, 1.0],
            "bloom": {{ "enabled": true, "threshold": 0.5, "knee": 0.2,
                        "intensity": {bloom_intensity}, "passes": 3 }}
          }}
        }} }}"#
    )))
}

fn textures() -> TextureBindings {
    let mut bindings = TextureBindings::new();
    bindings.bind("grass.png", TextureId::new(1));
    bindings
}

fn tolerant() -> SceneExtractor {
    let mut extractor = SceneExtractor::new().expect("built-in components register");
    extractor.tolerate_invalid_components();
    extractor
}

fn extract(extractor: &SceneExtractor, world: &World) -> Result<PreparedFrame, SceneExtractError> {
    extractor.extract(world, VIEWPORT, CameraView::default(), &textures())
}

fn voxel_draws(frame: &PreparedFrame) -> usize {
    frame
        .passes()
        .iter()
        .filter(|pass| matches!(pass.command, FrameCommand::CachedTexturedMesh { .. }))
        .count()
}

#[test]
fn a_strict_extractor_still_fails_the_frame() {
    let extractor = SceneExtractor::new().expect("built-in components register");
    assert!(extract(&extractor, &voxel_world(&[1, 1, 2, 3])).is_err());
    assert!(
        extractor.problems().is_empty(),
        "problems are only collected tolerantly"
    );
}

#[test]
fn a_broken_voxel_world_keeps_drawing_what_it_last_could() {
    let extractor = tolerant();
    let valid = extract(&extractor, &voxel_world(&[1, 2, 3])).expect("a valid world extracts");
    let drawn = voxel_draws(&valid);
    assert!(drawn > 0, "the valid world draws its terrain");

    // What the inspector's Add used to write: a copy of material 1.
    let broken = extract(&extractor, &voxel_world(&[1, 2, 3, 1]))
        .expect("a tolerant frame is not failed by one component");
    assert_eq!(
        voxel_draws(&broken),
        drawn,
        "the resident world stays on screen while its definition is invalid"
    );
    let problems = extractor.problems();
    assert_eq!(problems.len(), 1);
    assert_eq!(problems[0].component, "sindri.voxel_world");
    assert!(
        problems[0].message.contains("unique"),
        "{}",
        problems[0].message
    );

    extract(&extractor, &voxel_world(&[1, 2, 3, 4])).expect("the repaired world extracts");
    assert!(
        extractor.problems().is_empty(),
        "a problem lasts as long as its cause, not longer"
    );
}

#[test]
fn a_voxel_world_that_was_never_valid_is_left_out() {
    let extractor = tolerant();
    let frame = extract(&extractor, &voxel_world(&[1, 2]))
        .expect("a tolerant frame is not failed by one component");
    assert_eq!(voxel_draws(&frame), 0);
    let problems = extractor.problems();
    assert_eq!(problems.len(), 1);
    assert!(
        problems[0].message.contains("material ID 3"),
        "{}",
        problems[0].message
    );
}

#[test]
fn an_invalid_environment_keeps_the_last_valid_one() {
    let extractor = tolerant();
    let first = extract(&extractor, &environment(0.25, 0.5)).expect("a valid environment");
    let dragged = extract(&extractor, &environment(0.75, -1.22))
        .expect("a tolerant frame is not failed by the environment");
    let (kept, valid) = (dragged.clear().color[0], first.clear().color[0]);
    assert!(
        (kept - valid).abs() < 1.0e-6,
        "the frame is cleared with the last valid background ({valid}), not the new one ({kept})"
    );
    let problems = extractor.problems();
    assert_eq!(problems.len(), 1);
    assert_eq!(problems[0].component, "sindri.environment");
    assert_eq!(
        problems[0].message,
        "environment `bloom.intensity` must be a finite number of at least 0"
    );
}
