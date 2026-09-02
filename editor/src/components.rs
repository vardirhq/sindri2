//! What the editor knows about a component type that the registry does not.
//!
//! The registry answers what a component *is* — its fields, its blank, what to
//! call it. Two things it does not answer are presentational and belong to the
//! tool: which glyph a component is drawn with, and which family it is listed
//! under when someone goes looking for it.
//!
//! One table for both, rather than a match per question. They are the same
//! per-type fact asked twice, and kept apart they drift: the icon table already
//! had no entry for audio, rigid bodies or colliders, so three components drew
//! with the generic entity box and nobody noticed. A test asserts every
//! registered type appears here, so the next component added to the engine
//! fails the build rather than quietly arriving unfamilied and unglyphed.

use egui_material_icons::MaterialIcon;

use crate::ui::icons;

/// The family a component is listed under.
///
/// Chosen by what an author is looking for rather than by the type name.
/// Splitting `sindri.grid.occupant` on its dots is tempting and wrong: the
/// namespace is a naming scheme, not a taxonomy, and it produces two one-item
/// submenus and five components with no family at all.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Family {
    /// What puts something on the screen.
    Rendering,
    /// What puts something on the viewport rather than in the world.
    Ui,
    /// What makes something move by itself.
    Physics,
    /// What ties something to a grid.
    Grid,
    /// What an entity does, and what it answers to.
    ///
    /// Gameplay rather than presentation: the script that drives it, the clip
    /// it plays, the sound it makes, and the tags a script finds it by. A tag
    /// is not a behaviour, but it exists for the same reader — nothing else
    /// asks what an entity is tagged.
    Behaviour,
}

impl Family {
    /// Every family, in the order a menu lists them.
    ///
    /// Rendering first because it is what an entity usually needs before
    /// anything else is worth adding, and Behaviour last because it is what you
    /// reach for once the thing exists and you want it to do something.
    pub const ALL: [Self; 5] = [
        Self::Rendering,
        Self::Ui,
        Self::Physics,
        Self::Grid,
        Self::Behaviour,
    ];

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Rendering => "Rendering",
            Self::Ui => "UI",
            Self::Physics => "Physics",
            Self::Grid => "Grid",
            Self::Behaviour => "Behaviour",
        }
    }
}

/// One component type as the editor presents it.
struct Known {
    type_name: &'static str,
    family: Family,
    icon: MaterialIcon,
}

/// Every component the editor has something to say about.
///
/// Adding a component to the engine means adding one row here. Forgetting is a
/// failing test rather than a menu entry that quietly went missing.
const KNOWN: &[Known] = &[
    Known {
        type_name: "sindri.camera",
        family: Family::Rendering,
        icon: icons::CAMERA,
    },
    Known {
        type_name: "sindri.mesh",
        family: Family::Rendering,
        icon: icons::MESH,
    },
    Known {
        type_name: "sindri.sprite",
        family: Family::Rendering,
        icon: icons::SPRITE,
    },
    Known {
        type_name: "sindri.tilemap",
        family: Family::Rendering,
        icon: icons::TILEMAP,
    },
    Known {
        type_name: "sindri.ui.image",
        family: Family::Ui,
        icon: icons::UI_ELEMENT,
    },
    Known {
        type_name: "sindri.ui.text",
        family: Family::Ui,
        icon: icons::TEXT,
    },
    Known {
        type_name: "sindri.ui.button",
        family: Family::Ui,
        icon: icons::BUTTON,
    },
    Known {
        type_name: "sindri.ui.layout",
        family: Family::Ui,
        icon: icons::LAYOUT,
    },
    Known {
        type_name: "sindri.physics2d.rigid_body",
        family: Family::Physics,
        icon: icons::PHYSICS,
    },
    Known {
        type_name: "sindri.physics2d.collider",
        family: Family::Physics,
        icon: icons::COLLIDER,
    },
    Known {
        type_name: "sindri.grid.navigation",
        family: Family::Grid,
        icon: icons::GRID,
    },
    Known {
        type_name: "sindri.grid.occupant",
        family: Family::Grid,
        icon: icons::GRID,
    },
    Known {
        type_name: "sindri.script",
        family: Family::Behaviour,
        icon: icons::SCRIPT,
    },
    Known {
        type_name: "sindri.animation.sprite",
        family: Family::Behaviour,
        icon: icons::ANIMATION,
    },
    Known {
        type_name: "sindri.audio.source",
        family: Family::Behaviour,
        icon: icons::AUDIO,
    },
    Known {
        type_name: "sindri.tags",
        family: Family::Behaviour,
        icon: icons::LABEL,
    },
];

fn known(type_name: &str) -> Option<&'static Known> {
    KNOWN.iter().find(|known| known.type_name == type_name)
}

/// Which family a component is listed under, or `None` for one the editor has
/// never heard of.
///
/// `None` is reachable in principle — a scene may carry a component from a
/// newer tool, and the format exists to keep it — though not from the Add
/// Component menu, which only offers what is registered. A menu puts an
/// unfamilied component at its top level rather than inventing a family called
/// "Other" to hide it in.
#[must_use]
pub fn family(type_name: &str) -> Option<Family> {
    known(type_name).map(|known| known.family)
}

/// The glyph a component is drawn with, wherever it is named.
///
/// Total, unlike [`family`]: an unknown component still gets a section header
/// in the inspector, and a header needs an icon.
#[must_use]
pub fn icon(type_name: &str) -> MaterialIcon {
    known(type_name).map_or(icons::ENTITY, |known| known.icon)
}

#[cfg(test)]
mod tests;
