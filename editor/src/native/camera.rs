//! How the editor looks at a scene, as opposed to what the scene says.
//!
//! Scene navigation belongs to the editor. Authored cameras remain ordinary
//! scene entities; this module only reconstructs editor-only markers and
//! projection volumes for the Scene view.

use std::f32::consts::TAU;

use eframe::egui::{self, Color32, LayerId, Order, Painter, Pos2, Rect, Response, Shape, Stroke};
use glam::{Mat4, Quat, Vec2 as GlamVec2, Vec3};
use sindri_core::{EntityId, SceneComponent, Transform3D};
use sindri_scene::{CameraComponent, CameraView, ViewCamera, WorldProjection};

use crate::preferences::CameraProjection;

use super::{ACCENT_BRIGHT, EditorApp, TEXT_MUTED, WorkspaceTab};

#[cfg(test)]
mod tests;

const CAMERA_PICK_STATE: &str = "sindri-authored-camera-pick";
const CAMERA_OVERLAY_LAYER: &str = "sindri-authored-camera-overlay";

#[derive(Clone, Copy)]
pub(super) struct EditorCamera {
    orbit: GlamVec2,
    zoom: f32,
    pan: GlamVec2,
    projection: CameraProjection,
}

impl Default for EditorCamera {
    fn default() -> Self {
        Self {
            orbit: GlamVec2::ZERO,
            zoom: 1.0,
            pan: GlamVec2::ZERO,
            projection: CameraProjection::Perspective,
        }
    }
}

#[derive(Clone, Debug)]
struct AuthoredCameraVisual {
    entity: EntityId,
    body: Vec<Pos2>,
    lines: Vec<[Pos2; 2]>,
    hit_lines: Vec<[Pos2; 2]>,
}

impl AuthoredCameraVisual {
    fn hit_test(&self, pointer: Pos2) -> bool {
        polygon_contains(&self.body, pointer)
            || self
                .hit_lines
                .iter()
                .any(|line| distance_to_segment(pointer, line[0], line[1]) <= 6.0)
    }
}

pub(super) fn pan_to_centre(camera: ViewCamera, pan: GlamVec2, position: Vec3) -> GlamVec2 {
    if camera.framed_half_height <= 0.0 {
        return pan;
    }
    let offset = camera.view.transform_point3(position);
    pan - GlamVec2::new(offset.x, offset.y) / camera.framed_half_height
}

pub(super) const PITCH_LIMIT: f32 = 1.5;
pub(super) const MIN_ZOOM: f32 = 0.05;
pub(super) const MAX_ZOOM: f32 = 20.0;

pub(super) fn camera_for(tab: WorkspaceTab, editor: EditorCamera) -> CameraView {
    match tab {
        WorkspaceTab::Scene => CameraView {
            orbit: editor.orbit,
            distance_scale: 1.0 / editor.zoom,
            pan: editor.pan,
            projection: match editor.projection {
                CameraProjection::Perspective => WorldProjection::Perspective,
                CameraProjection::Orthographic => WorldProjection::Orthographic,
            },
        },
        WorkspaceTab::Game => CameraView::default(),
    }
}

fn safe_rotation(transform: Transform3D) -> Quat {
    let rotation = Quat::from_array(transform.rotation);
    if rotation.is_finite() && rotation.length_squared() > f32::EPSILON {
        rotation.normalize()
    } else {
        Quat::IDENTITY
    }
}

fn perspective_corners(
    vertical_fov_degrees: f32,
    near: f32,
    far: f32,
    aspect: f32,
) -> [[Vec3; 4]; 2] {
    let tan = (vertical_fov_degrees.to_radians() * 0.5).tan();
    let plane = |distance: f32| {
        let half_height = distance * tan;
        let half_width = half_height * aspect;
        [
            Vec3::new(-half_width, -half_height, -distance),
            Vec3::new(half_width, -half_height, -distance),
            Vec3::new(half_width, half_height, -distance),
            Vec3::new(-half_width, half_height, -distance),
        ]
    };
    [plane(near), plane(far)]
}

fn orthographic_corners(vertical_size: f32, near: f32, far: f32, aspect: f32) -> [[Vec3; 4]; 2] {
    let half_height = vertical_size * 0.5;
    let half_width = half_height * aspect;
    let plane = |distance: f32| {
        [
            Vec3::new(-half_width, -half_height, -distance),
            Vec3::new(half_width, -half_height, -distance),
            Vec3::new(half_width, half_height, -distance),
            Vec3::new(-half_width, half_height, -distance),
        ]
    };
    [plane(near), plane(far)]
}

fn project_point(rect: Rect, view_projection: Mat4, point: Vec3) -> Option<Pos2> {
    let clip = view_projection * point.extend(1.0);
    if !clip.is_finite() || clip.w.abs() <= f32::EPSILON {
        return None;
    }
    let ndc = clip.truncate() / clip.w;
    if !ndc.is_finite() {
        return None;
    }
    Some(Pos2::new(
        rect.min.x + (ndc.x + 1.0) * 0.5 * rect.width(),
        rect.min.y + (1.0 - (ndc.y + 1.0) * 0.5) * rect.height(),
    ))
}

fn projected_plane(
    rect: Rect,
    view_projection: Mat4,
    model: Mat4,
    plane: [Vec3; 4],
) -> Option<[Pos2; 4]> {
    Some([
        project_point(rect, view_projection, model.transform_point3(plane[0]))?,
        project_point(rect, view_projection, model.transform_point3(plane[1]))?,
        project_point(rect, view_projection, model.transform_point3(plane[2]))?,
        project_point(rect, view_projection, model.transform_point3(plane[3]))?,
    ])
}

fn camera_visual(
    entity: EntityId,
    transform: Transform3D,
    camera: CameraComponent,
    rect: Rect,
    view_projection: Mat4,
    aspect: f32,
) -> Option<AuthoredCameraVisual> {
    let rotation = safe_rotation(transform);
    let position = Vec3::from_array(transform.position);
    let model = Mat4::from_rotation_translation(rotation, position);
    let forward = rotation * -Vec3::Z;
    let corners = match camera {
        CameraComponent::Perspective {
            vertical_fov_degrees,
            near,
            far,
        } => perspective_corners(vertical_fov_degrees, near, far, aspect),
        CameraComponent::Orthographic {
            vertical_size,
            near,
            far,
        } => orthographic_corners(vertical_size, near, far, aspect),
    };

    let centre = project_point(rect, view_projection, position)?;
    let forward_tip = project_point(rect, view_projection, position + forward * 0.8)?;
    let body_radius = 7.0;
    let body = vec![
        Pos2::new(centre.x - body_radius, centre.y - body_radius * 0.65),
        Pos2::new(centre.x + body_radius * 0.45, centre.y - body_radius * 0.65),
        Pos2::new(centre.x + body_radius, centre.y),
        Pos2::new(centre.x + body_radius * 0.45, centre.y + body_radius * 0.65),
        Pos2::new(centre.x - body_radius, centre.y + body_radius * 0.65),
    ];

    let near = projected_plane(rect, view_projection, model, corners[0])?;
    let far = projected_plane(rect, view_projection, model, corners[1])?;
    let mut lines = Vec::with_capacity(13);
    for index in 0..4 {
        lines.push([near[index], near[(index + 1) % 4]]);
        lines.push([far[index], far[(index + 1) % 4]]);
        lines.push([near[index], far[index]]);
    }
    lines.push([centre, forward_tip]);

    Some(AuthoredCameraVisual {
        entity,
        body,
        hit_lines: lines.clone(),
        lines,
    })
}

fn distance_to_segment(point: Pos2, a: Pos2, b: Pos2) -> f32 {
    let ab = b - a;
    let length_squared = ab.length_sq();
    if length_squared <= f32::EPSILON {
        return point.distance(a);
    }
    let t = ((point - a).dot(ab) / length_squared).clamp(0.0, 1.0);
    point.distance(a + ab * t)
}

fn polygon_contains(points: &[Pos2], point: Pos2) -> bool {
    if points.len() < 3 {
        return false;
    }
    let mut inside = false;
    let mut previous = points.len() - 1;
    for current in 0..points.len() {
        let a = points[current];
        let b = points[previous];
        if ((a.y > point.y) != (b.y > point.y))
            && point.x < (b.x - a.x) * (point.y - a.y) / (b.y - a.y) + a.x
        {
            inside = !inside;
        }
        previous = current;
    }
    inside
}

fn pick_authored_camera(visuals: &[AuthoredCameraVisual], pointer: Pos2) -> Option<EntityId> {
    visuals
        .iter()
        .rev()
        .find(|visual| visual.hit_test(pointer))
        .map(|visual| visual.entity)
}

fn paint_authored_cameras(
    painter: &Painter,
    visuals: &[AuthoredCameraVisual],
    selection: Option<EntityId>,
) {
    for visual in visuals {
        let selected = selection == Some(visual.entity);
        let stroke = Stroke::new(
            if selected { 2.0 } else { 1.25 },
            if selected { ACCENT_BRIGHT } else { TEXT_MUTED },
        );
        for line in &visual.lines {
            painter.line_segment(*line, stroke);
        }
        painter.add(Shape::convex_polygon(
            visual.body.clone(),
            if selected {
                Color32::from_rgba_unmultiplied(246, 169, 35, 48)
            } else {
                Color32::from_rgba_unmultiplied(170, 177, 190, 28)
            },
            stroke,
        ));
    }
}

impl EditorApp {
    fn authored_camera_visuals(&self, rect: Rect, camera: CameraView) -> Vec<AuthoredCameraVisual> {
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
            .filter_map(|(entity, data)| {
                let payload = data.components.get(CameraComponent::TYPE_NAME)?;
                let camera = serde_json::from_value::<CameraComponent>(payload.clone()).ok()?;
                let transform = data.transform_3d.unwrap_or_default();
                camera_visual(
                    entity,
                    transform,
                    camera,
                    rect,
                    scene_camera.view_projection,
                    aspect,
                )
            })
            .collect()
    }

    fn authored_camera_overlay(
        &mut self,
        context: &egui::Context,
        response: &Response,
        tool_owns_primary: bool,
    ) {
        let pick_state = egui::Id::new(CAMERA_PICK_STATE);
        if let Some(Some(entity)) =
            context.data_mut(|data| data.remove_temp::<Option<EntityId>>(pick_state))
        {
            self.select(Some(entity));
        }

        let camera = self.scene_camera();
        let visuals = self.authored_camera_visuals(response.rect, camera);
        if !tool_owns_primary
            && response.clicked_by(egui::PointerButton::Primary)
            && let Some(pointer) = response.interact_pointer_pos()
            && let Some(entity) = pick_authored_camera(&visuals, pointer)
        {
            context.data_mut(|data| data.insert_temp(pick_state, Some(entity)));
        }

        let painter = context
            .layer_painter(LayerId::new(
                Order::Foreground,
                egui::Id::new(CAMERA_OVERLAY_LAYER),
            ))
            .with_clip_rect(response.rect);
        paint_authored_cameras(&painter, &visuals, self.selection);
    }

    pub(super) fn move_camera(
        &mut self,
        context: &egui::Context,
        response: &Response,
        height: f32,
        painting: bool,
    ) {
        if response.dragged() {
            let delta = response.drag_motion();
            if response.dragged_by(egui::PointerButton::Middle)
                || context.input(|input| input.modifiers.shift)
            {
                let height = height.max(1.0);
                self.viewport_pan.x += delta.x * 2.0 / height;
                self.viewport_pan.y -= delta.y * 2.0 / height;
            } else if response.dragged_by(egui::PointerButton::Secondary)
                || (!painting && response.dragged_by(egui::PointerButton::Primary))
            {
                self.viewport_yaw = (self.viewport_yaw + delta.x * 0.008) % TAU;
                self.viewport_pitch =
                    (self.viewport_pitch + delta.y * 0.008).clamp(-PITCH_LIMIT, PITCH_LIMIT);
            }
        }
        if response.hovered() {
            let delta = context.input(|input| input.smooth_scroll_delta.y);
            self.viewport_zoom =
                (self.viewport_zoom * (delta * 0.002).exp()).clamp(MIN_ZOOM, MAX_ZOOM);
        }
        self.authored_camera_overlay(context, response, painting);
    }

    pub(super) fn focus_selection(&mut self) {
        let Some(position) = self
            .selection
            .and_then(|entity| self.world.get(entity))
            .and_then(|data| data.transform_3d)
            .map(|transform| Vec3::from_array(transform.position))
        else {
            return;
        };
        let Ok(Some(camera)) = self.scene.world_camera(&self.world, self.scene_camera()) else {
            return;
        };
        self.viewport_pan = pan_to_centre(camera, self.viewport_pan, position);
    }

    pub(super) fn scene_camera(&self) -> CameraView {
        camera_for(
            WorkspaceTab::Scene,
            EditorCamera {
                orbit: GlamVec2::new(self.viewport_yaw, self.viewport_pitch),
                zoom: self.viewport_zoom,
                pan: self.viewport_pan,
                projection: self.preferences.projection,
            },
        )
    }

    pub(super) fn view_moved(&self) -> bool {
        self.viewport_yaw != 0.0
            || self.viewport_pitch != 0.0
            || self.viewport_pan != GlamVec2::ZERO
            || (self.viewport_zoom - 1.0).abs() > f32::EPSILON
    }

    pub(super) fn reset_view(&mut self) {
        self.viewport_yaw = 0.0;
        self.viewport_pitch = 0.0;
        self.viewport_pan = GlamVec2::ZERO;
        self.viewport_zoom = 1.0;
    }
}
