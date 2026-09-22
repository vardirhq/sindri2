//! `sindri.camera`: how a scene's own camera sees the world.

use serde::{Deserialize, Serialize};
use sindri_core::{EntityId, SceneComponent, Transform3D, World};

/// A camera authored into a scene.
///
/// Every authored camera renders the world. The projection tag chooses which
/// fields apply, so a scene cannot describe a perspective camera with an
/// orthographic size. Position and orientation come from the entity's
/// `Transform3D`: local -Z is forward and local +Y is up.
///
/// Screen-space sprites and text are viewport-owned and do not require a camera
/// entity.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(tag = "projection", rename_all = "snake_case")]
pub enum CameraComponent {
    Perspective {
        vertical_fov_degrees: f32,
        near: f32,
        far: f32,
    },
    Orthographic {
        vertical_size: f32,
        near: f32,
        far: f32,
        /// Which axis `vertical_size` measures.
        ///
        /// Defaulted, so every scene written before this keeps framing exactly
        /// what it framed.
        #[serde(default)]
        fit: CameraFit,
    },
}

/// Which way round a camera frames what it was told to frame.
///
/// An orthographic camera says how much world it shows and the other axis
/// follows the aspect ratio. Which axis is told is the whole question on a
/// phone: a game framed by height shows a fixed amount vertically and whatever
/// the width happens to be, so turning a wide window into a tall one takes the
/// sides off the world — an arena that filled a desktop is cropped down its
/// middle on a portrait screen, and the player is shooting at things nobody
/// can see.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum CameraFit {
    /// Frame the size vertically, and let the width follow the aspect.
    ///
    /// What every camera did before there was a choice, and the right answer
    /// for a game that will only ever be wide.
    #[default]
    Height,
    /// Frame the size on whichever axis is shorter.
    ///
    /// The size becomes a promise rather than a measurement: *this much world
    /// is visible whichever way the screen is turned*. A square arena framed
    /// this way fills the height of a landscape window and the width of a
    /// portrait one, and is never cut off by either.
    Shorter,
}

impl CameraComponent {
    /// The projections a camera may have, by the names a scene stores.
    ///
    /// Named here because the tag decides which other fields the component
    /// has: an editor that offers this choice has to write the fields the
    /// chosen projection needs, and it should not be inventing its own idea of
    /// what those are.
    pub const PROJECTIONS: [&'static str; 2] = ["perspective", "orthographic"];

    /// What a perspective camera frames when nothing has said otherwise.
    ///
    /// A quarter turn of vertical view: wide enough to see a scene, narrow
    /// enough not to distort it.
    pub const DEFAULT_VERTICAL_FOV_DEGREES: f32 = 60.0;

    /// How much world an orthographic camera frames vertically by default.
    ///
    /// Six units rather than the renderer's own two, because two is the screen
    /// overlay's extent and a world camera framing two units of world puts an
    /// author inside whatever they were looking at.
    pub const DEFAULT_VERTICAL_SIZE: f32 = 6.0;

    /// The near and far planes a camera starts with.
    pub const DEFAULT_NEAR: f32 = 0.1;
    pub const DEFAULT_FAR: f32 = 100.0;

    /// The name of the projection this camera has.
    #[must_use]
    pub const fn projection_name(&self) -> &'static str {
        match self {
            Self::Perspective { .. } => Self::PROJECTIONS[0],
            Self::Orthographic { .. } => Self::PROJECTIONS[1],
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub struct CameraBehaviorComponent {
    #[serde(default)]
    pub follow: Option<CameraFollow>,
    #[serde(default)]
    pub confine: Option<CameraBounds>,
    #[serde(default)]
    pub shake: CameraShake,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub struct CameraFollow {
    pub target: EntityId,
    #[serde(default)]
    pub offset: [f32; 3],
    #[serde(default)]
    pub dead_zone: [f32; 2],
    #[serde(default = "default_follow_smoothing")]
    pub smoothing: f32,
    #[serde(default)]
    pub max_speed: f32,
}

const fn default_follow_smoothing() -> f32 {
    8.0
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub struct CameraBounds {
    pub min: [f32; 2],
    pub max: [f32; 2],
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub struct CameraShake {
    #[serde(default)]
    pub trauma: f32,
    #[serde(default = "default_shake_strength")]
    pub strength: f32,
    #[serde(default = "default_shake_decay")]
    pub decay: f32,
    #[serde(default = "default_shake_frequency")]
    pub frequency: f32,
    #[serde(default)]
    pub phase: f32,
}

const fn default_shake_strength() -> f32 {
    0.12
}
const fn default_shake_decay() -> f32 {
    2.8
}
const fn default_shake_frequency() -> f32 {
    57.0
}

impl Default for CameraShake {
    fn default() -> Self {
        Self {
            trauma: 0.0,
            strength: default_shake_strength(),
            decay: default_shake_decay(),
            frequency: default_shake_frequency(),
            phase: 0.0,
        }
    }
}

impl SceneComponent for CameraBehaviorComponent {
    const TYPE_NAME: &'static str = "sindri.camera.behavior";
}

pub fn update_camera_behaviors(world: &mut World, dt: f32) {
    if !dt.is_finite() || dt <= 0.0 {
        return;
    }
    let cameras: Vec<_> = world
        .entities()
        .filter_map(|(entity, data)| {
            let behavior = data.components.get(CameraBehaviorComponent::TYPE_NAME)?;
            serde_json::from_value::<CameraBehaviorComponent>(behavior.clone())
                .ok()
                .map(|behavior| (entity, behavior))
        })
        .collect();

    for (entity, mut behavior) in cameras {
        update_camera_behavior(world, entity, &mut behavior, dt);
    }
}

fn update_camera_behavior(
    world: &mut World,
    entity: EntityId,
    behavior: &mut CameraBehaviorComponent,
    dt: f32,
) {
    let Some(current) = world.get(entity).and_then(|data| data.transform_3d) else {
        return;
    };
    let previous_shake = shake_offset(&behavior.shake);
    let mut position = current.position;
    position[0] -= previous_shake[0];
    position[1] -= previous_shake[1];
    apply_follow(world, behavior.follow, &mut position, dt);
    apply_confine(behavior.confine, &mut position);
    advance_shake(&mut behavior.shake, dt);
    let shake = shake_offset(&behavior.shake);
    position[0] += shake[0];
    position[1] += shake[1];

    if let Some(data) = world.get_mut(entity) {
        data.transform_3d = Some(Transform3D {
            position,
            ..current
        });
        if let Ok(value) = serde_json::to_value(*behavior) {
            data.components
                .insert(CameraBehaviorComponent::TYPE_NAME.into(), value);
        }
    }
}

fn apply_follow(world: &World, follow: Option<CameraFollow>, position: &mut [f32; 3], dt: f32) {
    let Some(follow) = follow else {
        return;
    };
    let Some(target) = world.get(follow.target).and_then(|data| data.transform_3d) else {
        return;
    };
    let desired = [
        target.position[0] + follow.offset[0],
        target.position[1] + follow.offset[1],
        target.position[2] + follow.offset[2],
    ];
    for axis in 0..2 {
        let delta = desired[axis] - position[axis];
        let dead = follow.dead_zone[axis].max(0.0) * 0.5;
        if delta.abs() <= dead {
            continue;
        }
        let outside = delta - delta.signum() * dead;
        let alpha = 1.0 - (-follow.smoothing.max(0.0) * dt).exp();
        let mut step = outside * alpha;
        if follow.max_speed > 0.0 {
            step = step.clamp(-follow.max_speed * dt, follow.max_speed * dt);
        }
        position[axis] += step;
    }
}

fn apply_confine(bounds: Option<CameraBounds>, position: &mut [f32; 3]) {
    let Some(bounds) = bounds else {
        return;
    };
    position[0] = position[0].clamp(bounds.min[0], bounds.max[0]);
    position[1] = position[1].clamp(bounds.min[1], bounds.max[1]);
}

fn advance_shake(shake: &mut CameraShake, dt: f32) {
    shake.phase += dt * shake.frequency.max(0.0);
    let trauma = shake.trauma.clamp(0.0, 1.0);
    shake.trauma = (trauma - shake.decay.max(0.0) * dt).max(0.0);
}

fn shake_offset(shake: &CameraShake) -> [f32; 2] {
    let trauma = shake.trauma.clamp(0.0, 1.0);
    let amplitude = trauma * trauma * shake.strength.max(0.0);
    [
        shake.phase.sin() * amplitude,
        (shake.phase * 1.37).cos() * amplitude,
    ]
}

impl SceneComponent for CameraComponent {
    const TYPE_NAME: &'static str = "sindri.camera";
}

#[cfg(test)]
mod behavior_tests {
    use super::*;
    use serde_json::json;
    use sindri_core::EntityData;

    fn entity(position: [f32; 3]) -> EntityData {
        EntityData {
            transform_3d: Some(Transform3D {
                position,
                ..Transform3D::default()
            }),
            ..EntityData::default()
        }
    }

    #[test]
    fn follow_respects_dead_zone_and_bounds() {
        let mut world = World::default();
        let target = world.spawn(entity([8.0, 0.0, 0.0]));
        let mut camera = entity([0.0, 0.0, 5.0]);
        camera.components.insert(CameraBehaviorComponent::TYPE_NAME.into(), json!({
            "follow": { "target": target, "dead_zone": [2.0, 2.0], "smoothing": 1000.0, "max_speed": 0.0 },
            "confine": { "min": [-4.0, -3.0], "max": [4.0, 3.0] },
            "shake": {}
        }));
        let camera = world.spawn(camera);
        update_camera_behaviors(&mut world, 1.0);
        let x = world.get(camera).unwrap().transform_3d.unwrap().position[0];
        assert!((x - 4.0).abs() < f32::EPSILON);
    }

    #[test]
    fn shake_decays_and_is_deterministic() {
        let mut world = World::default();
        let mut camera = entity([0.0, 0.0, 5.0]);
        camera.components.insert(CameraBehaviorComponent::TYPE_NAME.into(), json!({
            "shake": { "trauma": 1.0, "strength": 1.0, "decay": 1.0, "frequency": 1.0, "phase": 0.0 }
        }));
        let camera = world.spawn(camera);
        update_camera_behaviors(&mut world, 0.25);
        let data = world.get(camera).unwrap();
        assert!(data.transform_3d.unwrap().position[0].abs() > f32::EPSILON);
        let behavior: CameraBehaviorComponent =
            serde_json::from_value(data.components[CameraBehaviorComponent::TYPE_NAME].clone())
                .unwrap();
        assert!((behavior.shake.trauma - 0.75).abs() < f32::EPSILON);
    }
}
