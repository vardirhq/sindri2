//! The edit in progress, and the checked commands it becomes.
//!
//! Nothing here writes the world. A panel edits a draft or a payload, and
//! what actually changed turns into commands on the way out, so an edit
//! undoes in one step and one the schema refuses is refused rather than
//! written.

use std::collections::BTreeMap;

use serde_json::Value;
use sindri_core::{
    CommandBuffer, ComponentMetadata, ComponentSchemaRegistry, EntityData, EntityId, SceneEntityId,
    Transform3D, World, WorldCommand,
};

use crate::{
    animation::{self},
    space::{self, declared_space},
    tilemap::{self},
};

use super::super::hierarchy::row::entity_name;
use super::super::{
    AUDIO_SOURCE_COMPONENT, CAMERA_COMPONENT, GRID_NAVIGATION_COMPONENT, GRID_OCCUPANT_COMPONENT,
    SCRIPT_COMPONENT, UI_TEXT_COMPONENT,
};

/// The inspector's editable copy of an entity.
///
/// Widgets write here rather than into the world, so every change can be
/// turned into a command instead of a silent mutation.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct EntityDraft {
    pub(crate) name: String,
    /// The identity the scene file keys this entity under.
    ///
    /// Separate from the name, and not a cosmetic difference: this is what a
    /// parent link names, what sibling order is derived from, and what
    /// `sindri.grid.occupant` points at. It was invisible, so the editor made
    /// `game-object-1` and a shipped scene's `player`, `floor` and `orb-1`
    /// could not be reproduced.
    pub(crate) source_id: String,
    pub(crate) transform_3d: Option<Transform3D>,
}

impl From<&EntityData> for EntityDraft {
    fn from(data: &EntityData) -> Self {
        Self {
            name: entity_name(data),
            source_id: data
                .source_id
                .as_ref()
                .map(|id| id.as_str().to_owned())
                .unwrap_or_default(),
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

/// What the rest of the scene already holds.
///
/// Not everything about a component is decided by the entity it would go on. A
/// world camera is a fact about the *scene*: the extract draws the player's
/// view through exactly one and refuses a scene with two, so whether Camera can
/// be added here depends on what some other entity is carrying.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct SceneHolds {
    pub(crate) world_camera: bool,
}

/// One component type as Add Component presents it.
pub(crate) struct Offer {
    pub(crate) metadata: ComponentMetadata,
    /// Why this cannot be added yet, or `None` when it can.
    ///
    /// Listed and disabled rather than absent. The rule used to be that a type
    /// nothing could build was simply missing, on the grounds that a button
    /// which adds a component the engine then rejects is worse than no button —
    /// which is right, and is not what this is. An entry that says *why* is
    /// neither of those: Sprite Animation needs a sliced sheet, Grid Occupant
    /// needs a grid, UI Text needs a font, and an author who cannot see that is
    /// left guessing why the menu is shorter than the documentation.
    pub(crate) withheld: Option<&'static str>,
}

/// Every component this entity could carry, and whether it can carry it yet.
///
/// An entity is in the world or on the viewport, and what it already carries
/// says which; the other family's components are left out entirely rather than
/// listed and refused, because offering them would let one entity be drawn
/// twice, in two spaces, from one transform — and a menu twice as long saying
/// "no" to half of itself explains nothing. An entity carrying nothing yet is
/// offered both, and the first component added is what settles it.
///
/// A type with neither a default payload nor a field template is left out for
/// the same reason: there is nothing to say about it and nothing to write.
pub(crate) fn addable_components(
    components: &ComponentSchemaRegistry,
    present: &BTreeMap<String, Value>,
    project: ProjectDefaults<'_>,
    scene: SceneHolds,
) -> Vec<Offer> {
    let space = declared_space(present);
    components
        .registered_components()
        .filter(|metadata| !present.contains_key(&metadata.type_name))
        .filter(|metadata| space::accepts(space, &metadata.type_name))
        .filter(|metadata| {
            components.fields(&metadata.type_name).is_some()
                || components.default_payload(&metadata.type_name).is_some()
        })
        .map(|metadata| Offer {
            metadata: metadata.clone(),
            withheld: withheld(components, metadata, present, project, scene),
        })
        .collect()
}

/// Why a component is listed but cannot be added.
fn withheld(
    components: &ComponentSchemaRegistry,
    metadata: &ComponentMetadata,
    present: &BTreeMap<String, Value>,
    project: ProjectDefaults<'_>,
    scene: SceneHolds,
) -> Option<&'static str> {
    // First, because it is the one that breaks a scene rather than merely
    // failing to help. A second authored world camera is not tolerated: the
    // extract refuses the whole scene with `MultipleWorldCameras`, both
    // viewports go dark, and nothing says which two entities are the cameras.
    // Offering Camera here was a button that broke the scene in one click.
    if metadata.type_name == CAMERA_COMPONENT && scene.world_camera {
        return Some("This scene already has a world camera, and a second one stops it opening");
    }
    if metadata.type_name == GRID_NAVIGATION_COMPONENT && !present.contains_key(tilemap::TYPE_NAME)
    {
        return Some("Add a Tilemap to this entity first: navigation is over its grid");
    }
    if component_default(components, &metadata.type_name, project).is_some() {
        return None;
    }
    // What is missing is whatever the project could not supply, and each of
    // these names the one thing to go and make.
    Some(match metadata.type_name.as_str() {
        UI_TEXT_COMPONENT => "No font in the project beside this scene",
        AUDIO_SOURCE_COMPONENT => "No audio clip in the project beside this scene",
        GRID_OCCUPANT_COMPONENT => "Nothing in this scene has a grid to occupy",
        SCRIPT_COMPONENT => "No .decay script in the project declares a container to run",
        animation::TYPE_NAME => "Slice an image into sprites first: a clip is made of them",
        _ => "The editor has no blank to start this component from",
    })
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

/// Why a stable ID cannot be used.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum IdentityRefusal {
    Empty,
    Taken,
}

impl IdentityRefusal {
    pub(crate) const fn reason(self) -> &'static str {
        match self {
            Self::Empty => "Every entity needs a stable ID: it is what the file keys this one by",
            Self::Taken => "Another entity already has this ID, and two cannot share one",
        }
    }
}

/// The commands that give an entity a new stable ID, keeping the scene
/// pointing at it.
///
/// A stable ID is a reference, not a label. `sindri.grid.occupant` names the
/// grid it stands on by one, so renaming a grid without rewriting its occupants
/// would leave every piece pointing at an entity that no longer exists — the
/// scene would still open, and nothing would be on the board. One buffer, so
/// the rename and the re-pointing are one undo step.
///
/// `Ok` with an empty buffer means there was nothing to change.
pub(crate) fn identity_commands(
    world: &World,
    entity: EntityId,
    wanted: &str,
) -> Result<CommandBuffer, IdentityRefusal> {
    let mut buffer = CommandBuffer::new();
    let wanted = wanted.trim();
    let Some(data) = world.get(entity) else {
        return Ok(buffer);
    };
    let current = data.source_id.as_ref().map(SceneEntityId::as_str);
    if current == Some(wanted) {
        return Ok(buffer);
    }
    let Ok(id) = SceneEntityId::new(wanted) else {
        return Err(IdentityRefusal::Empty);
    };
    if world
        .entities()
        .any(|(other, data)| other != entity && data.source_id.as_ref() == Some(&id))
    {
        return Err(IdentityRefusal::Taken);
    }
    buffer.push(WorldCommand::SetSourceId {
        entity,
        source_id: Some(id.clone()),
    });
    for (occupant, payload) in occupants_of(world, current) {
        let mut payload = payload;
        payload["grid"] = Value::from(id.as_str());
        buffer.push(WorldCommand::SetComponent {
            entity: occupant,
            type_name: GRID_OCCUPANT_COMPONENT.to_owned(),
            payload,
        });
    }
    Ok(buffer)
}

/// Every occupant whose grid is the entity being renamed.
///
/// Read from the stored payload rather than the decoded component, because the
/// payload is what gets written back: an occupant carrying a field the editor
/// has never heard of keeps it through the rename.
fn occupants_of(world: &World, grid: Option<&str>) -> Vec<(EntityId, Value)> {
    let Some(grid) = grid else {
        return Vec::new();
    };
    world
        .entities()
        .filter_map(|(entity, data)| {
            let payload = data.components.get(GRID_OCCUPANT_COMPONENT)?;
            (payload.get("grid").and_then(Value::as_str) == Some(grid))
                .then(|| (entity, payload.clone()))
        })
        .collect()
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
