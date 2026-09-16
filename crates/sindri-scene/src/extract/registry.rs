//! Which components ship with the engine, and what each one consists of.
//!
//! The registry owns the complete field template and the safe default payload.

use sindri_core::{ComponentSchemaRegistry, TagsComponent};

use crate::animation::SpriteAnimationComponent;
use crate::audio::AudioSourceComponent;
use crate::components::{
    CameraComponent, GridNavigationComponent, GridOccupantComponent, MeshComponent, ShapeComponent,
    SpriteComponent, TileGridComponent, TileVolumeComponent, TilemapComponent, UiImageComponent,
    UiShapeBlend, UiShapeComponent, UiShapeKind, UiTextComponent,
};
use crate::effects::EffectBurstComponent;
use crate::physics::{Collider2dComponent, RigidBody2dComponent};
use crate::screen_ui::{UiButtonComponent, UiLayoutComponent, UiSliderComponent};
use crate::textures::PROCEDURAL_TEXTURES;

use super::SceneExtractError;

pub(super) fn builtin_components() -> Result<ComponentSchemaRegistry, SceneExtractError> {
    let mut components = ComponentSchemaRegistry::default();
    register_drawables(&mut components)?;
    register_gameplay(&mut components)?;
    super::meanings::describe_builtins(&mut components)?;
    Ok(components)
}

fn register_shapes(components: &mut ComponentSchemaRegistry) -> Result<(), SceneExtractError> {
    components.register_with_default::<ShapeComponent>(
        "Shape",
        serde_json::json!({
            "kind": UiShapeKind::default().as_str(), "count": 6.0,
            "fill": [0.0, 0.0, 0.0, 0.0], "stroke": [0.49, 1.0, 0.77, 1.0],
            "stroke_width": 0.06, "corner_radius": 0.0, "dashes": 0.0,
            "dash_duty": 0.5, "sweep_start": 0.0, "sweep_turns": 1.0,
            "blend": UiShapeBlend::default().as_str(), "layer": 0
        }),
    )?;
    components.register_with_default::<UiShapeComponent>(
        "UI Shape",
        serde_json::json!({
            "kind": UiShapeKind::default().as_str(), "count": 6.0,
            "fill": [0.0, 0.0, 0.0, 0.0], "stroke": [0.49, 1.0, 0.77, 1.0],
            "stroke_width": 0.04, "corner_radius": 0.0, "dashes": 0.0,
            "dash_duty": 0.5, "sweep_start": 0.0, "sweep_turns": 1.0,
            "blend": UiShapeBlend::default().as_str(), "anchor": "center", "layer": 0
        }),
    )?;
    Ok(())
}

fn register_drawables(components: &mut ComponentSchemaRegistry) -> Result<(), SceneExtractError> {
    components.register_with_default::<CameraComponent>(
        "Camera",
        serde_json::json!({
            "projection": CameraComponent::PROJECTIONS[0],
            "vertical_fov_degrees": CameraComponent::DEFAULT_VERTICAL_FOV_DEGREES,
            "near": CameraComponent::DEFAULT_NEAR, "far": CameraComponent::DEFAULT_FAR
        }),
    )?;
    components.register_with_default::<MeshComponent>(
        "Mesh",
        serde_json::json!({ "primitive": "cube", "texture": PROCEDURAL_TEXTURES[0].reference, "layer": 0 }),
    )?;
    components.register_with_default::<SpriteComponent>(
        "Sprite",
        serde_json::json!({
            "texture": PROCEDURAL_TEXTURES[0].reference,
            "tint": [1.0, 1.0, 1.0, 1.0],
            "color_transform": {
                "multiply": [1.0, 1.0, 1.0, 1.0],
                "offset": [0.0, 0.0, 0.0, 0.0]
            },
            "layer": 0
        }),
    )?;
    components.register_with_default::<UiImageComponent>(
        "UI Image",
        serde_json::json!({
            "texture": PROCEDURAL_TEXTURES[0].reference, "anchor": "center",
            "tint": [1.0, 1.0, 1.0, 1.0], "layer": 0,
            "fill": { "amount": 1.0, "from": "left" }
        }),
    )?;
    components.register_with_default::<UiButtonComponent>("UI Button", serde_json::json!({ "label": "" }))?;
    components.register_with_default::<UiSliderComponent>(
        "UI Slider",
        serde_json::json!({
            "label": "", "orientation": "horizontal", "min": 0.0, "max": 1.0,
            "step": 0.0, "value": 0.0, "disabled": false
        }),
    )?;
    components.register_with_default::<UiLayoutComponent>(
        "UI Layout",
        serde_json::json!({ "direction": "column", "spacing": 0.25, "justify": "center", "align": "center" }),
    )?;
    components.register_with_default::<EffectBurstComponent>(
        "Effect Burst",
        serde_json::json!({
            "texture": PROCEDURAL_TEXTURES[0].reference, "count": 12, "speed": 4.0,
            "spread": 0.5, "lifetime": 0.5, "size": 0.1,
            "tint": [1.0, 1.0, 1.0, 1.0], "fade": true, "drag": 0.25, "layer": 0
        }),
    )?;
    components.register_with_fields::<SpriteAnimationComponent>(
        "Sprite Animation", serde_json::json!({ "clips": {}, "playing": null, "speed": 1.0 }),
    )?;
    register_ui_text(components)?;
    register_shapes(components)?;
    register_tiles(components)?;
    Ok(())
}

fn register_tiles(components: &mut ComponentSchemaRegistry) -> Result<(), SceneExtractError> {
    components.register_with_default::<TilemapComponent>(
        "Tilemap",
        serde_json::json!({
            "texture": PROCEDURAL_TEXTURES[0].reference, "palette": [], "columns": 1, "rows": 1,
            "tile_size": [1.0, 1.0], "tile_overhang": 0.0, "projection": "orthogonal",
            "tiles": [null], "tint": [1.0, 1.0, 1.0, 1.0], "layer": 0
        }),
    )?;
    components.register_with_default::<TileGridComponent>(
        "Tile Grid", serde_json::json!({ "columns": 1, "rows": 1, "cell_size": [1.0, 1.0], "level_step": [0.0, 0.5], "projection": "orthogonal" }),
    )?;
    components.register_with_default::<TileVolumeComponent>(
        "Tile Volume", serde_json::json!({ "tileset": "", "cells": [], "layer": 0 }),
    )?;
    Ok(())
}

fn register_ui_text(components: &mut ComponentSchemaRegistry) -> Result<(), SceneExtractError> {
    components.register_with_fields::<UiTextComponent>(
        "UI Text",
        serde_json::json!({
            "text": "Text", "font": "", "font_size": 0.0667, "line_height": 0.0833,
            "color": [1.0, 1.0, 1.0, 1.0], "anchor": "center", "layer": 0, "values": [],
            "bounds": [0.0, 0.0], "wrap": "none", "line_align": "follow", "letter_spacing": 0.0,
            "bold": false, "italic": false, "case": "as_written", "visible": -1.0,
            "outline": { "width": 0.0, "color": [0.0, 0.0, 0.0, 1.0] },
            "shadow": { "offset": [0.0, 0.0], "color": [0.0, 0.0, 0.0, 1.0], "softness": 0.0 },
            "auto_size": { "enabled": false, "min": 0.02 }
        }),
    )?;
    Ok(())
}

fn register_gameplay(components: &mut ComponentSchemaRegistry) -> Result<(), SceneExtractError> {
    components.register_with_default::<TagsComponent>("Tags", serde_json::json!({ "tags": [] }))?;
    components.register_with_default::<GridNavigationComponent>("Grid Navigation", serde_json::json!({ "walls": [] }))?;
    components.register_with_fields::<GridOccupantComponent>("Grid Occupant", serde_json::json!({ "grid": "", "footprint": [[0, 0]] }))?;
    components.register_with_default::<RigidBody2dComponent>(
        "Rigid Body 2D",
        serde_json::json!({
            "kind": "dynamic", "pose": { "position": [0.0, 0.0], "rotation": 0.0 },
            "linear_velocity": [0.0, 0.0], "angular_velocity": 0.0, "gravity_scale": 1.0,
            "linear_damping": 0.0, "angular_damping": 0.0, "lock_rotation": false, "continuous": false
        }),
    )?;
    components.register_with_default::<Collider2dComponent>(
        "Collider 2D",
        serde_json::json!({
            "pieces": [{
                "shape": { "shape": "box", "half_extents": [0.5, 0.5] },
                "offset": [0.0, 0.0], "rotation": 0.0, "sensor": false,
                "friction": 0.5, "restitution": 0.0,
                "layers": { "memberships": 1, "filter": 4294967295u32 }
            }]
        }),
    )?;
    components.register_with_default::<AudioSourceComponent>(
        "Audio Source",
        serde_json::json!({ "clip": "", "volume": 1.0, "looping": false, "autoplay": false }),
    )?;
    Ok(())
}
