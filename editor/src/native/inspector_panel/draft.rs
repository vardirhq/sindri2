//! The edit in progress, and the checked commands it becomes.
//!
//! Nothing here writes the world. A panel edits a draft or a payload, and
//! what actually changed turns into commands on the way out, so an edit
//! undoes in one step and one the schema refuses is refused rather than
//! written.

use std::collections::BTreeMap;

use serde_json::Value;
use sindri_core::{
    CommandBuffer, ComponentMetadata, ComponentSchemaRegistry, EntityData, EntityId, Transform3D,
    WorldCommand,
};

use crate::{
    animation::{self},
    space::{self, declared_space},
    tilemap::{self},
};

use super::super::hierarchy::row::entity_name;
use super::super::{GRID_NAVIGATION_COMPONENT, GRID_OCCUPANT_COMPONENT, UI_TEXT_COMPONENT};

/// The inspector's editable copy of an entity.
///
/// Widgets write here rather than into the world, so every change can be
/// turned into a command instead of a silent mutation.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct EntityDraft {
    pub(crate) name: String,
    pub(crate) transform_3d: Option<Transform3D>,
}

impl From<&EntityData> for EntityDraft {
    fn from(data: &EntityData) -> Self {
        Self {
            name: entity_name(data),
            transform_3d: data.transform_3d,
        }
    }
}

/// Turns the difference between an entity's stored state and the drawn draft
/// into the commands that close the gap.
/// Turns every changed component payload into a command, and says what it
/// refused.
///
/// Kept apart from the drawing of it so the claims — that an edit becomes a
/// command, and that one which breaks a schema becomes nothing — are things a
/// test can check without a window or a GPU.
///
/// A payload is written back exactly as stored, so an edit that stopped it
/// decoding would produce a scene the engine refuses to open. Checking here
/// means the author hears about it at the field they were editing rather than
/// at the next launch.
pub(crate) fn component_commands(
    entity: EntityId,
    original: &BTreeMap<String, Value>,
    draft: &BTreeMap<String, Value>,
    components: &ComponentSchemaRegistry,
) -> (CommandBuffer, Vec<String>) {
    let mut buffer = CommandBuffer::new();
    let mut refused = Vec::new();
    for (type_name, payload) in draft {
        if original.get(type_name) == Some(payload) {
            continue;
        }
        if let Err(error) = components.validate_payload(type_name, payload) {
            refused.push(error.to_string());
            continue;
        }
        buffer.push(WorldCommand::SetComponent {
            entity,
            type_name: type_name.clone(),
            payload: payload.clone(),
        });
    }
    (buffer, refused)
}

/// The components an entity does not have and the registry can create.
///
/// A type with no default payload is missing from the list rather than offered
/// and refused: a button that adds a component the engine will then reject is
/// worse than no button, which is why the old Add Component was removed rather
/// than left drawn.
///
/// The same rule decides the other filter. An entity is in the world or on the
/// viewport, and what it already carries says which; offering the other
/// family's components would let one entity be drawn twice, in two spaces, from
/// one transform. An entity carrying nothing yet is offered both, and the first
/// component added is what settles it.
pub(crate) fn addable_components(
    components: &ComponentSchemaRegistry,
    present: &BTreeMap<String, Value>,
    first_font: Option<&str>,
    first_sprite: Option<&str>,
    first_grid: Option<&str>,
) -> Vec<ComponentMetadata> {
    let space = declared_space(present);
    components
        .registered_components()
        .filter(|metadata| !present.contains_key(&metadata.type_name))
        .filter(|metadata| space::accepts(space, &metadata.type_name))
        .filter(|metadata| {
            metadata.type_name != GRID_NAVIGATION_COMPONENT
                || present.contains_key(tilemap::TYPE_NAME)
        })
        .filter(|metadata| {
            component_default(
                components,
                &metadata.type_name,
                first_font,
                first_sprite,
                first_grid,
            )
            .is_some()
        })
        .cloned()
        .collect()
}

/// What Add Component writes for a fresh component.
///
/// Built-ins normally own a fixed default in the registry. UI text and sprite
/// animation cannot: their reproducible asset references must come from the
/// project, so their defaults are completed at the editor boundary.
pub(crate) fn component_default(
    components: &ComponentSchemaRegistry,
    type_name: &str,
    first_font: Option<&str>,
    first_sprite: Option<&str>,
    first_grid: Option<&str>,
) -> Option<Value> {
    if type_name == GRID_OCCUPANT_COMPONENT {
        return first_grid.map(|grid| {
            serde_json::json!({
                "grid": grid,
                "footprint": [[0, 0]]
            })
        });
    }
    if type_name == UI_TEXT_COMPONENT {
        return first_font.map(|font| {
            serde_json::json!({
                "text": "Text",
                "font": font
            })
        });
    }
    if type_name == animation::TYPE_NAME {
        return first_sprite.map(|sprite| {
            serde_json::json!({
                "clips": {
                    "clip": {
                        "frames": [sprite],
                        "seconds_per_frame": 0.1,
                        "looping": true
                    }
                },
                "playing": "clip",
                "speed": 1.0
            })
        });
    }
    components.default_payload(type_name).cloned()
}

pub(crate) fn draft_commands(
    entity: EntityId,
    original: &EntityDraft,
    draft: &EntityDraft,
) -> CommandBuffer {
    let mut buffer = CommandBuffer::new();
    if original.name != draft.name {
        buffer.push(WorldCommand::SetName {
            entity,
            name: Some(draft.name.clone()),
        });
    }
    if original.transform_3d != draft.transform_3d {
        buffer.push(WorldCommand::SetTransform3D {
            entity,
            transform: draft.transform_3d,
        });
    }
    buffer
}
