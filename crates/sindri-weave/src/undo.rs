//! Styling the live world for one draw, and putting it back.
//!
//! Presenting into a copy of the world is simple and, for a world holding a
//! level's worth of tiles, a whole copy of that level every frame: measured at
//! ten milliseconds natively for Causeway, which has seventeen entities and
//! one large tilemap. So a running game is styled where it stands instead.
//! Before anything is written, what styling could change is saved — each
//! element's transform, and the UI components of each element a rule
//! matched — and [`Undo`] puts exactly that back. Nothing Weave never touches
//! is copied, and gameplay never sees a styled value, because the host undoes
//! the styling before anything else runs.

use std::collections::BTreeMap;

use sindri_core::{EntityId, Transform3D, World};

/// The components styling writes to. Anything else on an element is left
/// alone by Weave, and so is not saved.
const WRITTEN: [&str; 6] = [
    "sindri.ui.shape",
    "sindri.ui.text",
    "sindri.ui.image",
    "sindri.ui.layout",
    "sindri.ui.box",
    "weave.style",
];

#[derive(Debug)]
struct Saved {
    transform: Option<Transform3D>,
    /// Each written component as it was, `None` where it was absent.
    components: Option<Vec<(&'static str, Option<serde_json::Value>)>>,
}

/// What styling changed, to be put back with [`Undo::undo`].
#[derive(Debug, Default)]
#[must_use = "a styled world must be put back"]
pub struct Undo {
    saved: BTreeMap<EntityId, Saved>,
}

impl Undo {
    /// Remembers what styling `entity` could change: always its transform,
    /// which sizing settles for every element, and its UI components when a
    /// rule matched it and may write them.
    pub(crate) fn save(&mut self, world: &World, entity: EntityId, components: bool) {
        let Some(data) = world.get(entity) else {
            return;
        };
        let saved = self.saved.entry(entity).or_insert_with(|| Saved {
            transform: data.transform_3d,
            components: None,
        });
        if components && saved.components.is_none() {
            saved.components = Some(
                WRITTEN
                    .iter()
                    .map(|name| (*name, data.components.get(*name).cloned()))
                    .collect(),
            );
        }
    }

    /// Puts back everything styling changed.
    pub fn undo(self, world: &mut World) {
        for (entity, saved) in self.saved {
            let Some(data) = world.get_mut(entity) else {
                continue;
            };
            data.transform_3d = saved.transform;
            for (name, value) in saved.components.into_iter().flatten() {
                match value {
                    Some(value) => {
                        data.components.insert(name.to_owned(), value);
                    }
                    None => {
                        data.components.remove(name);
                    }
                }
            }
        }
    }
}
