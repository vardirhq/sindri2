//! How the editor looks at a scene, as opposed to what the scene says.
//!
//! Scene navigation belongs to the editor. Authored cameras remain ordinary
//! scene entities; this module only reconstructs editor-only markers and
//! projection volumes for the Scene view.

use std::f32::consts::TAU;

use eframe::egui::{self, Color32, LayerId, Order, Painter, Pos2, Rect, Response, Shape, Stroke};
use glam::{Mat4, Quat, Vec2 as GlamVec2, Vec3};
use sindri_core::{EntityId, SceneComponent, Transform3D};
use sindri_scene::{CameraComponent, CameraFit, CameraView, ViewCamera, WorldProjection};

use crate::preferences::CameraProjection;
use crate::selection::Selection;

use crate::ui::theme::color;

use super::projection::{distance_to_segment, polygon_contains, project_point, project_segment};
use super::{EditorApp, WorkspaceTab};

#[cfg(test)]
mod tests;

const CAMERA_OVERLAY_LAYER: &str = "sindri-authored-camera-overlay";
const CAMERA_PICK_STATE: &str = "sindri-authored-camera-pick";

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
                CameraProjection::Flat => WorldProjection::Flat,
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

fn orthographic_corners(
    vertical_size: f32,
    near: f32,
    far: f32,
    aspect: f32,
    fit: CameraFit,
) -> [[Vec3; 4]; 2] {
    // The same choice the extractor makes, because a gizmo that drew a frustum
    // the camera does not have would be worse than no gizmo: it is the picture
    // an author trusts when deciding what is on screen.
    let half = vertical_size * 0.5;
    let half_height = match fit {
        CameraFit::Shorter if aspect < 1.0 => half / aspect,
        CameraFit::Shorter | CameraFit::Height => half,
    };
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

/// The canvas the UI is laid out on, as an outline in the Scene view.
///
/// Without it the overlay is a set of things floating at the origin with no
/// visible edge, and "runs off the side of the screen" is not something a
/// picture can show. The rectangle is where the screen's edges are, so an
/// element outside it is outside the screen — which is the whole question a
/// person is asking when they arrange one.
pub(super) fn canvas_outline(rect: Rect, view_projection: Mat4, aspect: f32) -> Vec<[Pos2; 2]> {
    let half_height = 1.0;
    let half_width = half_height * aspect.max(f32::EPSILON);
    let corners = [
        Vec3::new(-half_width, -half_height, 0.0),
        Vec3::new(half_width, -half_height, 0.0),
        Vec3::new(half_width, half_height, 0.0),
        Vec3::new(-half_width, half_height, 0.0),
    ];
    let mut lines = Vec::with_capacity(4);
    for index in 0..4 {
        let next = (index + 1) % 4;
        lines.extend(project_segment(
            rect,
            view_projection,
            corners[index],
            corners[next],
        ));
    }
    lines
}

/// A rectangle lying on the canvas, as an outline in the Scene view.
///
/// What a text element's box is drawn as. Without it a wrap width is a number in
/// the inspector and the only way to find out where it falls is to retype it and
/// look — which is exactly the kind of authoring the editor audit was written to
/// remove.
pub(super) fn canvas_rect_outline(
    rect: Rect,
    view_projection: Mat4,
    centre: [f32; 2],
    size: [f32; 2],
) -> Vec<[Pos2; 2]> {
    let half = [size[0] * 0.5, size[1] * 0.5];
    let corners = [
        Vec3::new(centre[0] - half[0], centre[1] - half[1], 0.0),
        Vec3::new(centre[0] + half[0], centre[1] - half[1], 0.0),
        Vec3::new(centre[0] + half[0], centre[1] + half[1], 0.0),
        Vec3::new(centre[0] - half[0], centre[1] + half[1], 0.0),
    ];
    let mut lines = Vec::with_capacity(4);
    for index in 0..4 {
        lines.extend(project_segment(
            rect,
            view_projection,
            corners[index],
            corners[(index + 1) % 4],
        ));
    }
    lines
}

/// One authored camera as it is drawn in the Scene view.
///
/// `aspect` is the aspect the camera actually renders at — the Game view's —
/// and not the Scene view's. They used to be the same number, so resizing the
/// Scene view reshaped a frustum that had not changed: the picture said the
/// camera frames more of the world because the panel beside it got wider, which
/// is a lie about the scene.
///
/// The frustum is drawn for the selected camera only. An unselected one is its
/// marker and a short forward stub, because a frustum is a hundred units long
/// and five of them crossing the viewport say nothing about the camera anybody
/// is actually working on.
fn camera_visual(
    entity: EntityId,
    transform: Transform3D,
    camera: CameraComponent,
    rect: Rect,
    view_projection: Mat4,
    aspect: f32,
    selected: bool,
) -> Option<AuthoredCameraVisual> {
    let rotation = safe_rotation(transform);
    let position = Vec3::from_array(transform.position);
    let model = Mat4::from_rotation_translation(rotation, position);
    let forward = rotation * -Vec3::Z;

    let centre = project_point(rect, view_projection, position)?;
    let body_radius = 7.0;
    let body = vec![
        Pos2::new(centre.x - body_radius, centre.y - body_radius * 0.65),
        Pos2::new(centre.x + body_radius * 0.45, centre.y - body_radius * 0.65),
        Pos2::new(centre.x + body_radius, centre.y),
        Pos2::new(centre.x + body_radius * 0.45, centre.y + body_radius * 0.65),
        Pos2::new(centre.x - body_radius, centre.y + body_radius * 0.65),
    ];

    // The stub is both what says which way the camera faces and what a click
    // lands on besides the marker. Everything else is a picture rather than a
    // target: a far-plane edge a hundred units away is not what someone means
    // when they click on it.
    let mut lines = Vec::with_capacity(13);
    let hit_lines = project_segment(rect, view_projection, position, position + forward * 0.8)
        .map(|stub| vec![stub])
        .unwrap_or_default();
    lines.extend(hit_lines.iter().copied());

    if selected {
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
                fit,
            } => orthographic_corners(vertical_size, near, far, aspect, fit),
        };
        let world = |plane: [Vec3; 4]| plane.map(|corner| model.transform_point3(corner));
        let (near, far) = (world(corners[0]), world(corners[1]));
        // Each edge is clipped on its own, so an edge that leaves the view
        // shortens instead of taking the whole frustum with it.
        for index in 0..4 {
            let next = (index + 1) % 4;
            lines.extend(project_segment(
                rect,
                view_projection,
                near[index],
                near[next],
            ));
            lines.extend(project_segment(
                rect,
                view_projection,
                far[index],
                far[next],
            ));
            lines.extend(project_segment(
                rect,
                view_projection,
                near[index],
                far[index],
            ));
        }
    }

    Some(AuthoredCameraVisual {
        entity,
        body,
        lines,
        hit_lines,
    })
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
    selection: &Selection,
) {
    for visual in visuals {
        let selected = selection.contains(visual.entity);
        let stroke = Stroke::new(
            if selected { 2.0 } else { 1.25 },
            if selected {
                color::FORGE_BRIGHT
            } else {
                color::TEXT_MUTED
            },
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
        // Two aspects, deliberately. The Scene view's decides how the world is
        // projected onto this panel; the Game view's decides what an authored
        // camera frames, because that is the viewport it renders into.
        let scene_aspect = rect.width() / rect.height().max(1.0);
        let framed_aspect = self.game_viewport.aspect();
        let Some(scene_camera) = self
            .scene
            .world_camera_for_viewport(&self.world, scene_aspect, camera)
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
                    framed_aspect,
                    self.selection.contains(entity),
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
            // Background, so the floating panels stay over it; a layer of
            // its own, which egui paints after the Scene view's.
            .layer_painter(LayerId::new(
                Order::Background,
                egui::Id::new(CAMERA_OVERLAY_LAYER),
            ))
            .with_clip_rect(response.rect);
        paint_authored_cameras(&painter, &visuals, &self.selection);
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
            // A flat view has nothing to orbit: every drag that would turn
            // the view moves it instead, as a 2D editor's does.
            let flat = self.preferences.projection == CameraProjection::Flat;
            if response.dragged_by(egui::PointerButton::Middle)
                || context.input(|input| input.modifiers.shift)
                || (flat
                    && (response.dragged_by(egui::PointerButton::Secondary)
                        || (!painting && response.dragged_by(egui::PointerButton::Primary))))
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
        // The middle of everything selected rather than of the primary, so
        // framing a row of five pips puts the row on screen rather than
        // whichever one was clicked last.
        let placed: Vec<Vec3> = self
            .selection
            .all()
            .iter()
            .filter_map(|entity| self.world.get(*entity))
            .filter_map(|data| data.transform_3d)
            .map(|transform| Vec3::from_array(transform.position))
            .collect();
        let Some(position) = centre_of(&placed) else {
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

    /// Opens a 2D scene the way it is played: flat, and framed on what its
    /// camera frames.
    ///
    /// A scene is 2D when its camera is orthographic and looks straight down
    /// -Z, as every sprite game's does. A scene that is not leaves a flat view
    /// for the perspective one, because a flat view of a 3D world is a view of
    /// nothing in particular.
    pub(super) fn frame_scene_camera(&mut self) {
        let flat_camera = flat_camera(&self.world, self.scene.components());
        let Some((position, vertical_size)) = flat_camera else {
            if self.preferences.projection == CameraProjection::Flat {
                self.preferences.projection = CameraProjection::Perspective;
            }
            return;
        };
        self.preferences.projection = CameraProjection::Flat;
        self.reset_view();
        self.viewport_zoom = zoom_to_frame(vertical_size);
        let Ok(Some(camera)) = self.scene.world_camera(&self.world, self.scene_camera()) else {
            return;
        };
        self.viewport_pan = pan_to_centre(camera, self.viewport_pan, position);
    }

    pub(super) fn reset_view(&mut self) {
        self.viewport_yaw = 0.0;
        self.viewport_pitch = 0.0;
        self.viewport_pan = GlamVec2::ZERO;
        self.viewport_zoom = 1.0;
    }
}

/// Where a 2D scene's camera is and how tall a slice of the world it frames:
/// the first orthographic camera looking straight down -Z, if there is one.
pub(super) fn flat_camera(
    world: &sindri_core::World,
    components: &sindri_core::ComponentSchemaRegistry,
) -> Option<(Vec3, f32)> {
    components
        .query::<CameraComponent>(world)
        .unwrap_or_default()
        .into_iter()
        .find_map(|(entity, camera)| {
            let CameraComponent::Orthographic { vertical_size, .. } = camera else {
                return None;
            };
            let transform = world.get(entity)?.transform_3d?;
            let facing = safe_rotation(transform) * Vec3::NEG_Z;
            (facing.dot(Vec3::NEG_Z) > 0.999)
                .then(|| (Vec3::from_array(transform.position), vertical_size))
        })
}

/// The Scene view's zoom at which it frames `vertical_size` world units top
/// to bottom, as an orthographic camera of that size does.
///
/// The viewer frames a half height that shrinks as it zooms in, from what the
/// unzoomed viewer camera frames.
pub(super) fn zoom_to_frame(vertical_size: f32) -> f32 {
    let unzoomed = Vec3::new(3.0, 2.0, 4.0).length() * 22.5_f32.to_radians().tan();
    (unzoomed / (vertical_size / 2.0).max(f32::EPSILON)).clamp(MIN_ZOOM, MAX_ZOOM)
}

/// The middle of a set of points, or `None` for no points at all.
fn centre_of(points: &[Vec3]) -> Option<Vec3> {
    let mut total = Vec3::ZERO;
    let mut counted = 0.0_f32;
    for point in points {
        total += *point;
        counted += 1.0;
    }
    (counted > 0.0).then(|| total / counted)
}
