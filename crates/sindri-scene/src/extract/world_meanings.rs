//! What the fields of the scene-wide components mean: the environment and the
//! voxel world.
//!
//! Both were declared with no meanings at all, so the inspector drew every
//! field by shape. A colour was three or four unlabelled numbers, a shadow map
//! size was a drag through every integer, and a value the engine refuses was
//! one careless drag away. Each range here is the one the component's own
//! validation enforces; the tests below hold the two together.

use sindri_core::{AssetKind, ComponentSchemaRegistry, FieldMeaning};

use super::SceneExtractError;
use super::voxel_world::{MAX_HEIGHT_VARIATION, MAX_RESIDENCY_RADIUS};
use crate::components::{EnvironmentComponent, EnvironmentToneMapping, VoxelWorldComponent};

const COLOUR: FieldMeaning = FieldMeaning::Colour;

const fn range(min: f64, max: f64) -> FieldMeaning {
    FieldMeaning::Range { min, max }
}

const fn at_least(min: f64) -> FieldMeaning {
    FieldMeaning::Range {
        min,
        max: f64::INFINITY,
    }
}

const UNIT: FieldMeaning = range(0.0, 1.0);

pub(super) fn describe_world(
    components: &mut ComponentSchemaRegistry,
) -> Result<(), SceneExtractError> {
    describe_environment(components)?;
    describe_voxel_world(components)
}

/// Ranges mirror `EnvironmentComponent::validate`. Where the engine only
/// refuses zero, as with fog distance, the lower end is a small positive
/// number rather than zero, so every value the control can reach is one the
/// engine accepts.
fn describe_environment(components: &mut ComponentSchemaRegistry) -> Result<(), SceneExtractError> {
    components.describe::<EnvironmentComponent>([
        ("background", COLOUR),
        ("ambient_color", COLOUR),
        ("ambient_intensity", at_least(0.0)),
        ("directional.color", COLOUR),
        ("directional.intensity", at_least(0.0)),
        ("shadows.distance", at_least(1.0)),
        (
            "shadows.map_size",
            FieldMeaning::OneOf(vec![256, 512, 1024, 2048]),
        ),
        ("shadows.bias", range(0.0, 0.05)),
        ("ambient_occlusion.strength", UNIT),
        ("fog.color", COLOUR),
        ("fog.start", at_least(0.0)),
        ("fog.distance", at_least(0.01)),
        ("fog.density", UNIT),
        ("fog.height_falloff", UNIT),
        ("post_process.exposure", range(-8.0, 8.0)),
        ("post_process.contrast", range(0.0, 4.0)),
        ("post_process.saturation", range(0.0, 4.0)),
        (
            "post_process.tone_mapping",
            FieldMeaning::choice(EnvironmentToneMapping::NAMES),
        ),
        ("post_process.vignette", UNIT),
        ("bloom.threshold", at_least(0.0)),
        ("bloom.knee", range(1.0e-4, 1.0)),
        ("bloom.intensity", at_least(0.0)),
        ("bloom.passes", range(1.0, 8.0)),
    ])?;
    Ok(())
}

/// A voxel world's generator names its materials by ID, and an ID nothing
/// defines is a world the engine refuses. Declaring the IDs as keys is what
/// lets a tool offer only the materials that exist, number a new one so it
/// does not collide, and refuse to remove one the terrain is made of.
///
/// A material's faces name a texture or a sprite cut from one, which is what
/// the texture picker offers.
fn describe_voxel_world(components: &mut ComponentSchemaRegistry) -> Result<(), SceneExtractError> {
    const MATERIAL: &str = "materials[].voxel";
    let texture = || FieldMeaning::Asset(AssetKind::Texture);
    components.describe::<VoxelWorldComponent>([
        (MATERIAL, FieldMeaning::Key),
        ("materials[].top", texture()),
        ("materials[].side", texture()),
        ("materials[].bottom", texture()),
        ("generator.kind", FieldMeaning::choice(["layered_terrain"])),
        ("generator.surface_voxel", FieldMeaning::KeyOf(MATERIAL)),
        ("generator.subsurface_voxel", FieldMeaning::KeyOf(MATERIAL)),
        ("generator.deep_voxel", FieldMeaning::KeyOf(MATERIAL)),
        ("generator.seed", at_least(0.0)),
        (
            "generator.height_variation",
            range(0.0, f64::from(MAX_HEIGHT_VARIATION)),
        ),
        ("generator.subsurface_depth", at_least(0.0)),
        ("render_radius", range(0.0, f64::from(MAX_RESIDENCY_RADIUS))),
        (
            "vertical_radius",
            range(0.0, f64::from(MAX_RESIDENCY_RADIUS)),
        ),
    ])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use serde_json::Value;
    use sindri_core::FieldMeaning;

    use super::super::registry::builtin_components;
    use crate::components::EnvironmentComponent;

    /// Writes `number` at a dotted path, as an integer where the field holds
    /// one, because serde refuses `8.0` for a `u32`.
    fn set(payload: &mut Value, path: &str, number: f64) {
        let pointer = format!("/{}", path.replace('.', "/"));
        let slot = payload.pointer_mut(&pointer).expect("the path exists");
        *slot = if slot.is_u64() || slot.is_i64() {
            #[allow(clippy::cast_possible_truncation)]
            Value::from(number as i64)
        } else {
            Value::from(number)
        };
    }

    fn accepts(path: &str, number: f64) -> bool {
        let mut payload =
            serde_json::to_value(EnvironmentComponent::default()).expect("it serializes");
        set(&mut payload, path, number);
        serde_json::from_value::<EnvironmentComponent>(payload)
            .ok()
            .and_then(|environment| environment.validate().ok())
            .is_some()
    }

    /// Every value a control can reach is one the engine accepts. A range
    /// wider than validation would let the inspector write an environment the
    /// renderer refuses, which is the bug these meanings exist to prevent.
    #[test]
    fn every_environment_range_is_one_validation_accepts() {
        let components = builtin_components().expect("the built-ins register");
        let mut checked = 0;
        for (path, meaning) in components.meanings("sindri.environment") {
            match meaning {
                FieldMeaning::Range { min, max } => {
                    assert!(accepts(path, *min), "{path} refuses its own minimum {min}");
                    if max.is_finite() {
                        assert!(accepts(path, *max), "{path} refuses its own maximum {max}");
                    }
                    checked += 1;
                }
                FieldMeaning::OneOf(values) => {
                    for value in values {
                        #[allow(clippy::cast_precision_loss)]
                        let number = *value as f64;
                        assert!(
                            accepts(path, number),
                            "{path} refuses its own option {value}"
                        );
                    }
                    checked += 1;
                }
                _ => {}
            }
        }
        assert!(
            checked >= 18,
            "only {checked} environment bounds were declared"
        );
    }

    #[test]
    fn a_voxel_generator_names_its_materials_by_key() {
        let components = builtin_components().expect("the built-ins register");
        assert_eq!(
            components.meaning("sindri.voxel_world", "materials.2.voxel"),
            Some(&FieldMeaning::Key)
        );
        for layer in ["surface_voxel", "subsurface_voxel", "deep_voxel"] {
            assert_eq!(
                components.meaning("sindri.voxel_world", &format!("generator.{layer}")),
                Some(&FieldMeaning::KeyOf("materials[].voxel")),
                "{layer} should name a material rather than any number"
            );
        }
    }

    #[test]
    fn a_voxel_material_face_is_a_texture() {
        let components = builtin_components().expect("the built-ins register");
        assert_eq!(
            components.meaning("sindri.voxel_world", "materials.0.top"),
            Some(&FieldMeaning::Asset(sindri_core::AssetKind::Texture))
        );
    }
}
