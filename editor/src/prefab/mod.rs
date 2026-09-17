//! Putting an authored prefab into the open scene.
//!
//! The runtime already spawns prefabs, and deliberately does it differently:
//! `World::spawn_prefab` gives its entities no stable identity, because a
//! prefab's identities name entities *inside the prefab* and two instances
//! would collide on every one of them. That is right for a bullet that exists
//! for a second and is never saved.
//!
//! An editor is authoring a document. What it instantiates has to be saveable,
//! so every entity needs a stable identity that nothing else in the scene is
//! using, and the whole thing has to arrive as one undoable step. So this is
//! not a wrapper around the runtime's spawn; it is the same idea answered for
//! a file rather than a frame.
//!
//! The handle problem, and its answer, are `duplicate.rs`'s: `WorldCommand::
//! Spawn` names the handle it spawns at, and `World::next_handle` is a peek
//! rather than an allocation, so the instantiation is rehearsed into a clone of
//! the world that hands out the handles the real one is about to.

use std::path::{Path, PathBuf};

use sindri_core::{
    CommandBuffer, EntityData, EntityId, PREFAB_SUFFIX, PrefabDocument, SceneEntity, SceneEntityId,
    World, WorldCommand,
};

/// What a placed prefab carries to say where it stands.
pub const PLACEMENT_COMPONENT: &str = "sindri.grid.placement";

/// Whether the browser is pointing at a prefab.
#[must_use]
pub fn is_a_prefab(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.ends_with(PREFAB_SUFFIX))
}

/// Reads a prefab from disk, or says why it cannot be used.
///
/// Validated here rather than at instantiation, so a prefab with two roots is
/// refused while it is still an asset somebody chose and not a half-built
/// subtree in their scene.
pub fn load(path: &Path) -> Result<PrefabDocument, String> {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("this file");
    let json = std::fs::read_to_string(path)
        .map_err(|error| format!("{name} could not be read: {error}"))?;
    let document = PrefabDocument::from_json(&json)
        .map_err(|error| format!("{name} is not valid: {error}"))?;
    document
        .validate()
        .map_err(|error| format!("{name} cannot be instantiated: {error}"))?;
    Ok(document)
}

/// The prefab the browser is pointing at, and whether it is being placed.
///
/// Held the way the image slicer is held: selecting an asset and selecting an
/// entity are the same act from the author's side, so a chosen prefab is scene
/// view state rather than a mode somebody has to leave.
pub struct PrefabBrush {
    path: PathBuf,
    document: Result<PrefabDocument, String>,
    /// Whether a click on a cell puts it there.
    ///
    /// Off until asked for. Choosing a prefab to look at is not the same as
    /// arming the scene view to build with it, and a browser click that
    /// silently armed one would put a house wherever the next click landed.
    pub placing: bool,
}

impl PrefabBrush {
    #[must_use]
    pub fn open(path: &Path) -> Self {
        Self {
            path: path.to_owned(),
            document: load(path),
            placing: false,
        }
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// What to call it: the file's stem, without the doubled extension.
    #[must_use]
    pub fn name(&self) -> String {
        self.path
            .file_name()
            .and_then(|name| name.to_str())
            .map_or_else(
                || "Prefab".to_owned(),
                |name| name.trim_end_matches(PREFAB_SUFFIX).to_owned(),
            )
    }

    #[must_use]
    pub fn document(&self) -> Option<&PrefabDocument> {
        self.document.as_ref().ok()
    }

    /// Why it cannot be placed, when it cannot.
    #[must_use]
    pub fn problem(&self) -> Option<&str> {
        self.document.as_ref().err().map(String::as_str)
    }

    /// How many entities one instance would add.
    #[must_use]
    pub fn entities(&self) -> usize {
        self.document()
            .map_or(0, |document| document.entities.len())
    }
}

/// Adds the commands that put `prefab` into the scene under `parent`, and
/// answers with the handle its root will land on.
///
/// `rehearsal` is a clone of the world that receives exactly the spawns the
/// real one is about to, so it hands out the handles the real one will and
/// knows which stable IDs are taken as they are taken. Taken rather than made
/// here for the reason duplication takes one: instantiating twice in a single
/// transaction would otherwise pick the same handle and the same ID twice, and
/// spawn the second thing on top of the first.
pub fn instantiate_into(
    rehearsal: &mut World,
    prefab: &PrefabDocument,
    parent: Option<EntityId>,
    buffer: &mut CommandBuffer,
) -> Result<EntityId, String> {
    let root = prefab
        .root()
        .map_err(|error| format!("this prefab has no single root: {error}"))?;
    Ok(spawn_into(rehearsal, prefab, root, parent, buffer))
}

/// Puts `prefab` into the scene standing on one cell of a grid.
///
/// The click is the authored fact, so the cell it names replaces whatever the
/// prefab said about where it stands -- but not what shape it is. A prefab that
/// declares a three-by-four footprint is a three-by-four thing wherever it is
/// put, and that survives; only the anchor and the grid are answered by where
/// somebody clicked.
///
/// Placement itself refuses a cell that holds nothing up, so this does not
/// check: `resolve_grid_placements` is the one place that question is asked,
/// and asking it twice is how two answers start disagreeing.
pub fn instantiate_on_cell(
    rehearsal: &mut World,
    prefab: &PrefabDocument,
    grid: &SceneEntityId,
    cell: [i32; 2],
    buffer: &mut CommandBuffer,
) -> Result<EntityId, String> {
    let footprint = authored_footprint(prefab);
    let root = instantiate_into(rehearsal, prefab, None, buffer)?;
    buffer.push(WorldCommand::SetComponent {
        entity: root,
        type_name: PLACEMENT_COMPONENT.to_owned(),
        payload: serde_json::json!({
            "grid": grid.as_str(),
            "cell": cell,
            "offset": [0.0, 0.0],
            "footprint": footprint,
        }),
    });
    Ok(root)
}

/// The shape the prefab's root says it is, or one cell.
///
/// Read from the prefab rather than assumed, because what a thing covers is a
/// property of the thing and not of the click that placed it.
fn authored_footprint(prefab: &PrefabDocument) -> Vec<[i32; 2]> {
    prefab
        .root()
        .ok()
        .and_then(|root| root.components.get(PLACEMENT_COMPONENT))
        .and_then(|payload| payload.get("footprint"))
        .and_then(|value| serde_json::from_value::<Vec<[i32; 2]>>(value.clone()).ok())
        .filter(|cells| !cells.is_empty())
        .unwrap_or_else(|| vec![[0, 0]])
}

/// Spawns one authored entity and then everything under it, parents first.
///
/// Parents first so a child can name the handle its parent was given. The
/// prefab's own parent links are authored identities, which mean nothing in
/// this world; what a child is actually parented to is whatever its parent
/// just became.
fn spawn_into(
    rehearsal: &mut World,
    prefab: &PrefabDocument,
    entity: &SceneEntity,
    parent: Option<EntityId>,
    buffer: &mut CommandBuffer,
) -> EntityId {
    let data = EntityData {
        source_id: Some(unused_id(rehearsal, &entity.id)),
        name: entity.name.clone(),
        parent,
        // Rebuilt by the recursion below. Taking the authored list would name
        // identities that are the prefab's rather than entities in this world.
        children: Vec::new(),
        transform_3d: entity.transform_3d,
        components: entity.components.clone(),
        // A prefab that authored something switched off meant it.
        disabled: entity.disabled,
        // Editor state describes the prefab as a document -- what was folded
        // open while somebody edited it -- and says nothing about an instance.
        editor: std::collections::BTreeMap::new(),
    };
    let handle = rehearsal.spawn(data.clone());
    rehearsal
        .set_parent(handle, parent)
        .expect("a fresh entity accepts the parent it was given");
    buffer.push(WorldCommand::Spawn {
        entity: handle,
        data: Box::new(data),
    });
    for child in prefab.children_of(&entity.id) {
        spawn_into(rehearsal, prefab, child, Some(handle), buffer);
    }
    handle
}

/// A stable ID like the prefab's that nothing in the scene is using.
///
/// Derived from the authored one rather than generated, because `oak-tree`
/// says what it is and `game-object-7` does not. Instantiating the same prefab
/// twice is the ordinary case, so the collision path is the common one rather
/// than the exception.
fn unused_id(world: &World, authored: &SceneEntityId) -> SceneEntityId {
    let stem = authored.as_str();
    if !taken(world, stem) {
        return authored.clone();
    }
    let mut suffix = 2_u32;
    loop {
        let candidate = format!("{stem}-{suffix}");
        if !taken(world, &candidate) {
            return SceneEntityId::new(candidate).expect("a derived ID is never empty");
        }
        suffix += 1;
    }
}

fn taken(world: &World, candidate: &str) -> bool {
    world.entities().any(|(_, data)| {
        data.source_id
            .as_ref()
            .is_some_and(|id| id.as_str() == candidate)
    })
}

#[cfg(test)]
mod tests;
