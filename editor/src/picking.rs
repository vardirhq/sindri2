//! CPU-side picking for renderable entities in the Scene viewport.
//!
//! Picking uses the exact view-projection matrix used to draw the frame and
//! the same local geometry as the renderers. It deliberately remains an editor
//! concern: selecting an entity must not add IDs or readback buffers to a game
//! frame.

use glam::{Mat4, Quat, Vec3};
use sindri_core::{ComponentRegistryError, ComponentSchemaRegistry, EntityId, Transform3D, World};
use sindri_scene::{
    MeshComponent, MeshPrimitive, OverlayPlacement, OverlayView, SpriteComponent, TilemapComponent,
    UiImageComponent,
};

#[derive(Clone, Copy, Debug)]
struct Hit {
    entity: EntityId,
    depth: f32,
    layer: i32,
}

/// Selects the topmost world-space renderable under a normalized viewport point.
///
/// Opaque geometry blocks sprites behind it. Among visible transparent
/// renderables, the higher authored layer wins; distance and then entity index
/// break ties in the same direction as the renderer's back-to-front ordering.
pub fn pick_world(
    world: &World,
    components: &ComponentSchemaRegistry,
    view_projection: Mat4,
    point: [f32; 2],
) -> Result<Option<EntityId>, ComponentRegistryError> {
    let Some(ray) = RaySegment::through_viewport(view_projection, point) else {
        return Ok(None);
    };

    let nearest_mesh = components
        .query::<MeshComponent>(world)?
        .into_iter()
        .filter_map(|(entity, mesh)| {
            let transform = transform_of(world, entity);
            mesh_hit(ray, transform, mesh.primitive).map(|depth| Hit {
                entity,
                depth,
                layer: mesh.layer,
            })
        })
        .min_by(nearer_hit);

    let mesh_depth = nearest_mesh.map_or(f32::INFINITY, |hit| hit.depth);
    let sprites = components
        .query::<SpriteComponent>(world)?
        .into_iter()
        .filter(|(_, sprite)| is_drawn(sprite.tint))
        .filter_map(|(entity, sprite)| {
            let transform = transform_of(world, entity);
            plane_hit(ray, transform, 0.5).map(|(depth, _)| Hit {
                entity,
                depth,
                layer: sprite.layer,
            })
        });
    let tilemaps = components
        .query::<TilemapComponent>(world)?
        .into_iter()
        .filter_map(|(entity, map)| {
            let transform = transform_of(world, entity);
            let (depth, local) = plane_hit(ray, transform, f32::INFINITY)?;
            let (column, row) = map.local_to_tile(local.x, local.y)?;
            map.tile(column, row)?;
            Some(Hit {
                entity,
                depth,
                layer: map.layer,
            })
        });

    let transparent = sprites
        .chain(tilemaps)
        .filter(|hit| hit.depth < mesh_depth)
        .max_by(frontmost_transparent);
    Ok(transparent.or(nearest_mesh).map(|hit| hit.entity))
}

/// Selects the topmost UI element under a normalized viewport point.
///
/// A pass of its own, because UI is not in the world. It is laid out against
/// the viewport — an anchor picks a point, and the transform is an offset from
/// it — so a world ray through a world camera passes nowhere near it. Twelve of
/// Gather's twenty-two entities are UI, and none of them could be clicked in
/// the view that draws them.
///
/// Only images. What a string of UI text covers is decided by glyph layout
/// inside the text renderer, and a guessed box for it would select the wrong
/// thing near its edges; a UI text entity is still reachable from the hierarchy,
/// and its gizmo now appears where the text is.
pub fn pick_ui(
    world: &World,
    components: &ComponentSchemaRegistry,
    overlay: OverlayView,
    placement: &OverlayPlacement,
    point: [f32; 2],
) -> Result<Option<EntityId>, ComponentRegistryError> {
    let Some(ray) = RaySegment::through_viewport(overlay.view_projection, point) else {
        return Ok(None);
    };
    Ok(components
        .query::<UiImageComponent>(world)?
        .into_iter()
        .filter(|(_, image)| is_drawn(image.tint))
        .filter_map(|(entity, image)| {
            let model = placement.place(transform_of(world, entity), image.anchor);
            plane_hit_model(ray, model, 0.5).map(|(depth, _)| Hit {
                entity,
                depth,
                layer: image.layer,
            })
        })
        // The overlay is flat, so depth separates nothing: what decides which
        // of two overlapping elements is on top is the layer, then the entity
        // index, in the same direction the renderer stacks them.
        .max_by(frontmost_transparent)
        .map(|hit| hit.entity))
}

#[derive(Clone, Copy, Debug)]
struct RaySegment {
    near: Vec3,
    far: Vec3,
}

impl RaySegment {
    fn through_viewport(view_projection: Mat4, point: [f32; 2]) -> Option<Self> {
        if !(0.0..=1.0).contains(&point[0]) || !(0.0..=1.0).contains(&point[1]) {
            return None;
        }
        let inverse = view_projection.inverse();
        if !matrix_is_finite(inverse) {
            return None;
        }
        let x = point[0] * 2.0 - 1.0;
        let y = 1.0 - point[1] * 2.0;
        let near = inverse.project_point3(Vec3::new(x, y, 0.0));
        let far = inverse.project_point3(Vec3::new(x, y, 1.0));
        (near.is_finite() && far.is_finite()).then_some(Self { near, far })
    }

    fn in_local_space(self, transform: Transform3D) -> Option<Self> {
        self.in_model_space(transform_matrix(transform))
    }

    /// The same, for something placed by a matrix rather than by a transform.
    ///
    /// A UI element is one: where it lands is its transform *and* its anchor
    /// *and* the viewport's shape, which only the overlay can put together.
    fn in_model_space(self, model: Mat4) -> Option<Self> {
        let inverse = model.inverse();
        if !matrix_is_finite(inverse) {
            return None;
        }
        Some(Self {
            near: inverse.transform_point3(self.near),
            far: inverse.transform_point3(self.far),
        })
    }
}

fn mesh_hit(ray: RaySegment, transform: Transform3D, primitive: MeshPrimitive) -> Option<f32> {
    match primitive {
        MeshPrimitive::Cube => cube_hit(ray.in_local_space(transform)?),
        _ => None,
    }
}

/// Intersects the renderer's cube, whose local vertices span `[-1, 1]`.
fn cube_hit(ray: RaySegment) -> Option<f32> {
    let direction = ray.far - ray.near;
    let mut entering = 0.0_f32;
    let mut leaving = 1.0_f32;
    for (origin, direction) in [
        (ray.near.x, direction.x),
        (ray.near.y, direction.y),
        (ray.near.z, direction.z),
    ] {
        if direction.abs() <= f32::EPSILON {
            if !(-1.0..=1.0).contains(&origin) {
                return None;
            }
            continue;
        }
        let first = (-1.0 - origin) / direction;
        let second = (1.0 - origin) / direction;
        entering = entering.max(first.min(second));
        leaving = leaving.min(first.max(second));
        if entering > leaving {
            return None;
        }
    }
    (entering <= 1.0 && leaving >= 0.0).then_some(entering.max(0.0))
}

/// Intersects a local Z=0 plane, optionally bounded like the sprite quad.
fn plane_hit(ray: RaySegment, transform: Transform3D, half_extent: f32) -> Option<(f32, Vec3)> {
    plane_hit_model(ray, transform_matrix(transform), half_extent)
}

/// The same, against a model matrix that was not built from a transform alone.
fn plane_hit_model(ray: RaySegment, model: Mat4, half_extent: f32) -> Option<(f32, Vec3)> {
    let ray = ray.in_model_space(model)?;
    let direction = ray.far - ray.near;
    if direction.z.abs() <= f32::EPSILON {
        return None;
    }
    let depth = -ray.near.z / direction.z;
    if !(0.0..=1.0).contains(&depth) {
        return None;
    }
    let point = ray.near + direction * depth;
    (point.x.abs() <= half_extent && point.y.abs() <= half_extent).then_some((depth, point))
}

/// Whether a tint puts anything on the screen at all.
///
/// A fully transparent element is drawn as nothing, and clicking nothing must
/// not select it. Gather's win banner is exactly this: `tint` alpha zero, a
/// third of the viewport wide, sitting in the middle of the scene until the
/// game says otherwise. Picked, it would swallow every click in the centre of
/// the view and select an element nobody can see.
///
/// Exactly zero rather than a threshold: anything above it is visible, however
/// faintly, and a faint thing is still a thing someone aimed at. What is hidden
/// this way is still selectable from the hierarchy, which is where an invisible
/// entity is reachable at all.
fn is_drawn(tint: [f32; 4]) -> bool {
    tint[3] > 0.0
}

fn transform_of(world: &World, entity: EntityId) -> Transform3D {
    world
        .get(entity)
        .and_then(|data| data.transform_3d)
        .unwrap_or_default()
}

fn transform_matrix(transform: Transform3D) -> Mat4 {
    Mat4::from_scale_rotation_translation(
        Vec3::from_array(transform.scale),
        Quat::from_array(transform.rotation),
        Vec3::from_array(transform.position),
    )
}

fn matrix_is_finite(matrix: Mat4) -> bool {
    matrix.to_cols_array().into_iter().all(f32::is_finite)
}

fn nearer_hit(left: &Hit, right: &Hit) -> std::cmp::Ordering {
    left.depth
        .total_cmp(&right.depth)
        .then_with(|| left.layer.cmp(&right.layer))
        .then_with(|| left.entity.index().cmp(&right.entity.index()))
}

fn frontmost_transparent(left: &Hit, right: &Hit) -> std::cmp::Ordering {
    left.layer
        .cmp(&right.layer)
        .then_with(|| right.depth.total_cmp(&left.depth))
        .then_with(|| left.entity.index().cmp(&right.entity.index()))
}

#[cfg(test)]
mod tests;
