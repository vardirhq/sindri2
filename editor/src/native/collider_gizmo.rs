//! Colliders as the Scene view draws them: every collider's outline, and
//! handles on the selected one's pieces to size them by dragging.
//!
//! Collision is invisible, and sizing an invisible box by typing half extents
//! is guesswork. The outlines are drawn from the pieces the physics world is
//! given (a tilemap's merged rectangles included), so what is drawn is what
//! collides. A handle drags one edge while the opposite one stays where it
//! is, as Unity's Edit Collider does, and a whole drag is one undo step.

use eframe::egui::{self, Color32, LayerId, Order, Painter, Pos2, Rect, Response, Stroke};
use glam::{Mat4, Vec3};
use serde_json::{Value, json};
use sindri_core::{CommandBuffer, EntityId, SceneComponent, WorldCommand};
use sindri_scene::{
    Collider2d, Collider2dComponent, ColliderShape2d, CollisionShapes, RigidBodyKind,
    collision_shapes,
};

use super::EditorApp;
use super::projection::{project_point, project_segment, unproject_to_plane};

const COLLIDER_LAYER: &str = "sindri-collider-overlay";
const COLLIDER_DRAG: &str = "sindri-collider-drag";

/// Unity's collider green, so the colour means what an author expects.
const COLLIDER_GREEN: Color32 = Color32::from_rgb(140, 250, 140);

/// How near a press has to land to take a handle, in points.
const HANDLE_REACH: f32 = 8.0;
const HANDLE_SIZE: f32 = 7.0;

/// The smallest a piece may be dragged down to, so a handle never turns a
/// shape inside out or to nothing.
const SMALLEST: f32 = 0.02;

/// What a handle changes.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Grip {
    /// One edge of a box, along axis 0 (X) or 1 (Y), on the side of `sign`.
    Edge { axis: usize, sign: f32 },
    /// A circle's or a capsule's radius.
    Radius,
    /// A capsule's height: its top, with the bottom mirrored.
    Height,
}

/// One handle on screen.
#[derive(Clone, Copy, Debug)]
struct Handle {
    piece: usize,
    grip: Grip,
    at: Pos2,
}

/// A handle being dragged: the piece as it was when the drag began, so the
/// size follows the pointer rather than accumulating a frame's error at a time.
#[derive(Clone, Debug)]
struct ColliderDrag {
    entity: EntityId,
    piece: usize,
    grip: Grip,
    before: Collider2d,
}

/// Where a handle sits on a piece, in the piece's own space.
fn grips(shape: ColliderShape2d) -> Vec<(Grip, [f32; 2])> {
    match shape {
        ColliderShape2d::Box {
            half_extents: [x, y],
        } => vec![
            (Grip::Edge { axis: 0, sign: 1.0 }, [x, 0.0]),
            (
                Grip::Edge {
                    axis: 0,
                    sign: -1.0,
                },
                [-x, 0.0],
            ),
            (Grip::Edge { axis: 1, sign: 1.0 }, [0.0, y]),
            (
                Grip::Edge {
                    axis: 1,
                    sign: -1.0,
                },
                [0.0, -y],
            ),
        ],
        ColliderShape2d::Circle { radius } => vec![(Grip::Radius, [radius, 0.0])],
        ColliderShape2d::Capsule {
            half_height,
            radius,
        } => vec![
            (Grip::Radius, [radius, 0.0]),
            (Grip::Height, [0.0, half_height + radius]),
        ],
    }
}

/// The piece `before` with the handle `grip` dragged to `to`, a point in the
/// piece's own space as it was before the drag.
fn resized(before: Collider2d, grip: Grip, to: [f32; 2]) -> Collider2d {
    let mut piece = before;
    match (before.shape, grip) {
        (ColliderShape2d::Box { half_extents }, Grip::Edge { axis, sign }) => {
            // The opposite edge stays put; the dragged one follows the pointer.
            let fixed = -sign * half_extents[axis];
            let moved = if sign > 0.0 {
                to[axis].max(fixed + SMALLEST)
            } else {
                to[axis].min(fixed - SMALLEST)
            };
            let mut half = half_extents;
            half[axis] = (moved - fixed).abs() / 2.0;
            let mut shift = [0.0, 0.0];
            shift[axis] = f32::midpoint(moved, fixed);
            // The centre moved in the piece's own space, which the piece's
            // rotation turns into its parent's.
            let (sin, cos) = before.rotation.sin_cos();
            piece.offset = [
                before.offset[0] + shift[0] * cos - shift[1] * sin,
                before.offset[1] + shift[0] * sin + shift[1] * cos,
            ];
            piece.shape = ColliderShape2d::Box { half_extents: half };
        }
        (ColliderShape2d::Circle { .. }, Grip::Radius) => {
            piece.shape = ColliderShape2d::Circle {
                radius: to[0].hypot(to[1]).max(SMALLEST),
            };
        }
        (ColliderShape2d::Capsule { half_height, .. }, Grip::Radius) => {
            piece.shape = ColliderShape2d::Capsule {
                half_height,
                radius: to[0].abs().max(SMALLEST),
            };
        }
        (ColliderShape2d::Capsule { radius, .. }, Grip::Height) => {
            piece.shape = ColliderShape2d::Capsule {
                half_height: (to[1].abs() - radius).max(0.0),
                radius,
            };
        }
        _ => {}
    }
    piece
}

fn tone(kind: RigidBodyKind, selected: bool) -> Stroke {
    let colour = if selected {
        COLLIDER_GREEN
    } else if kind == RigidBodyKind::Static {
        COLLIDER_GREEN.gamma_multiply(0.45)
    } else {
        COLLIDER_GREEN.gamma_multiply(0.7)
    };
    Stroke::new(if selected { 1.75 } else { 1.0 }, colour)
}

fn paint_outline(
    painter: &Painter,
    rect: Rect,
    view_projection: Mat4,
    outline: &[Vec3],
    stroke: Stroke,
) {
    for (index, start) in outline.iter().enumerate() {
        let end = outline[(index + 1) % outline.len()];
        if let Some(segment) = project_segment(rect, view_projection, *start, end) {
            painter.line_segment(segment, stroke);
        }
    }
}

impl EditorApp {
    /// The authored collider on the selected entity, as the pieces it lists:
    /// the ones a handle may resize. A tilemap's pieces follow its tiles and
    /// have none.
    fn selected_collider(&self) -> Option<(EntityId, Vec<Collider2d>)> {
        let entity = self.selection.primary()?;
        let collider = self
            .scene
            .components()
            .get::<Collider2dComponent>(&self.world, entity)
            .ok()??;
        Some((entity, collider.0))
    }

    fn depth_of(&self, entity: EntityId) -> f32 {
        self.world
            .get(entity)
            .and_then(|data| data.transform_3d)
            .map_or(0.0, |transform| transform.position[2])
    }

    /// Draws every collider, and lets the selected one's handles be dragged.
    ///
    /// Returns whether a handle has the pointer, so the move gizmo and the
    /// camera leave the drag alone.
    pub(super) fn collider_overlay(
        &mut self,
        context: &egui::Context,
        response: &Response,
        blocked: bool,
    ) -> bool {
        let rect = response.rect;
        let aspect = rect.width() / rect.height().max(1.0);
        let Some(camera) = self
            .scene
            .world_camera_for_viewport(&self.world, aspect, self.scene_camera())
            .ok()
            .flatten()
        else {
            return false;
        };
        let view_projection = camera.view_projection;
        // A scene whose physics does not parse draws no outlines rather than
        // stale ones; the inspector names the broken field.
        let shapes = collision_shapes(&self.world, self.scene.components()).unwrap_or_default();
        let painter = context
            // Background, so the floating panels stay over it; a layer of
            // its own, which egui paints after the Scene view's.
            .layer_painter(LayerId::new(
                Order::Background,
                egui::Id::new(COLLIDER_LAYER),
            ))
            .with_clip_rect(rect);
        for shape in &shapes {
            let depth = self.depth_of(shape.entity);
            let stroke = tone(shape.kind, self.selection.contains(shape.entity));
            for outline in shape.outlines() {
                let outline: Vec<Vec3> = outline
                    .into_iter()
                    .map(|[x, y]| Vec3::new(x, y, depth))
                    .collect();
                paint_outline(&painter, rect, view_projection, &outline, stroke);
            }
        }

        let Some((entity, pieces)) = self.selected_collider() else {
            return false;
        };
        let Some(shape) = shapes.iter().find(|shape| shape.entity == entity) else {
            return false;
        };
        let depth = self.depth_of(entity);
        let handles: Vec<Handle> = pieces
            .iter()
            .enumerate()
            .flat_map(|(index, piece)| {
                grips(piece.shape)
                    .into_iter()
                    .filter_map(move |(grip, local)| {
                        let [x, y] = shape.piece_to_world(piece, local);
                        project_point(rect, view_projection, Vec3::new(x, y, depth)).map(|at| {
                            Handle {
                                piece: index,
                                grip,
                                at,
                            }
                        })
                    })
            })
            .collect();
        for handle in &handles {
            painter.rect(
                Rect::from_center_size(handle.at, egui::vec2(HANDLE_SIZE, HANDLE_SIZE)),
                1.0,
                COLLIDER_GREEN,
                Stroke::new(1.0, Color32::BLACK),
                egui::StrokeKind::Middle,
            );
        }
        self.drag_collider_handle(
            context,
            response,
            blocked,
            shape,
            &pieces,
            &handles,
            depth,
            view_projection,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn drag_collider_handle(
        &mut self,
        context: &egui::Context,
        response: &Response,
        blocked: bool,
        shape: &CollisionShapes,
        pieces: &[Collider2d],
        handles: &[Handle],
        depth: f32,
        view_projection: Mat4,
    ) -> bool {
        let id = egui::Id::new(COLLIDER_DRAG);
        let pointer = response.interact_pointer_pos().or(response.hover_pos());
        // A drag is only recognised once the pointer has travelled, by which
        // time it may be off the handle it was pressed on: where the press
        // began is what decides which handle it took.
        let pressed = context
            .input(|input| input.pointer.press_origin())
            .or(pointer);
        let near = pressed.and_then(|pointer| {
            handles
                .iter()
                .find(|handle| handle.at.distance(pointer) <= HANDLE_REACH)
        });
        let mut drag = context.data(|data| data.get_temp::<ColliderDrag>(id));
        if drag.is_none()
            && !blocked
            && response.drag_started_by(egui::PointerButton::Primary)
            && let Some(handle) = near
            && let Some(before) = pieces.get(handle.piece)
        {
            drag = Some(ColliderDrag {
                entity: shape.entity,
                piece: handle.piece,
                grip: handle.grip,
                before: *before,
            });
        }
        let Some(active) = drag.filter(|active| active.entity == shape.entity) else {
            context.data_mut(|data| data.remove::<ColliderDrag>(id));
            return near.is_some() && !blocked;
        };
        if response.dragged_by(egui::PointerButton::Primary)
            && let Some(pointer) = pointer
            && let Some(world) = unproject_to_plane(response.rect, view_projection, pointer, depth)
        {
            let to = shape.world_to_piece(&active.before, [world.x, world.y]);
            let mut next = pieces.to_vec();
            if let Some(piece) = next.get_mut(active.piece) {
                *piece = resized(active.before, active.grip, to);
            }
            self.write_collider(shape.entity, &next);
        }
        if response.drag_stopped_by(egui::PointerButton::Primary) {
            context.data_mut(|data| data.remove::<ColliderDrag>(id));
        } else {
            context.data_mut(|data| data.insert_temp(id, active));
        }
        true
    }

    /// Writes the collider back as its pieces, merged into one undo step for
    /// the whole drag.
    fn write_collider(&mut self, entity: EntityId, pieces: &[Collider2d]) {
        let pieces: Vec<Value> = pieces
            .iter()
            .filter_map(|piece| serde_json::to_value(piece).ok())
            .collect();
        let mut buffer = CommandBuffer::new();
        buffer.push(WorldCommand::SetComponent {
            entity,
            type_name: Collider2dComponent::TYPE_NAME.to_owned(),
            payload: json!({ "pieces": pieces }),
        });
        let transaction = buffer
            .into_transaction("Resize collider")
            .merging(format!("collider-handle:{}", entity.index()));
        if let Err(error) = self.history.apply(transaction, &mut self.world) {
            self.report(error.to_string());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn near(a: [f32; 2], b: [f32; 2]) -> bool {
        (a[0] - b[0]).abs() < 1.0e-5 && (a[1] - b[1]).abs() < 1.0e-5
    }

    #[test]
    fn dragging_a_box_edge_keeps_the_opposite_edge_where_it_was() {
        let before = Collider2d::rectangle([1.0, 0.5]);
        let grown = resized(before, Grip::Edge { axis: 0, sign: 1.0 }, [2.0, 0.3]);
        let ColliderShape2d::Box { half_extents } = grown.shape else {
            panic!("still a box")
        };
        // Left edge stays at -1; the right edge moves from 1 to 2.
        assert!(near(half_extents, [1.5, 0.5]));
        assert!(near(grown.offset, [0.5, 0.0]));
    }

    #[test]
    fn a_turned_box_grows_along_its_own_axis() {
        let before = Collider2d {
            rotation: std::f32::consts::FRAC_PI_2,
            ..Collider2d::rectangle([1.0, 1.0])
        };
        let grown = resized(before, Grip::Edge { axis: 0, sign: 1.0 }, [3.0, 0.0]);
        // Its own X is the world's Y, so the centre moves up, not right.
        assert!(near(grown.offset, [0.0, 1.0]));
    }

    #[test]
    fn an_edge_cannot_be_dragged_past_the_one_opposite() {
        let before = Collider2d::rectangle([1.0, 1.0]);
        let squashed = resized(
            before,
            Grip::Edge {
                axis: 1,
                sign: -1.0,
            },
            [0.0, 5.0],
        );
        let ColliderShape2d::Box { half_extents } = squashed.shape else {
            panic!("still a box")
        };
        assert!(half_extents[1] > 0.0 && half_extents[1] < 0.05);
    }

    #[test]
    fn a_circle_and_a_capsule_resize_from_their_handles() {
        let circle = resized(Collider2d::circle(0.5), Grip::Radius, [0.0, 2.0]);
        assert_eq!(circle.shape, ColliderShape2d::Circle { radius: 2.0 });
        let capsule = Collider2d {
            shape: ColliderShape2d::Capsule {
                half_height: 1.0,
                radius: 0.5,
            },
            ..Collider2d::circle(0.5)
        };
        let taller = resized(capsule, Grip::Height, [0.0, 3.0]);
        assert_eq!(
            taller.shape,
            ColliderShape2d::Capsule {
                half_height: 2.5,
                radius: 0.5
            }
        );
    }
}
