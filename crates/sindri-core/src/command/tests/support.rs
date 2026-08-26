//! Worlds and readings the command tests share.

use crate::{CommandBuffer, EntityData, EntityId, Transaction, World, WorldCommand};

pub(super) fn world_with_two_entities() -> (World, EntityId, EntityId) {
    let mut world = World::default();
    let parent = world.spawn(EntityData {
        name: Some("Parent".into()),
        ..EntityData::default()
    });
    let child = world.spawn(EntityData::default());
    (world, parent, child)
}

/// The layer an entity is on, as bits, because every assertion about it
/// here is that it is exactly where it was or exactly where it was put.
pub(super) fn layer_bits(world: &World, entity: EntityId) -> u32 {
    position(world, entity)[2].to_bits()
}

pub(super) fn position(world: &World, entity: EntityId) -> [f32; 3] {
    world
        .get(entity)
        .and_then(|data| data.transform_3d)
        .expect("entity has a 3D transform")
        .position
}

pub(super) fn edit(label: impl Into<String>, commands: Vec<WorldCommand>) -> Transaction {
    let mut buffer = CommandBuffer::new();
    for command in commands {
        buffer.push(command);
    }
    buffer.into_transaction(label)
}
