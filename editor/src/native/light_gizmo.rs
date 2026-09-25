//! Lights as the Scene view draws them: a sun where the light stands, and an
//! arrow the way it shines.
//!
//! A directional light's position lights nothing, but it is where the author
//! looks for it, and its arrow is the answer to "where is the light coming
//! from", which a direction vector in the Environment never gave. Selected, it
//! also draws a long trail along the light, so aiming it with the rotate tool
//! shows which slopes it will reach.

use eframe::egui::{self, Color32, LayerId, Order, Painter, Pos2, Rect, Response, Stroke};
use glam::{Mat4, Vec3};
use sindri_core::{EntityId, SceneComponent};
use sindri_scene::{CameraView, LightComponent, light_direction};

use crate::selection::Selection;
use crate::ui::theme::color;

use super::EditorApp;
use super::projection::{distance_to_segment, project_point, project_segment};

const LIGHT_OVERLAY_LAYER: &str = "sindri-light-overlay";
const LIGHT_PICK_STATE: &str = "sindri-light-pick";

/// The disc's radius, in points, and how far its rays reach past it.
const SUN_RADIUS: f32 = 6.0;
const RAY_REACH: f32 = 5.0;

/// How long the arrow is in the world, and the trail a selected light draws.
const ARROW_LENGTH: f32 = 3.0;
const TRAIL_LENGTH: f32 = 40.0;

/// How near a click has to land to pick a light.
const PICK_DISTANCE: f32 = 8.0;

const SUN_YELLOW: Color32 = Color32::from_rgb(250, 204, 92);

/// One light as the Scene view draws it.
struct LightVisual {
    entity: EntityId,
    centre: Pos2,
    arrow: Option<[Pos2; 2]>,
    trail: Option<[Pos2; 2]>,
}

impl LightVisual {
    fn hit_test(&self, pointer: Pos2) -> bool {
        pointer.distance(self.centre) <= SUN_RADIUS + RAY_REACH
            || self
                .arrow
                .is_some_and(|[a, b]| distance_to_segment(pointer, a, b) <= PICK_DISTANCE)
    }
}

fn light_visual(
    entity: EntityId,
    position: Vec3,
    direction: Vec3,
    rect: Rect,
    view_projection: Mat4,
    selected: bool,
) -> Option<LightVisual> {
    let centre = project_point(rect, view_projection, position)?;
    let arrow = project_segment(
        rect,
        view_projection,
        position,
        position + direction * ARROW_LENGTH,
    );
    let trail = selected
        .then(|| {
            project_segment(
                rect,
                view_projection,
                position + direction * ARROW_LENGTH,
                position + direction * TRAIL_LENGTH,
            )
        })
        .flatten();
    Some(LightVisual {
        entity,
        centre,
        arrow,
        trail,
    })
}

fn paint_lights(painter: &Painter, visuals: &[LightVisual], selection: &Selection) {
    for visual in visuals {
        let selected = selection.contains(visual.entity);
        let tone = if selected {
            color::FORGE_BRIGHT
        } else {
            SUN_YELLOW
        };
        let stroke = Stroke::new(if selected { 2.0 } else { 1.5 }, tone);
        painter.circle(visual.centre, SUN_RADIUS, tone.gamma_multiply(0.35), stroke);
        for step in 0..8 {
            let angle = std::f32::consts::FRAC_PI_4 * f32::from(u8::try_from(step).unwrap_or(0));
            let along = egui::vec2(angle.cos(), angle.sin());
            painter.line_segment(
                [
                    visual.centre + along * (SUN_RADIUS + 2.0),
                    visual.centre + along * (SUN_RADIUS + RAY_REACH),
                ],
                Stroke::new(1.25, tone),
            );
        }
        if let Some([from, to]) = visual.arrow {
            painter.line_segment([from, to], stroke);
            paint_head(painter, from, to, stroke);
        }
        if let Some([from, to]) = visual.trail {
            painter.add(egui::Shape::dashed_line(
                &[from, to],
                Stroke::new(1.0, tone.gamma_multiply(0.7)),
                6.0,
                4.0,
            ));
        }
    }
}

/// Two short strokes at the end of the arrow, so a light pointing straight at
/// the viewer, whose arrow is a few points long, still says which end is which.
fn paint_head(painter: &Painter, from: Pos2, to: Pos2, stroke: Stroke) {
    let along = to - from;
    let length = along.length();
    if length < 1.0 {
        return;
    }
    let back = -along / length * 7.0;
    let side = egui::vec2(-back.y, back.x) * 0.5;
    painter.line_segment([to, to + back + side], stroke);
    painter.line_segment([to, to + back - side], stroke);
}

impl EditorApp {
    fn light_visuals(&self, rect: Rect, camera: CameraView) -> Vec<LightVisual> {
        let aspect = rect.width() / rect.height().max(1.0);
        let Some(scene_camera) = self
            .scene
            .world_camera_for_viewport(&self.world, aspect, camera)
            .ok()
            .flatten()
        else {
            return Vec::new();
        };
        self.world
            .entities()
            .filter(|(_, data)| data.components.contains_key(LightComponent::TYPE_NAME))
            .filter_map(|(entity, _)| {
                let transform = self.world.world_transform(entity).unwrap_or_default();
                light_visual(
                    entity,
                    Vec3::from_array(transform.position),
                    light_direction(transform),
                    rect,
                    scene_camera.view_projection,
                    self.selection.contains(entity),
                )
            })
            .collect()
    }

    /// Draws every light and lets a click on one select it.
    ///
    /// The click is carried to the next frame, as an authored camera's is, so
    /// it lands after the world's own picking and wins over whatever is behind
    /// the light rather than racing it.
    pub(super) fn light_overlay(
        &mut self,
        context: &egui::Context,
        response: &Response,
        tool_owns_primary: bool,
    ) {
        let pick_state = egui::Id::new(LIGHT_PICK_STATE);
        if let Some(Some(entity)) =
            context.data_mut(|data| data.remove_temp::<Option<EntityId>>(pick_state))
        {
            self.select(Some(entity));
        }
        let visuals = self.light_visuals(response.rect, self.scene_camera());
        if !tool_owns_primary
            && response.clicked_by(egui::PointerButton::Primary)
            && let Some(pointer) = response.interact_pointer_pos()
            && let Some(visual) = visuals.iter().rev().find(|visual| visual.hit_test(pointer))
        {
            let entity = visual.entity;
            context.data_mut(|data| data.insert_temp(pick_state, Some(entity)));
        }
        let painter = context
            // Background, so the floating panels stay over it; a layer of
            // its own, which egui paints after the Scene view's.
            .layer_painter(LayerId::new(
                Order::Background,
                egui::Id::new(LIGHT_OVERLAY_LAYER),
            ))
            .with_clip_rect(response.rect);
        paint_lights(&painter, &visuals, &self.selection);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn looking_down_z() -> Mat4 {
        let view = sindri_render::look_at(Vec3::new(0.0, 0.0, 10.0), Vec3::ZERO, Vec3::Y);
        sindri_render::perspective_projection(1.0, 1.0, 0.1, 100.0) * view
    }

    fn entity() -> EntityId {
        sindri_core::World::default().spawn(sindri_core::EntityData::default())
    }

    fn rect() -> Rect {
        Rect::from_min_size(Pos2::ZERO, egui::vec2(400.0, 400.0))
    }

    #[test]
    fn a_light_is_drawn_where_it_stands_with_its_arrow_the_way_it_shines() {
        let visual = light_visual(
            entity(),
            Vec3::ZERO,
            Vec3::NEG_Y,
            rect(),
            looking_down_z(),
            false,
        )
        .expect("the light is in front of the view");
        assert!(visual.centre.distance(rect().center()) < 0.5);
        let [from, to] = visual.arrow.expect("the arrow is on screen");
        assert!(
            to.y > from.y,
            "a light shining down draws its arrow down the screen"
        );
        assert!(
            visual.trail.is_none(),
            "only a selected light draws its trail"
        );
    }

    #[test]
    fn a_click_on_the_sun_or_its_arrow_picks_it() {
        let visual = light_visual(
            entity(),
            Vec3::ZERO,
            Vec3::X,
            rect(),
            looking_down_z(),
            true,
        )
        .unwrap();
        assert!(visual.hit_test(visual.centre + egui::vec2(3.0, 3.0)));
        let [_, tip] = visual.arrow.unwrap();
        assert!(visual.hit_test(tip));
        assert!(!visual.hit_test(visual.centre + egui::vec2(0.0, 60.0)));
        assert!(visual.trail.is_some());
    }
}
