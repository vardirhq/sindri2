//! Which components ship with the engine, and what each one consists of.
//!
//! Two things are recorded per type and they are not the same thing.
//!
//! The **field template** is what the component *has*: every field, at the
//! value it takes when nobody has said. An editor draws a component by filling
//! this out with whatever the instance stored, so the same component shows the
//! same rows however it was authored. The registry checks each template against
//! what serde will actually ask the type for, so one that drifts from its
//! struct is a startup error rather than a row quietly missing from a panel.
//!
//! The **default payload** is what a *fresh* one is. Only some types have one:
//! a component that names a font, a sheet, another entity, or a clip has no
//! blank the engine can invent, and a button that adds a component the engine
//! then rejects is worse than no button. Those types are registered with fields
//! and no default, and whoever owns a project — the editor — completes them.
//!
//! Conflating the two is what made `sindri.ui.text` inspect as two rows when it
//! has seven.

use sindri_core::{ComponentSchemaRegistry, TagsComponent};

use crate::animation::SpriteAnimationComponent;
use crate::audio::AudioSourceComponent;
use crate::components::{
    CameraComponent, GridNavigationComponent, GridOccupantComponent, MeshComponent,
    SpriteComponent, TilemapComponent, UiImageComponent, UiTextComponent,
};
use crate::physics::{Collider2dComponent, RigidBody2dComponent};
use crate::textures::PROCEDURAL_TEXTURES;

use super::SceneExtractError;

/// The registry every `SceneExtractor` starts from.
pub(super) fn builtin_components() -> Result<ComponentSchemaRegistry, SceneExtractError> {
    let mut components = ComponentSchemaRegistry::default();
    register_drawables(&mut components)?;
    register_gameplay(&mut components)?;
    Ok(components)
}

/// Everything a scene puts on the screen.
fn register_drawables(components: &mut ComponentSchemaRegistry) -> Result<(), SceneExtractError> {
    // Each default is what a freshly added component of that type looks
    // like, and is what makes the type addable at all. They are chosen to
    // be visible rather than neutral: a sprite added to an entity should
    // appear, or the author is left wondering whether the click worked.
    components.register_with_default::<CameraComponent>(
        "Camera",
        serde_json::json!({
            "projection": CameraComponent::PROJECTIONS[0],
            "vertical_fov_degrees": CameraComponent::DEFAULT_VERTICAL_FOV_DEGREES,
            "near": CameraComponent::DEFAULT_NEAR,
            "far": CameraComponent::DEFAULT_FAR
        }),
    )?;
    components.register_with_default::<MeshComponent>(
        "Mesh",
        serde_json::json!({
            "primitive": "cube",
            "texture": PROCEDURAL_TEXTURES[0].reference,
            "layer": 0
        }),
    )?;
    components.register_with_default::<SpriteComponent>(
        "Sprite",
        serde_json::json!({
            "texture": PROCEDURAL_TEXTURES[0].reference,
            "tint": [1.0, 1.0, 1.0, 1.0],
            "layer": 0
        }),
    )?;
    // The UI half of the same picture: anchored to the middle of the
    // viewport, because an element that appears off the edge of the screen
    // looks like a click that did nothing.
    components.register_with_default::<UiImageComponent>(
        "UI Image",
        serde_json::json!({
            "texture": PROCEDURAL_TEXTURES[0].reference,
            "anchor": "center",
            "tint": [1.0, 1.0, 1.0, 1.0],
            "layer": 0
        }),
    )?;
    // Fields but no default: an animation with no sheet and no clips is a
    // component that does nothing, and one with an invented sheet would
    // claim a texture is laid out a way it is not. The editor completes it
    // from the entity's own sheet.
    components.register_with_fields::<SpriteAnimationComponent>(
        "Sprite Animation",
        serde_json::json!({ "clips": {}, "playing": null, "speed": 1.0 }),
    )?;
    // Fields but no default: unlike a procedural texture, there is no
    // honest font the engine can invent, and the editor's font picker
    // supplies a project asset when text is added. The fields are still
    // named, because they are what the component *is* — without them a
    // panel showed two rows for a component with seven, and the same text
    // authored by hand and added by a button were different components.
    components.register_with_fields::<UiTextComponent>(
        "UI Text",
        serde_json::json!({
            "text": "Text",
            "font": "",
            "font_size": 24.0,
            "line_height": 30.0,
            "color": [1.0, 1.0, 1.0, 1.0],
            "anchor": "center",
            "layer": 0
        }),
    )?;
    // A one-by-one map of one empty cell: the smallest tilemap that is
    // still a valid one, so adding the component in the editor gives
    // something to paint into rather than something to repair.
    components.register_with_default::<TilemapComponent>(
        "Tilemap",
        serde_json::json!({
            "texture": PROCEDURAL_TEXTURES[0].reference,
            "palette": [],
            "columns": 1,
            "rows": 1,
            "tile_size": [1.0, 1.0],
            "projection": "orthogonal",
            "tiles": [null],
            "tint": [1.0, 1.0, 1.0, 1.0],
            "layer": 0
        }),
    )?;
    Ok(())
}

/// Everything else a scene carries: how it navigates, how it collides, and what
/// it sounds like.
fn register_gameplay(components: &mut ComponentSchemaRegistry) -> Result<(), SceneExtractError> {
    // A fresh set of tags is empty rather than invented: the engine has no
    // opinion about what an entity is, and a tag it made up would be one a
    // query silently answered with.
    components.register_with_default::<TagsComponent>("Tags", serde_json::json!({ "tags": [] }))?;
    components.register_with_default::<GridNavigationComponent>(
        "Grid Navigation",
        serde_json::json!({ "walls": [] }),
    )?;
    // Fields but no default: an occupant must name the stable ID of the
    // grid it belongs to. Inventing one would create a valid-looking
    // component that cannot ever resolve, so the editor supplies a grid
    // that exists or does not offer the component.
    components.register_with_fields::<GridOccupantComponent>(
        "Grid Occupant",
        serde_json::json!({ "grid": "", "footprint": [[0, 0]] }),
    )?;
    // Physics defaults are ordinary Sindri values rather than backend
    // values. A newly added body starts dynamic and a collider starts as a
    // one-unit box, so both are immediately valid and visible in the
    // generic command-backed inspector.
    components.register_with_default::<RigidBody2dComponent>(
        "Rigid Body 2D",
        serde_json::json!({
            "kind": "dynamic",
            "pose": { "position": [0.0, 0.0], "rotation": 0.0 },
            "linear_velocity": [0.0, 0.0],
            "angular_velocity": 0.0,
            "gravity_scale": 1.0,
            "linear_damping": 0.0,
            "angular_damping": 0.0,
            "lock_rotation": false
        }),
    )?;
    components.register_with_default::<Collider2dComponent>(
        "Collider 2D",
        serde_json::json!({
            "shape": { "shape": "box", "half_extents": [0.5, 0.5] },
            "offset": [0.0, 0.0],
            "rotation": 0.0,
            "sensor": false,
            "layers": { "memberships": 4_294_967_295_u32, "filter": 4_294_967_295_u32 },
            "friction": 0.5,
            "restitution": 0.0
        }),
    )?;
    // Fields but no default, for the reason the registry states: a blank
    // one would name the empty clip, and a button that adds a component the
    // engine then rejects is worse than no button. The editor's clip picker
    // supplies a project asset, as its font picker does for text.
    components.register_with_fields::<AudioSourceComponent>(
        "Audio Source",
        serde_json::json!({
            "clip": "",
            "autoplay": false,
            "looping": false,
            "volume": 1.0
        }),
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::builtin_components;

    /// Every built-in says what it consists of.
    ///
    /// The check that stops the next component being registered with
    /// `register` by habit: without a field template its panel shows whatever
    /// fields its payload happens to carry, which is how one added by a button
    /// and one authored by hand became different-looking components.
    #[test]
    fn every_built_in_component_names_its_fields() {
        let components = builtin_components().expect("the built-in schemas register");
        for metadata in components.registered_components() {
            let fields = components
                .fields(&metadata.type_name)
                .unwrap_or_else(|| panic!("{} has no field template", metadata.type_name));
            assert!(
                fields.as_object().is_some_and(|fields| !fields.is_empty()),
                "{} has an empty field template",
                metadata.type_name
            );
        }
    }

    /// Which components have no honest blank, listed so that adding a sixth is
    /// a decision rather than an omission.
    ///
    /// Each of these names something only a project can supply — a font, a
    /// sheet, another entity, a clip. They inspect like any other component;
    /// they simply cannot be created without a host that knows what is lying
    /// beside the scene.
    #[test]
    fn only_the_components_that_name_a_project_asset_lack_a_default() {
        let components = builtin_components().expect("the built-in schemas register");
        let uncreatable: BTreeSet<&str> = components
            .registered_components()
            .filter(|metadata| components.default_payload(&metadata.type_name).is_none())
            .map(|metadata| metadata.type_name.as_str())
            .collect();
        assert_eq!(
            uncreatable,
            BTreeSet::from([
                "sindri.animation.sprite",
                "sindri.audio.source",
                "sindri.grid.occupant",
                "sindri.ui.text",
            ])
        );
    }
}
