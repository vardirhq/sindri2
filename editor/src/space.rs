//! Which of a scene's two spaces an entity belongs to.
//!
//! A scene holds two kinds of thing. A world object is placed by its transform
//! and seen through a camera; a UI object is anchored to the viewport and no
//! camera can move it. The engine says which by the components an entity
//! carries — `sindri.ui.*` is the UI family — and this is where the editor asks
//! that question, so the hierarchy, the icons, and the Add Component menu all
//! get the same answer.
//!
//! Only a component that *places* something has an opinion. A script, a clip,
//! a rigid body, a footprint and a set of clips say what an entity does, not
//! where it is, and an entity carrying only those has still chosen neither.
//!
//! Nothing here is stored. Deriving the space from the components means it
//! cannot disagree with what actually draws, and an entity cannot be marked as
//! UI while holding a component that puts it in the world.

use std::collections::BTreeMap;

use serde_json::Value;
use sindri_core::{EntityData, EntityId, World};

/// The prefix every UI component's type name starts with.
pub const UI_NAMESPACE: &str = "sindri.ui.";

/// The components that put the entity carrying them *in the world*.
///
/// Named, rather than taken to be everything that is not UI. A script, a clip,
/// a rigid body, a grid occupant and a set of animation clips say nothing about
/// which space their entity is in — Gather's banner and pips are UI images
/// driven by scripts, and its player is a world sprite driven by one. Treating
/// any non-UI component as a world component meant adding a script to a fresh
/// entity silently decided it was a world object, after which the menu would
/// not offer UI Text at all: the editor could not have built Gather's HUD.
///
/// These four are the ones that place something: three that draw through a
/// camera, and the camera itself.
pub const WORLD_COMPONENTS: [&str; 4] = [
    "sindri.camera",
    "sindri.mesh",
    "sindri.sprite",
    "sindri.tilemap",
];

/// The space one entity is in.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EntitySpace {
    World,
    Ui,
}

impl EntitySpace {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::World => "World",
            Self::Ui => "UI",
        }
    }
}

/// Whether a component type puts the entity carrying it on the viewport.
#[must_use]
pub fn is_ui_component(type_name: &str) -> bool {
    type_name.starts_with(UI_NAMESPACE)
}

/// Whether a component type puts the entity carrying it in the world.
#[must_use]
pub fn is_world_component(type_name: &str) -> bool {
    WORLD_COMPONENTS.contains(&type_name)
}

/// The space an entity's own components put it in, if they say anything.
///
/// An entity with no components — a fresh `GameObject`, a node used only to
/// group others — says nothing, which is `None` rather than a guess. So does
/// one carrying only components that are about behaviour rather than about
/// where a thing is: a script decides nothing, and an entity holding one is
/// still free to become either.
#[must_use]
pub fn declared_space(components: &BTreeMap<String, Value>) -> Option<EntitySpace> {
    if components
        .keys()
        .any(|type_name| is_ui_component(type_name))
    {
        return Some(EntitySpace::Ui);
    }
    components
        .keys()
        .any(|type_name| is_world_component(type_name))
        .then_some(EntitySpace::World)
}

/// The space an entity is treated as being in, including what it holds.
///
/// An entity that says nothing itself takes the answer from what is under it,
/// so a node grouping five HUD elements sits with the HUD rather than in the
/// world. Descendants that disagree fall back to the world, because a group
/// holding both is not a UI object.
#[must_use]
pub fn space_of(world: &World, entity: EntityId) -> EntitySpace {
    let Some(data) = world.get(entity) else {
        return EntitySpace::World;
    };
    if let Some(space) = declared_space(&data.components) {
        return space;
    }
    let mut found: Option<EntitySpace> = None;
    for child in &data.children {
        let child_space = space_of(world, *child);
        match found {
            Some(space) if space != child_space => return EntitySpace::World,
            _ => found = Some(child_space),
        }
    }
    found.unwrap_or(EntitySpace::World)
}

/// The space an entity's data alone puts it in, for a row that has no world.
#[must_use]
pub fn space_of_data(data: &EntityData) -> EntitySpace {
    declared_space(&data.components).unwrap_or(EntitySpace::World)
}

/// Whether an entity in `space` may be given `type_name`.
///
/// The two families are mutually exclusive: a thing is either in the world or
/// on the viewport, and one entity holding a world sprite and a UI image would
/// be drawn twice, in two places, from one transform. Offering the component
/// and then drawing that is worse than not offering it.
#[must_use]
pub fn accepts(space: Option<EntitySpace>, type_name: &str) -> bool {
    match space {
        None => true,
        Some(EntitySpace::Ui) => is_ui_component(type_name),
        Some(EntitySpace::World) => !is_ui_component(type_name),
    }
}

#[cfg(test)]
mod tests {
    use super::{EntitySpace, accepts, declared_space, space_of};
    use serde_json::json;
    use sindri_core::{EntityData, SceneEntityId, World};

    fn components(names: &[&str]) -> std::collections::BTreeMap<String, serde_json::Value> {
        names
            .iter()
            .map(|name| ((*name).to_owned(), json!({})))
            .collect()
    }

    #[test]
    fn a_ui_component_puts_its_entity_on_the_viewport() {
        assert_eq!(
            declared_space(&components(&["sindri.ui.image"])),
            Some(EntitySpace::Ui)
        );
        assert_eq!(
            declared_space(&components(&["sindri.ui.text"])),
            Some(EntitySpace::Ui)
        );
        assert_eq!(
            declared_space(&components(&["sindri.sprite"])),
            Some(EntitySpace::World)
        );
    }

    /// An entity with nothing on it is not in either space yet, and saying so
    /// is what lets the Add Component menu offer both.
    #[test]
    fn an_empty_entity_has_not_chosen() {
        assert_eq!(declared_space(&components(&[])), None);
        assert!(accepts(None, "sindri.ui.image"));
        assert!(accepts(None, "sindri.sprite"));
    }

    /// A script says what an entity *does*, not where it is. Gather's banner
    /// and pips are UI images driven by scripts; treating a script as a world
    /// component meant a fresh entity given one could never be given UI Text,
    /// so the editor could not have built that HUD.
    #[test]
    fn a_component_about_behaviour_decides_no_space() {
        for neutral in [
            "sindri.script",
            "sindri.audio.source",
            "sindri.animation.sprite",
            "sindri.grid.occupant",
            "sindri.physics.rigid_body_2d",
        ] {
            assert_eq!(
                declared_space(&components(&[neutral])),
                None,
                "{neutral} says nothing about which space its entity is in"
            );
            assert!(accepts(
                declared_space(&components(&[neutral])),
                "sindri.ui.text"
            ));
            assert!(accepts(
                declared_space(&components(&[neutral])),
                "sindri.sprite"
            ));
        }
        assert_eq!(
            declared_space(&components(&["sindri.script", "sindri.ui.image"])),
            Some(EntitySpace::Ui),
            "and it goes wherever the component that does place it says"
        );
        assert_eq!(
            declared_space(&components(&["sindri.script", "sindri.sprite"])),
            Some(EntitySpace::World)
        );
    }

    #[test]
    fn the_two_families_are_mutually_exclusive() {
        assert!(!accepts(Some(EntitySpace::World), "sindri.ui.image"));
        assert!(accepts(Some(EntitySpace::World), "sindri.sprite"));
        assert!(!accepts(Some(EntitySpace::Ui), "sindri.sprite"));
        assert!(accepts(Some(EntitySpace::Ui), "sindri.ui.text"));
    }

    /// A node that holds nothing but HUD elements belongs with the HUD, which
    /// is what makes grouping UI under one parent possible at all.
    #[test]
    fn a_group_takes_the_space_of_what_is_under_it() {
        let mut world = World::default();
        let parent = world.spawn(EntityData {
            source_id: Some(SceneEntityId::new("hud").unwrap()),
            ..EntityData::default()
        });
        let child = world.spawn(EntityData {
            components: components(&["sindri.ui.image"]),
            ..EntityData::default()
        });
        world.set_parent(child, Some(parent)).unwrap();
        assert_eq!(space_of(&world, parent), EntitySpace::Ui);

        let mixed = world.spawn(EntityData {
            components: components(&["sindri.sprite"]),
            ..EntityData::default()
        });
        world.set_parent(mixed, Some(parent)).unwrap();
        assert_eq!(
            space_of(&world, parent),
            EntitySpace::World,
            "a group holding both is not a UI object"
        );
    }
}
