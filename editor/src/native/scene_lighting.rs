//! The Scene view's own light, for working in a scene that is dark on purpose.
//!
//! A night level, a cave, a sun turned down to nothing: the Game view should
//! show them as dark as they are, and the Scene view is where someone has to
//! see what they are doing in them. Unity's answer is a toggle on the Scene
//! view: with the scene's lighting off, the view is lit by an even ambient and
//! a light that comes from over the camera's shoulder, so whatever it looks at
//! is lit. The scene's own lights, ambient and shadows are left out; its fog,
//! sky and post-processing are not, because they are how the scene looks
//! rather than how it is lit.

use glam::Vec3;
use sindri_render::{ShadowSettings, WorldLighting};
use sindri_scene::{CameraView, EnvironmentComponent};

use super::scene_io::SceneSource;

/// How bright the editor's even fill and its headlight are. Together they
/// light a face turned toward the camera fully and one turned away from it at
/// the fill alone, so shape still reads.
const FILL: f32 = 0.45;
const HEADLIGHT: f32 = 0.7;

/// The lighting and shadows a view draws with this frame.
pub(super) fn lighting_for(
    source: SceneSource<'_>,
    camera: CameraView,
    aspect: f32,
    environment: Option<EnvironmentComponent>,
) -> Result<(WorldLighting, ShadowSettings), String> {
    if source.studio_lighting {
        let forward = source
            .scene
            .world_camera_for_viewport(source.world, aspect, camera)
            .ok()
            .flatten()
            .map_or(Vec3::NEG_Z, |camera| {
                camera.view.inverse().transform_vector3(Vec3::NEG_Z)
            });
        return Ok((studio_lighting(forward), ShadowSettings::default()));
    }
    let lighting = source
        .scene
        .lighting(source.world)
        .map_err(|error| error.to_string())?;
    let shadows = environment
        .map(EnvironmentComponent::shadow_settings)
        .unwrap_or_default();
    Ok((lighting, shadows))
}

/// An even fill and a light travelling the way the camera looks, tipped down
/// a little so tops read brighter than sides, as they would outdoors.
fn studio_lighting(forward: Vec3) -> WorldLighting {
    let direction =
        (forward.normalize_or(Vec3::NEG_Z) + Vec3::new(0.0, -0.6, 0.0)).normalize_or(Vec3::NEG_Y);
    WorldLighting {
        ambient_color: [1.0; 3],
        ambient_intensity: FILL,
        directional_direction: direction.to_array(),
        directional_color: [1.0; 3],
        directional_intensity: HEADLIGHT,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_headlight_shines_the_way_the_camera_looks_and_a_little_down() {
        let lighting = studio_lighting(Vec3::NEG_Z);
        let [x, y, z] = lighting.directional_direction;
        assert!(z < -0.5 && y < 0.0 && x.abs() < 1.0e-6, "{x} {y} {z}");
        assert!(
            lighting.ambient_intensity > 0.0,
            "a face turned away still reads"
        );
    }
}
