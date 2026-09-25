use crate::{EntityData, Transform3D, World};

fn at(position: [f32; 3], turn: f32, scale: f32) -> Transform3D {
    let mut transform = Transform3D {
        position,
        scale: [scale, scale, 1.0],
        ..Transform3D::default()
    };
    transform.set_rotation_z_radians(turn);
    transform
}

fn close(a: [f32; 3], b: [f32; 3]) -> bool {
    a.iter().zip(b).all(|(a, b)| (a - b).abs() < 1e-4)
}

fn spawn(world: &mut World, transform: Option<Transform3D>) -> crate::EntityId {
    world.spawn(EntityData {
        transform_3d: transform,
        ..EntityData::default()
    })
}

#[test]
fn a_child_moves_turns_and_grows_with_its_parent() {
    let mut world = World::default();
    let parent = spawn(
        &mut world,
        Some(at([10.0, 0.0, 0.0], std::f32::consts::FRAC_PI_2, 2.0)),
    );
    let child = spawn(&mut world, Some(at([1.0, 0.0, 0.5], 0.0, 1.0)));
    world.set_parent(child, Some(parent)).expect("parents");
    let placed = world.world_transform(child).expect("a transform");
    // One unit along the parent's x, which has turned to face up, doubled.
    assert!(
        close(placed.position, [10.0, 2.0, 0.5]),
        "{:?}",
        placed.position
    );
    assert!((placed.rotation_z_radians() - std::f32::consts::FRAC_PI_2).abs() < 1e-4);
    assert!(close(placed.scale, [2.0, 2.0, 1.0]));
}

#[test]
fn a_top_level_entity_is_where_it_says() {
    let mut world = World::default();
    let alone = spawn(&mut world, Some(at([3.0, 4.0, 0.0], 0.3, 1.5)));
    assert_eq!(
        world.world_transform(alone),
        Some(at([3.0, 4.0, 0.0], 0.3, 1.5))
    );
}

#[test]
fn a_parent_without_a_transform_only_groups() {
    let mut world = World::default();
    let root = spawn(&mut world, Some(at([5.0, 0.0, 0.0], 0.0, 1.0)));
    let group = spawn(&mut world, None);
    let child = spawn(&mut world, Some(at([1.0, 0.0, 0.0], 0.0, 1.0)));
    world.set_parent(group, Some(root)).expect("parents");
    world.set_parent(child, Some(group)).expect("parents");
    let placed = world.world_transform(child).expect("a transform");
    assert!(close(placed.position, [6.0, 0.0, 0.0]));
}

#[test]
fn relative_to_undoes_within() {
    let parent = at([2.0, -3.0, 1.0], 0.7, 1.6);
    let world = at([-4.0, 5.0, 2.0], -0.4, 0.8);
    let back = world.relative_to(parent).within(parent);
    assert!(close(back.position, world.position));
    assert!(close(back.scale, world.scale));
    assert!((back.rotation_z_radians() - world.rotation_z_radians()).abs() < 1e-4);
}

#[test]
fn reparenting_keeps_an_entity_where_it_was() {
    let mut world = World::default();
    let parent = spawn(&mut world, Some(at([10.0, 5.0, 0.0], 1.0, 2.0)));
    let child = spawn(&mut world, Some(at([3.0, 3.0, 0.0], 0.2, 1.0)));
    world
        .set_parent_keeping_place(child, Some(parent))
        .expect("parents");
    let placed = world.world_transform(child).expect("a transform");
    assert!(
        close(placed.position, [3.0, 3.0, 0.0]),
        "{:?}",
        placed.position
    );
    assert!(close(placed.scale, [1.0, 1.0, 1.0]));
    world
        .set_parent_keeping_place(child, None)
        .expect("unparents");
    let local = world
        .get(child)
        .and_then(|data| data.transform_3d)
        .expect("stored");
    assert!(close(local.position, [3.0, 3.0, 0.0]));
}
