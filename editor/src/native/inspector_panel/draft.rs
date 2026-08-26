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
use super::super::{
    AUDIO_SOURCE_COMPONENT, GRID_NAVIGATION_COMPONENT, GRID_OCCUPANT_COMPONENT, SCRIPT_COMPONENT,
    UI_TEXT_COMPONENT,
};

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

/// What the open project can complete a component with.
///
/// Five components cannot be created out of nothing: they name a font, a
/// sheet, another entity, a clip, or a script source, and the engine has no
/// honest guess for any of them. What it does have is a project sitting beside
/// the scene, so the editor completes the registry's field template with the
/// first thing of the right kind the project holds. This is the whole of that
/// list, gathered rather than passed one argument at a time, because it only
/// grows.
#[derive(Clone, Copy, Default)]
pub(crate) struct ProjectDefaults<'a> {
    pub(crate) font: Option<&'a str>,
    pub(crate) sprite: Option<&'a str>,
    pub(crate) grid: Option<&'a str>,
    pub(crate) audio: Option<&'a str>,
    /// A `.decay` source and one of the script containers it declares.
    ///
    /// Both or neither: a script component naming a source but no container
    /// loads and then runs nothing, reporting itself once a frame. Waiting for
    /// the source to declare something is the difference between offering a
    /// component that works and offering one that complains.
    pub(crate) script: Option<(&'a str, &'a str)>,
}

/// The components an entity does not have and the registry can create.
///
/// A type with no way to build a fresh payload is missing from the list rather
/// than offered and refused: a button that adds a component the engine will
/// then reject is worse than no button, which is why the old Add Component was
/// removed instead of left drawn.
///
/// The same rule decides the other filter. An entity is in the world or on the
/// viewport, and what it already carries says which; offering the other
/// family's components would let one entity be drawn twice, in two spaces, from
/// one transform. An entity carrying nothing yet is offered both, and the first
/// component added is what settles it.
pub(crate) fn addable_components(
    components: &ComponentSchemaRegistry,
    present: &BTreeMap<String, Value>,
    project: ProjectDefaults<'_>,
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
        .filter(|metadata| component_default(components, &metadata.type_name, project).is_some())
        .cloned()
        .collect()
}

/// What Add Component writes for a fresh component.
///
/// A type with an honest blank has one in the registry. The rest are the
/// registry's field template with the one thing only the project can supply
/// filled in — so a component added here has every field the same component
/// authored by hand has, rather than the two or three the editor happened to
/// write down.
pub(crate) fn component_default(
    components: &ComponentSchemaRegistry,
    type_name: &str,
    project: ProjectDefaults<'_>,
) -> Option<Value> {
    if let Some(default) = components.default_payload(type_name) {
        return Some(default.clone());
    }
    let template = components.fields(type_name)?;
    match type_name {
        GRID_OCCUPANT_COMPONENT => completed(template, [("grid", Value::from(project.grid?))]),
        UI_TEXT_COMPONENT => completed(template, [("font", Value::from(project.font?))]),
        AUDIO_SOURCE_COMPONENT => completed(template, [("clip", Value::from(project.audio?))]),
        SCRIPT_COMPONENT => {
            let (source, script) = project.script?;
            completed(
                template,
                [
                    ("source", Value::from(source)),
                    ("script", Value::from(script)),
                ],
            )
        }
        animation::TYPE_NAME => completed(
            template,
            [
                (
                    "clips",
                    serde_json::json!({
                        "clip": {
                            "frames": [project.sprite?],
                            "seconds_per_frame": 0.1,
                            "looping": true
                        }
                    }),
                ),
                ("playing", Value::from("clip")),
            ],
        ),
        _ => None,
    }
}

/// A field template with some of its fields answered.
///
/// `None` when the template is not an object, which the registry already
/// refuses — the check is here so this cannot be the thing that panics if it
/// ever stops refusing.
fn completed<'a>(
    template: &Value,
    edits: impl IntoIterator<Item = (&'a str, Value)>,
) -> Option<Value> {
    let mut payload = template.as_object()?.clone();
    for (key, value) in edits {
        payload.insert(key.to_owned(), value);
    }
    Some(Value::Object(payload))
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
