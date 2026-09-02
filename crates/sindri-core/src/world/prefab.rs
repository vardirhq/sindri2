//! Creating entities in a world from an authored prefab.

use std::collections::HashMap;

use crate::{EntityId, PrefabDocument, PrefabError, SceneEntityId};

use super::{EntityData, World, WorldError};

/// What one spawn produced.
#[derive(Clone, Debug)]
pub struct SpawnedPrefab {
    /// The prefab's single root, and what a caller holds on to.
    pub root: EntityId,
    /// Every entity the spawn created, including the root, in document order.
    ///
    /// A caller undoing a spawn needs all of them, and a caller reaching into
    /// a named child needs to be able to find it without walking the world.
    pub entities: Vec<EntityId>,
    /// Which runtime entity each authored identity became.
    ///
    /// The prefab's identities are the *prefab's*, not the world's: spawning
    /// the same prefab twice makes two entities that shared an authored name
    /// and share nothing else.
    pub by_source_id: HashMap<SceneEntityId, EntityId>,
}

impl World {
    /// Creates the prefab's entities and returns what they became.
    ///
    /// Spawned entities carry **no** `source_id`. A prefab's identities name
    /// entities inside the prefab and are not stable identities in this world:
    /// two instances would collide on every one of them, and a scene saved
    /// with the collision would refuse to load. `assign_missing_source_ids`
    /// remains how a runtime entity earns a stable identity, which is a
    /// decision about persisting a world rather than about spawning.
    ///
    /// The prefab is validated first, so a document with several roots is
    /// refused rather than half-spawned. Nothing reaches the world until the
    /// whole document has been checked.
    pub fn spawn_prefab(&mut self, prefab: &PrefabDocument) -> Result<SpawnedPrefab, WorldError> {
        prefab.validate()?;
        let root_id = prefab.root()?.id.clone();

        let mut by_source_id = HashMap::with_capacity(prefab.entities.len());
        let mut entities = Vec::with_capacity(prefab.entities.len());
        for entity in &prefab.entities {
            let runtime = self.spawn(EntityData {
                source_id: None,
                name: entity.name.clone(),
                transform_3d: entity.transform_3d,
                components: entity.components.clone(),
                disabled: entity.disabled,
                // Editor-only state describes the prefab in the editor, not the
                // instance in a running world. Carrying it would put a
                // selection highlight and a fold state on every bullet.
                editor: std::collections::BTreeMap::new(),
                ..EntityData::default()
            });
            by_source_id.insert(entity.id.clone(), runtime);
            entities.push(runtime);
        }

        for entity in &prefab.entities {
            if let Some(parent) = &entity.parent {
                self.set_parent(by_source_id[&entity.id], Some(by_source_id[parent]))?;
            }
        }

        Ok(SpawnedPrefab {
            root: by_source_id[&root_id],
            entities,
            by_source_id,
        })
    }
}

impl From<PrefabError> for WorldError {
    fn from(error: PrefabError) -> Self {
        Self::InvalidPrefab(error)
    }
}
