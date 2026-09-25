//! Lights as objects in the scene.
//!
//! The environment used to hold the sun as a direction vector, and a direction
//! vector is the one thing a person cannot see: `[-0.55, 1.01, -0.4]` reads
//! as "somewhere up and to the left", and is in fact light travelling
//! *upwards*, which put the shadows on the tops of hills. A light is now an
//! entity, aimed the way a camera is, by its rotation: it shines along its own
//! forward axis, `-Z`, and the Scene view draws it where it is with an arrow
//! showing where the light goes.

use glam::Vec3;
use serde::{Deserialize, Serialize};
use sindri_core::{EntityId, SceneComponent, World};

use crate::extract::safe_rotation;

/// What kind of light an entity is.
///
/// Only the sun for now. Point and spot lights will be further kinds of this
/// one component rather than components of their own, so an entity switches
/// between them by a choice, as a camera switches projection.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LightKind {
    /// Parallel light from infinitely far away, like the sun. Only its
    /// rotation matters; where it stands is only where the Scene view draws
    /// it.
    #[default]
    Directional,
}

impl LightKind {
    /// Every spelling, in the order a picker offers them.
    pub const NAMES: [&'static str; 1] = ["directional"];
}

/// A light, shining along its entity's forward axis.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct LightComponent {
    pub kind: LightKind,
    pub color: [f32; 3],
    pub intensity: f32,
}

impl SceneComponent for LightComponent {
    const TYPE_NAME: &'static str = "sindri.light";
}

impl Default for LightComponent {
    fn default() -> Self {
        Self {
            kind: LightKind::Directional,
            color: [1.0, 0.95, 0.86],
            intensity: 1.0,
        }
    }
}

/// Why a light was refused.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum LightError {
    #[error("the light payload does not match the authored schema")]
    InvalidPayload,
    #[error("light `{field}` {expected}")]
    Field {
        field: &'static str,
        expected: &'static str,
    },
}

impl LightComponent {
    /// Refuses values the renderer cannot use, naming the field.
    pub fn validate(self) -> Result<Self, LightError> {
        if !self.color.iter().all(|value| value.is_finite()) {
            return Err(LightError::Field {
                field: "color",
                expected: "must be a finite number",
            });
        }
        if !self.intensity.is_finite() || self.intensity < 0.0 {
            return Err(LightError::Field {
                field: "intensity",
                expected: "must be a finite number of at least 0",
            });
        }
        Ok(self)
    }
}

/// The light a frame is lit by, resolved from the entity carrying it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sun {
    pub entity: EntityId,
    /// The way the light travels, unit length: from the sun towards the
    /// world.
    pub direction: [f32; 3],
    pub color: [f32; 3],
    pub intensity: f32,
}

/// The way a light with this rotation shines: its forward axis, `-Z`, the
/// same axis a camera looks along.
#[must_use]
pub fn light_direction(transform: sindri_core::Transform3D) -> Vec3 {
    (safe_rotation(transform) * Vec3::NEG_Z).normalize_or(Vec3::NEG_Y)
}

/// Where a new sun stands and how it is aimed: above the origin, pitched 50°
/// below the horizon and turned 30°, so light rakes across the world from one
/// side instead of arriving from straight overhead, where every slope is lit
/// the same and relief reads as flat.
#[must_use]
pub fn default_sun_transform() -> sindri_core::Transform3D {
    let rotation = glam::Quat::from_euler(
        glam::EulerRot::YXZ,
        30.0_f32.to_radians(),
        -50.0_f32.to_radians(),
        0.0,
    );
    sindri_core::Transform3D {
        position: [0.0, 10.0, 0.0],
        rotation: rotation.to_array(),
        ..sindri_core::Transform3D::default()
    }
}

/// Every entity carrying a light, active or not, each with its own verdict.
pub fn lights_in(world: &World) -> Vec<(EntityId, Result<LightComponent, LightError>)> {
    world
        .entities()
        .filter_map(|(entity, data)| {
            let payload = data.components.get(LightComponent::TYPE_NAME)?;
            let light = serde_json::from_value::<LightComponent>(payload.clone())
                .map_err(|_| LightError::InvalidPayload)
                .and_then(LightComponent::validate);
            Some((entity, light))
        })
        .collect()
}

/// The sun a world is lit by: the first active directional light.
///
/// One sun, as in most engines: a second is not added to the first, because
/// two parallel lights cast two sets of shadows and the renderer draws one.
/// An entity switched off, or under one that is, does not light anything.
#[must_use]
pub fn sun_in(world: &World, light: LightComponent, entity: EntityId) -> Option<Sun> {
    if !world.is_active(entity) {
        return None;
    }
    let LightKind::Directional = light.kind;
    world.get(entity)?;
    let transform = world.world_transform(entity).unwrap_or_default();
    Some(Sun {
        entity,
        direction: light_direction(transform).to_array(),
        color: light.color,
        intensity: light.intensity,
    })
}

#[cfg(test)]
mod tests {
    use sindri_core::Transform3D;

    use super::*;

    fn aimed(rotation: glam::Quat) -> Vec3 {
        light_direction(Transform3D {
            rotation: rotation.to_array(),
            ..Transform3D::default()
        })
    }

    #[test]
    fn an_unrotated_light_shines_forward_like_a_camera_looks() {
        assert!(aimed(glam::Quat::IDENTITY).abs_diff_eq(Vec3::NEG_Z, 1.0e-6));
    }

    #[test]
    fn a_light_pitched_down_shines_down() {
        let down = aimed(glam::Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2));
        assert!(down.abs_diff_eq(Vec3::NEG_Y, 1.0e-6), "{down}");
    }

    #[test]
    fn a_new_sun_shines_down_and_across() {
        let direction = light_direction(default_sun_transform());
        assert!(direction.y < -0.7, "{direction}");
        assert!(direction.x.hypot(direction.z) > 0.5, "{direction}");
    }

    #[test]
    fn a_light_names_the_field_it_cannot_use() {
        let light = LightComponent {
            intensity: -1.0,
            ..LightComponent::default()
        };
        assert_eq!(
            light.validate().unwrap_err().to_string(),
            "light `intensity` must be a finite number of at least 0"
        );
    }

    #[test]
    fn every_kind_name_is_a_kind() {
        for name in LightKind::NAMES {
            serde_json::from_value::<LightKind>(serde_json::json!(name)).expect(name);
        }
    }
}
