//! CPU-side picking for renderable entities in the Scene viewport.
//!
//! Picking uses the exact view-projection matrix used to draw the frame and
//! the same local geometry as the renderers. It deliberately remains an editor
//! concern: selecting an entity must not add IDs or readback buffers to a game
//! frame.

use glam::{Mat4, Quat, Vec3};
use sindri_core::{ComponentRegistryError, ComponentSchemaRegistry, EntityId, Transform3D, World};
use sindri_scene::{MeshComponent, MeshPrimitive, SpriteComponent, TilemapComponent};

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
        let inverse = transform_matrix(transform).inverse();
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
    let ray = ray.in_local_space(transform)?;
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
mod tests {
    use std::collections::BTreeMap;

    use serde_json::{Value, json};
    use sindri_core::EntityData;
    use sindri_scene::SceneExtractor;

    use super::*;

    fn spawn(
        world: &mut World,
        transform: Transform3D,
        type_name: &str,
        payload: Value,
    ) -> EntityId {
        world.spawn(EntityData {
            transform_3d: Some(transform),
            components: BTreeMap::from([(type_name.to_owned(), payload)]),
            ..EntityData::default()
        })
    }

    fn sprite(layer: i32) -> Value {
        json!({
            "texture": "procedural:checkerboard",
            "space": "world",
            "layer": layer
        })
    }

    #[test]
    fn a_click_selects_the_sprite_quad_and_misses_outside_it() {
        let extractor = SceneExtractor::new().unwrap();
        let mut world = World::default();
        let sprite = spawn(
            &mut world,
            Transform3D {
                position: [0.0, 0.0, 0.5],
                ..Transform3D::default()
            },
            "sindri.sprite",
            sprite(0),
        );

        assert_eq!(
            pick_world(&world, extractor.components(), Mat4::IDENTITY, [0.5, 0.5]).unwrap(),
            Some(sprite)
        );
        assert_eq!(
            pick_world(&world, extractor.components(), Mat4::IDENTITY, [0.9, 0.5]).unwrap(),
            None
        );
    }

    #[test]
    fn the_higher_sprite_layer_wins_even_when_it_is_farther_back() {
        let extractor = SceneExtractor::new().unwrap();
        let mut world = World::default();
        let _near = spawn(
            &mut world,
            Transform3D {
                position: [0.0, 0.0, 0.2],
                ..Transform3D::default()
            },
            "sindri.sprite",
            sprite(0),
        );
        let high_layer = spawn(
            &mut world,
            Transform3D {
                position: [0.0, 0.0, 0.8],
                ..Transform3D::default()
            },
            "sindri.sprite",
            sprite(1),
        );

        assert_eq!(
            pick_world(&world, extractor.components(), Mat4::IDENTITY, [0.5, 0.5]).unwrap(),
            Some(high_layer)
        );
    }

    #[test]
    fn opaque_geometry_blocks_a_sprite_behind_but_not_one_in_front() {
        let extractor = SceneExtractor::new().unwrap();
        let mut world = World::default();
        let cube = spawn(
            &mut world,
            Transform3D {
                position: [0.0, 0.0, 0.6],
                scale: [0.1, 0.1, 0.1],
                ..Transform3D::default()
            },
            "sindri.mesh",
            json!({
                "primitive": "cube",
                "texture": "procedural:checkerboard"
            }),
        );
        let _behind = spawn(
            &mut world,
            Transform3D {
                position: [0.0, 0.0, 0.8],
                ..Transform3D::default()
            },
            "sindri.sprite",
            sprite(10),
        );

        assert_eq!(
            pick_world(&world, extractor.components(), Mat4::IDENTITY, [0.5, 0.5]).unwrap(),
            Some(cube)
        );

        let in_front = spawn(
            &mut world,
            Transform3D {
                position: [0.0, 0.0, 0.2],
                ..Transform3D::default()
            },
            "sindri.sprite",
            sprite(-10),
        );
        assert_eq!(
            pick_world(&world, extractor.components(), Mat4::IDENTITY, [0.5, 0.5]).unwrap(),
            Some(in_front)
        );
    }

    #[test]
    fn only_a_filled_tilemap_cell_selects_the_map() {
        let extractor = SceneExtractor::new().unwrap();
        let mut world = World::default();
        let map = spawn(
            &mut world,
            Transform3D {
                position: [-0.5, 0.5, 0.5],
                ..Transform3D::default()
            },
            "sindri.tilemap",
            json!({
                "texture": "textures/tiles.png",
                "palette": ["floor"],
                "columns": 2,
                "rows": 1,
                "tiles": [0, null],
                "space": "world"
            }),
        );

        assert_eq!(
            pick_world(&world, extractor.components(), Mat4::IDENTITY, [0.5, 0.5]).unwrap(),
            Some(map)
        );
        assert_eq!(
            pick_world(&world, extractor.components(), Mat4::IDENTITY, [1.0, 0.5]).unwrap(),
            None
        );
    }
}
