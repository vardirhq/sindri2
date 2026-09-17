use glam::Mat4;
use serde_json::json;
use sindri_core::{
    SCENE_FORMAT_VERSION, SceneDocument, SceneEntity, SceneEntityId, SceneMetadata,
    TileSetDocument, Transform3D, World,
};
use sindri_scene::{OcclusionProbe, SceneExtractor, TileSetBindings};

use super::OcclusionOverlay;

fn id(value: &str) -> SceneEntityId {
    SceneEntityId::new(value).expect("a valid scene ID")
}

/// Baked block art, at the proportions baked block art has.
///
/// A face sprite is routinely much taller than the cell it belongs to -- a
/// whole block drawn into one quad, mostly transparent -- because a visual may
/// overflow its cell without changing what the cell means. That overflow is
/// what reaches a walker standing nearby, so a fixture with tidy half-cell
/// quads would find nothing and prove nothing.
fn tile_sets() -> TileSetBindings {
    let document = TileSetDocument::from_json(
        r#"{ "format_version": 1, "tiles": { "block": { "faces": {
             "top": { "sprite": "b.png#0", "size": [1.0, 1.9] },
             "south": { "sprite": "b.png#1", "size": [1.0, 1.9] }
           } } } }"#,
    )
    .expect("the tile set decodes");
    let mut bindings = TileSetBindings::new();
    bindings.bind("world.tileset.json", document).unwrap();
    bindings
}

/// A flat floor, plus one raised block so the wall corner is reachable.
fn world_with_raised(raised: bool) -> (World, SceneExtractor) {
    let mut cells: Vec<_> = (0..4)
        .flat_map(|row| {
            (0..4).map(move |column| json!({ "position": [column, row, 0], "tile": "block" }))
        })
        .collect();
    if raised {
        cells.push(json!({ "position": [2, 2, 1], "tile": "block" }));
    }
    let mut floor = SceneEntity::new(id("floor"));
    floor.transform_3d = Some(Transform3D::default());
    floor.components.insert(
        "sindri.tile_grid".to_owned(),
        json!({
            "columns": 4, "rows": 4, "cell_size": [1.0, 0.5],
            "level_step": [0.0, 0.5], "projection": "isometric", "depth_step": 0.01
        }),
    );
    floor.components.insert(
        "sindri.tile_volume".to_owned(),
        json!({ "tileset": "world.tileset.json", "cells": cells }),
    );
    let document = SceneDocument {
        format_version: SCENE_FORMAT_VERSION,
        metadata: SceneMetadata::default(),
        entities: vec![floor],
    };
    let extractor = SceneExtractor::new().expect("the schemas register");
    let mut world = World::default();
    sindri_core::LoadedScenes::new()
        .enter_keeping_identities(&mut world, "test", &document)
        .expect("the scene loads");
    (world, extractor)
}

#[test]
fn a_disabled_overlay_does_no_work() {
    let (world, extractor) = world_with_raised(true);
    let mut overlay = OcclusionOverlay::default();
    overlay.ensure(
        &world,
        extractor.components(),
        &tile_sets(),
        OcclusionProbe::default(),
    );
    assert_eq!(
        overlay.report().probes,
        0,
        "a hidden overlay should not be sweeping"
    );
}

#[test]
fn an_enabled_overlay_sweeps_and_then_caches() {
    let (world, extractor) = world_with_raised(true);
    let sets = tile_sets();
    let mut overlay = OcclusionOverlay {
        enabled: true,
        ..Default::default()
    };
    overlay.ensure(
        &world,
        extractor.components(),
        &sets,
        OcclusionProbe::default(),
    );
    let probes = overlay.report().probes;
    assert!(probes > 0, "the floor should be swept");

    // The second call must not sweep again: the report is the same object, so
    // the count staying put is the only observable difference between a cache
    // hit and a repeat.
    let before = overlay.report().faces;
    overlay.ensure(
        &world,
        extractor.components(),
        &sets,
        OcclusionProbe::default(),
    );
    assert_eq!(overlay.report().probes, probes);
    assert_eq!(overlay.report().faces, before);
}

/// Editing the volume has to re-sweep, or the marks describe the old scene.
///
/// The fingerprint is what makes that work without this module being told an
/// edit happened, and undo is the same thing in reverse.
#[test]
fn changing_the_volume_re_sweeps() {
    let sets = tile_sets();
    let probe = OcclusionProbe::default();

    let (flat, extractor) = world_with_raised(false);
    let mut overlay = OcclusionOverlay {
        enabled: true,
        ..Default::default()
    };
    overlay.ensure(&flat, extractor.components(), &sets, probe);
    let flat_findings = overlay.report().findings.len();

    let (raised, extractor) = world_with_raised(true);
    overlay.ensure(&raised, extractor.components(), &sets, probe);
    assert_ne!(
        overlay.report().findings.len(),
        flat_findings,
        "raising a block reaches the wall corner, so the sweep should differ"
    );
}

/// A mark is only useful if it lands on the cell it describes.
#[test]
fn faults_project_into_the_viewport() {
    let (world, extractor) = world_with_raised(true);
    let mut overlay = OcclusionOverlay {
        enabled: true,
        ..Default::default()
    };
    overlay.ensure(
        &world,
        extractor.components(),
        &tile_sets(),
        OcclusionProbe::default(),
    );
    assert!(
        !overlay.report().is_clean(),
        "the raised block should reach the corner: {:?}",
        overlay.report()
    );

    let marks = overlay.marks(&world, extractor.components(), Mat4::IDENTITY);
    assert_eq!(
        marks.len(),
        overlay.report().findings.len(),
        "every finding should project"
    );
    for mark in &marks {
        for point in mark.standing.iter().chain(mark.covering.iter()) {
            assert!(
                point[0].is_finite() && point[1].is_finite(),
                "a mark lands somewhere: {point:?}"
            );
        }
    }
}

/// Two grids in a scene are two spaces, and a mark drawn through the wrong one
/// lands somewhere convincing and false.
#[test]
fn a_missing_grid_drops_its_marks_rather_than_guessing() {
    let (mut world, extractor) = world_with_raised(true);
    let mut overlay = OcclusionOverlay {
        enabled: true,
        ..Default::default()
    };
    overlay.ensure(
        &world,
        extractor.components(),
        &tile_sets(),
        OcclusionProbe::default(),
    );
    assert!(!overlay.report().is_clean());

    let floor = world
        .entities()
        .find(|(_, data)| {
            data.source_id
                .as_ref()
                .is_some_and(|value| value.as_str() == "floor")
        })
        .map(|(entity, _)| entity)
        .expect("the floor is in the world");
    if let Some(data) = world.get_mut(floor) {
        data.components.remove("sindri.tile_grid");
    }
    assert!(
        overlay
            .marks(&world, extractor.components(), Mat4::IDENTITY)
            .is_empty(),
        "without a grid there is no space to draw in"
    );
}
