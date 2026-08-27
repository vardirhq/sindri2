//! Deciding which camera a pass is drawn through.
//!
//! A viewer's camera — the editor's Scene view — overrides what the
//! scene authored; with no viewer, the authored cameras decide, and a
//! scene that authored none draws through the screen-space default.

pub(super) mod view;

use glam::{Mat4, Vec2, Vec3};
use sindri_core::{Transform3D, World};
use sindri_render::{PerspectiveCamera, orthographic_projection, perspective_projection};

use self::view::{
    CameraView, OverlayExtent, WorldProjection, orbited_offset, panned_shift,
    resolved_screen_overlay, safe_rotation,
};
use crate::{CameraComponent, UiAnchor};

use super::ui::ui_matrix;

use super::{SceneExtractError, SceneExtractor};

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct ResolvedCameras {
    pub(super) world: Option<ResolvedCamera>,
    /// Viewport-owned screen projection. This is not an authored camera.
    pub(super) overlay: Option<ResolvedCamera>,
    pub(super) overlay_extent: Option<OverlayExtent>,
}

/// A camera as extraction needs it: the matrix that draws through it, and the
/// view on its own, which is what a distance is measured in.
#[derive(Clone, Copy, Debug)]
pub(super) struct ResolvedCamera {
    pub(super) view: Mat4,
    pub(super) view_projection: Mat4,
    pub(super) framed_half_height: f32,
}

/// The world camera as a viewport's own chrome and camera controls need it.
///
/// Handed out by [`SceneExtractor::world_camera`], which is the only supported
/// way to ask: everything here is derived from the authored camera and the
/// viewer's adjustment together, and deriving it a second time somewhere else
/// is how two answers about the same camera come to disagree.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ViewCamera {
    /// The matrix a frame drawn now would be seen through.
    pub view: Mat4,
    /// The exact projection and view used for a viewport of the requested
    /// aspect ratio. Inverting it turns a pointer into a world-space ray.
    pub view_projection: Mat4,
    /// Half the height the camera frames at its target, in world units.
    ///
    /// This is the unit a pan is measured in — a pan of one moves the picture
    /// by exactly this much — so it is also what turns a distance on screen
    /// back into a pan, which is how a viewport centres itself on something.
    pub framed_half_height: f32,
}

/// The overlay a UI element is laid out against, for one viewport.
///
/// The overlay is viewport-owned rather than camera-owned — no scene entity
/// decides where it is, which is why moving or deleting a gameplay camera
/// cannot move a HUD — so it needs nothing but the aspect ratio to resolve.
///
/// Handed out for the same reason [`ViewCamera`] is: the editor has to turn a
/// pointer back into the element under it, and a matrix rebuilt in the editor
/// is a second answer about the same overlay that only has to disagree once.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OverlayView {
    /// The exact projection and view a UI element is drawn through.
    pub view_projection: Mat4,
    /// Half the height the overlay spans, in overlay units. A UI element's
    /// position is measured in these, so this is what turns a drag on screen
    /// into an offset from an anchor.
    pub framed_half_height: f32,
}

/// Where the overlay sits for a viewport of this shape.
pub fn overlay_for_viewport(aspect: f32) -> Option<(OverlayView, OverlayPlacement)> {
    let resolved = resolved_screen_overlay(aspect);
    let camera = resolved.overlay?;
    let extent = resolved.overlay_extent?;
    Some((
        OverlayView {
            view_projection: camera.view_projection,
            framed_half_height: camera.framed_half_height,
        },
        OverlayPlacement { extent },
    ))
}

/// Where an anchored element lands on the overlay.
///
/// A separate type from [`OverlayView`] because it answers a different
/// question — not "what is this drawn through" but "where on the viewport does
/// this end up" — and because it holds the extent, which is the only part a
/// caller outside this crate has no business reading field by field.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OverlayPlacement {
    extent: OverlayExtent,
}

impl OverlayPlacement {
    /// The model matrix a UI element with this transform and anchor is drawn
    /// with — the same one the frame uses, from the same function.
    #[must_use]
    pub fn place(&self, transform: Transform3D, anchor: UiAnchor) -> Mat4 {
        ui_matrix(transform, anchor, self.extent)
    }

    /// Where the element's own origin sits, in overlay units.
    ///
    /// What a transform gizmo for a UI element is drawn at. Drawn at the raw
    /// transform instead, it appears wherever that offset happens to point in
    /// the world — for an element anchored top-centre with a small offset,
    /// that is the middle of the viewport rather than the element.
    #[must_use]
    pub fn origin(&self, transform: Transform3D, anchor: UiAnchor) -> Vec2 {
        let unit = Vec2::from_array(anchor.unit_offset());
        self.extent.center
            + unit * self.extent.half_extent
            + Vec2::from_array(transform.position_2d())
    }
}

impl SceneExtractor {
    /// Where the world camera ends up looking, under the same adjustment a
    /// frame would be extracted with.
    ///
    /// An editor paints chrome of its own — an axis indicator, a grid, a
    /// gizmo — and moves the camera on the user's behalf, and both need to know
    /// which way the world is facing and how much of it is framed. Without this
    /// it either extracts a frame it throws away or keeps a second copy of the
    /// orbit maths, and a second copy is how an indicator ends up disagreeing
    /// with the picture it sits on top of.
    ///
    /// No projection: chrome sits in the corner of a viewport rather than in
    /// the world, and where a thing is on screen relative to the middle does
    /// not depend on how the world is flattened. `None` means the world holds
    /// no authored camera, which is what extraction reports as
    /// [`SceneExtractError::MissingWorldCamera`] when world content needs one.
    pub fn world_camera(
        &self,
        world: &World,
        view: CameraView,
    ) -> Result<Option<ViewCamera>, SceneExtractError> {
        // Any aspect ratio will do for camera controls and corner chrome. A
        // tool that maps a viewport point back into the world asks
        // `world_camera_for_viewport` with the actual one.
        self.world_camera_for_viewport(world, 1.0, view)
    }

    /// Where the world camera looks, including the projection for one viewport.
    ///
    /// Tile painting is the first editor action that travels from a screen
    /// point back into the world. It must invert the exact view-projection the
    /// frame used; rebuilding one in the editor would be a second camera that
    /// only has to disagree once for every click to land on the wrong tile.
    pub fn world_camera_for_viewport(
        &self,
        world: &World,
        aspect: f32,
        view: CameraView,
    ) -> Result<Option<ViewCamera>, SceneExtractError> {
        Ok(self
            .resolve_cameras(world, aspect, view)?
            .world
            .map(|camera| ViewCamera {
                view: camera.view,
                view_projection: camera.view_projection,
                framed_half_height: camera.framed_half_height,
            }))
    }

    pub(super) fn resolve_cameras(
        &self,
        world: &World,
        aspect: f32,
        view: CameraView,
    ) -> Result<ResolvedCameras, SceneExtractError> {
        if !view.distance_scale.is_finite() || view.distance_scale <= 0.0 {
            return Err(SceneExtractError::InvalidCameraDistanceScale);
        }
        if !view.pan.is_finite() {
            return Err(SceneExtractError::InvalidCameraPan);
        }

        match view.projection {
            WorldProjection::Authored => self.resolve_authored_cameras(world, aspect),
            WorldProjection::Perspective | WorldProjection::Orthographic => {
                Ok(Self::resolve_viewer_cameras(aspect, view))
            }
        }
    }

    pub(super) fn resolve_viewer_cameras(aspect: f32, view: CameraView) -> ResolvedCameras {
        // Scene/editor world viewing is independent of authored cameras. Screen
        // overlay projection is viewport-owned too, so nothing in this path
        // resolves a camera entity at all.
        let mut resolved = resolved_screen_overlay(aspect);
        let up = Vec3::Y;
        let offset = orbited_offset(Vec3::new(3.0, 2.0, 4.0), up, view);
        let vertical_fov_radians = 45.0_f32.to_radians();
        let near = 0.1;
        let far = 1_000.0;
        let half_height = offset.length() * (vertical_fov_radians * 0.5).tan();
        let shift = panned_shift(offset, up, view.pan * half_height);
        let target = shift;
        let eye = target + offset;
        let camera = PerspectiveCamera {
            eye,
            target,
            up,
            vertical_fov_radians,
            near,
            far,
        };
        let projection = match view.projection {
            WorldProjection::Perspective => {
                perspective_projection(vertical_fov_radians, aspect, near, far)
            }
            WorldProjection::Orthographic => {
                let half_width = half_height * aspect;
                orthographic_projection(
                    -half_width,
                    half_width,
                    -half_height,
                    half_height,
                    near,
                    far,
                )
            }
            WorldProjection::Authored => unreachable!("viewer cameras are never authored"),
        };
        let view = camera.view();
        resolved.world = Some(ResolvedCamera {
            view,
            view_projection: projection * view,
            framed_half_height: half_height,
        });
        resolved
    }

    /// Resolves the one authored world camera.
    ///
    /// Perspective and orthographic are projection choices of the same role;
    /// both use `Transform3D` for position and orientation. Multiple authored
    /// cameras are rejected rather than relying on entity iteration order.
    pub(super) fn resolve_authored_cameras(
        &self,
        world: &World,
        aspect: f32,
    ) -> Result<ResolvedCameras, SceneExtractError> {
        let mut resolved = resolved_screen_overlay(aspect);
        for (entity, camera) in self.components.query::<CameraComponent>(world)? {
            if resolved.world.is_some() {
                return Err(SceneExtractError::MultipleWorldCameras);
            }
            let transform = world
                .get(entity)
                .and_then(|data| data.transform_3d)
                .unwrap_or_default();
            let eye = Vec3::from_array(transform.position);
            let rotation = safe_rotation(transform);
            // Authored cameras are ordinary transformed entities. Scale has no
            // projection meaning, so the camera pose is exactly rotation plus
            // translation and the view is its inverse.
            let view = Mat4::from_rotation_translation(rotation, eye).inverse();

            resolved.world = Some(match camera {
                CameraComponent::Perspective {
                    vertical_fov_degrees,
                    near,
                    far,
                } => {
                    let vertical_fov_radians = vertical_fov_degrees.to_radians();
                    ResolvedCamera {
                        view,
                        view_projection: perspective_projection(
                            vertical_fov_radians,
                            aspect,
                            near,
                            far,
                        ) * view,
                        // Without an authored target there is no privileged
                        // focus distance. One world unit is the neutral measure
                        // for callers that only need a scale with this view.
                        framed_half_height: (vertical_fov_radians * 0.5).tan(),
                    }
                }
                CameraComponent::Orthographic {
                    vertical_size,
                    near,
                    far,
                } => {
                    let half_height = vertical_size * 0.5;
                    let half_width = half_height * aspect;
                    ResolvedCamera {
                        view,
                        view_projection: orthographic_projection(
                            -half_width,
                            half_width,
                            -half_height,
                            half_height,
                            near,
                            far,
                        ) * view,
                        framed_half_height: half_height,
                    }
                }
            });
        }
        Ok(resolved)
    }
}
