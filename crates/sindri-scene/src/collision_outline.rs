//! Where a scene's colliders are, as outlines a view can draw.
//!
//! Collision is invisible, and an invisible thing is guessed at: a character
//! that stops short of a wall, or falls through a ledge, gives no clue which
//! shape was wrong. The editor draws these so the author sees what collides.
//! They come from the very pieces the physics world is given, a tilemap's
//! merged rectangles included, so what is drawn is what collides rather than
//! a second idea of it.

use sindri_core::{ComponentSchemaRegistry, EntityId, World};
use sindri_physics::{Collider2d, ColliderShape2d, PhysicsPose2d, RigidBodyKind};

use crate::physics::RigidBody2dComponent;
use crate::physics_sync::{PhysicsSyncError, collider_pieces, pose_of};

/// How many straight edges a circle is drawn with.
const ROUND: usize = 32;

/// One entity's collision, as physics is given it.
#[derive(Clone, Debug, PartialEq)]
pub struct CollisionShapes {
    pub entity: EntityId,
    pub pose: PhysicsPose2d,
    pub kind: RigidBodyKind,
    pub pieces: Vec<Collider2d>,
}

/// Every entity's collision in `world`, in entity order.
pub fn collision_shapes(
    world: &World,
    components: &ComponentSchemaRegistry,
) -> Result<Vec<CollisionShapes>, PhysicsSyncError> {
    collider_pieces(world, components)?
        .into_iter()
        .map(|(entity, pieces)| {
            let body = components
                .get::<RigidBody2dComponent>(world, entity)?
                .map(|authored| authored.0);
            Ok(CollisionShapes {
                entity,
                pose: pose_of(world, entity, body),
                kind: body.map_or(RigidBodyKind::Static, |body| body.kind),
                pieces,
            })
        })
        .collect()
}

impl CollisionShapes {
    /// Each piece's outline, as a closed loop of points in world XY.
    pub fn outlines(&self) -> impl Iterator<Item = Vec<[f32; 2]>> + '_ {
        self.pieces
            .iter()
            .map(|piece| place(self.pose, piece, local_outline(piece.shape)))
    }

    /// Where a point in a piece's own space is in the world.
    #[must_use]
    pub fn piece_to_world(&self, piece: &Collider2d, point: [f32; 2]) -> [f32; 2] {
        place(self.pose, piece, vec![point])[0]
    }

    /// Where a point in the world is in a piece's own space: the inverse of
    /// [`Self::piece_to_world`], which is what a handle dragged in the world
    /// needs to say how big the piece now is.
    #[must_use]
    pub fn world_to_piece(&self, piece: &Collider2d, point: [f32; 2]) -> [f32; 2] {
        let local = turn(
            [
                point[0] - self.pose.position[0],
                point[1] - self.pose.position[1],
            ],
            -self.pose.rotation,
        );
        turn(
            [local[0] - piece.offset[0], local[1] - piece.offset[1]],
            -piece.rotation,
        )
    }
}

fn turn([x, y]: [f32; 2], angle: f32) -> [f32; 2] {
    let (sin, cos) = angle.sin_cos();
    [x * cos - y * sin, x * sin + y * cos]
}

/// A shape's outline around its own centre, before its offset and rotation.
fn local_outline(shape: ColliderShape2d) -> Vec<[f32; 2]> {
    match shape {
        ColliderShape2d::Box {
            half_extents: [x, y],
        } => {
            vec![[-x, -y], [x, -y], [x, y], [-x, y]]
        }
        ColliderShape2d::Circle { radius } => arc(radius, [0.0, 0.0], 0.0, ROUND),
        // Upright, as the backend builds it: a half circle on top and one
        // underneath, joined by the two straight sides.
        ColliderShape2d::Capsule {
            half_height,
            radius,
        } => {
            let half = ROUND / 2;
            let mut points = arc(radius, [0.0, half_height], 0.0, half);
            points.extend(arc(radius, [0.0, -half_height], std::f32::consts::PI, half));
            points
        }
    }
}

/// Half a turn of points (or a whole one, for `ROUND`), starting `from`
/// radians round from +X.
#[allow(clippy::cast_precision_loss)]
fn arc(radius: f32, centre: [f32; 2], from: f32, steps: usize) -> Vec<[f32; 2]> {
    let whole = steps == ROUND;
    let sweep = if whole {
        std::f32::consts::TAU
    } else {
        std::f32::consts::PI
    };
    // A whole circle closes on its first point; a half includes both ends.
    let count = if whole { steps } else { steps + 1 };
    (0..count)
        .map(|step| {
            let angle = from + sweep * step as f32 / steps as f32;
            [
                centre[0] + radius * angle.cos(),
                centre[1] + radius * angle.sin(),
            ]
        })
        .collect()
}

/// Moves a piece's outline to where the piece is in the world.
fn place(pose: PhysicsPose2d, piece: &Collider2d, points: Vec<[f32; 2]>) -> Vec<[f32; 2]> {
    points
        .into_iter()
        .map(|point| {
            let [x, y] = turn(point, piece.rotation);
            let local = [x + piece.offset[0], y + piece.offset[1]];
            let [x, y] = turn(local, pose.rotation);
            [x + pose.position[0], y + pose.position[1]]
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn near(a: [f32; 2], b: [f32; 2]) -> bool {
        (a[0] - b[0]).abs() < 1.0e-5 && (a[1] - b[1]).abs() < 1.0e-5
    }

    fn shapes(pose: PhysicsPose2d, piece: Collider2d) -> CollisionShapes {
        CollisionShapes {
            entity: World::default().spawn(sindri_core::EntityData::default()),
            pose,
            kind: RigidBodyKind::Static,
            pieces: vec![piece],
        }
    }

    #[test]
    fn a_box_is_drawn_where_its_entity_and_offset_put_it() {
        let piece = Collider2d {
            offset: [1.0, 0.0],
            ..Collider2d::rectangle([0.5, 0.25])
        };
        let pose = PhysicsPose2d {
            position: [10.0, 5.0],
            rotation: 0.0,
        };
        let outline: Vec<_> = shapes(pose, piece).outlines().next().unwrap();
        assert!(near(outline[0], [10.5, 4.75]));
        assert!(near(outline[2], [11.5, 5.25]));
    }

    #[test]
    fn a_turned_entity_turns_its_pieces_about_itself() {
        let piece = Collider2d {
            offset: [1.0, 0.0],
            ..Collider2d::circle(0.5)
        };
        let pose = PhysicsPose2d {
            position: [0.0, 0.0],
            rotation: std::f32::consts::FRAC_PI_2,
        };
        let outline: Vec<_> = shapes(pose, piece).outlines().next().unwrap();
        let centre = outline.iter().fold([0.0, 0.0], |sum, point| {
            [sum[0] + point[0], sum[1] + point[1]]
        });
        #[allow(clippy::cast_precision_loss)]
        let count = outline.len() as f32;
        assert!(near([centre[0] / count, centre[1] / count], [0.0, 1.0]));
    }

    #[test]
    fn world_to_piece_undoes_piece_to_world() {
        let piece = Collider2d {
            offset: [1.0, -2.0],
            rotation: 0.4,
            ..Collider2d::circle(0.5)
        };
        let pose = PhysicsPose2d {
            position: [3.0, 5.0],
            rotation: -1.1,
        };
        let shapes = shapes(pose, piece);
        let world = shapes.piece_to_world(&piece, [0.7, 0.2]);
        assert!(near(shapes.world_to_piece(&piece, world), [0.7, 0.2]));
    }

    #[test]
    fn a_capsule_stands_upright_and_reaches_its_full_height() {
        let piece = Collider2d {
            shape: ColliderShape2d::Capsule {
                half_height: 1.0,
                radius: 0.5,
            },
            ..Collider2d::circle(0.5)
        };
        let outline: Vec<_> = shapes(PhysicsPose2d::default(), piece)
            .outlines()
            .next()
            .unwrap();
        let top = outline
            .iter()
            .map(|point| point[1])
            .fold(f32::MIN, f32::max);
        let side = outline
            .iter()
            .map(|point| point[0])
            .fold(f32::MIN, f32::max);
        assert!((top - 1.5).abs() < 1.0e-5, "{top}");
        assert!((side - 0.5).abs() < 1.0e-5, "{side}");
    }
}
