//! The one description of what a script can reach.
//!
//! Two things need this and they must never disagree. The **analyzer** needs
//! types, so `this.transfrom.position.x` is a compile error and an editor can
//! one day complete after a dot. The **host** needs accessors, so the same path
//! reaches a real transform at runtime. Written twice, they drift: a path the
//! analyzer accepts and the host does not is a clean compile followed by
//! `UnknownPath` at frame one, which is the worst of both.
//!
//! So the surface is a tree, written once, and both are derived from it.
//! `the_host_answers_every_path_the_analyzer_accepts` walks the description and
//! asserts the host answers every path in it, so the two cannot be shipped
//! disagreeing even if someone adds a third reader.
//!
//! Widening the surface means adding a node. That is the point: it should be
//! one edit, visible in a diff, and not something that can be half-done. A
//! member goes in `member`, a call goes in `call`, and a new host type gets its
//! name in `names` and a root here.

mod call;
mod member;
pub(super) mod names;

#[cfg(test)]
mod tests;

pub(crate) use call::{
    FUNCTIONS, GAME_CALLS, GRID_CALLS, GameCall, GridCall, HostFunction, INPUT_QUERIES, InputQuery,
    PHYSICS_CALLS, POINTER_QUERIES, POINTER_VALUES, PRINT, PhysicsCall, PointerQuery, PointerValue,
    TIME_VALUES, TOUCH_CALLS, TOUCH_COUNT, TimeValue, TouchCall, UI_CALLS, UiCall, WORLD_CALLS,
    WorldCall,
};
pub(crate) use member::{SPRITE_MEMBERS, TRANSFORM_MEMBERS, UI_IMAGE_MEMBERS};
pub(crate) use names::{
    ENTITY, GAME, GRID, INPUT, PHYSICS, POINTER, PREFAB, SPRITE, TILEMAP_COMPONENT, TIME, TOUCH,
    TRANSFORM, UI, UI_IMAGE, WORLD,
};

use serde_json::Value as Json;
use sindri_core::Transform3D;

/// A node in the surface: either a nested type, or a value a script reaches.
pub(crate) enum Node {
    /// A named type with members of its own.
    Group(&'static str, &'static [(&'static str, Node)]),
    /// A number a script can read and write.
    Leaf(Leaf),
    /// A reference to something the host owns.
    ///
    /// Read-only, and deliberately: a script gets to *name* another entity, not
    /// to reassign which entity it is running on. There is no arithmetic on one
    /// and no way to build one, so the only references a script can hold are
    /// ones the host handed it.
    Handle(Handle),
}

/// The references the surface offers.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Handle {
    /// The entity this script is running on, so it can be passed to something
    /// that takes an entity — or left on the blackboard for another script.
    Own,
}

/// How the host reaches one number.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Leaf {
    /// One axis of a transform's vector member.
    TransformAxis(Vector, usize),
    /// A scalar member of the transform.
    TransformScalar(Scalar),
    /// A number inside a component's stored JSON payload.
    ///
    /// Reached by pointer rather than through the typed view, because a
    /// component is a `Deserialize`-only view over a payload and the payload is
    /// what gets written back — going through the view would mean rebuilding
    /// and reserializing it, which is how a field the view does not know about
    /// gets dropped.
    Component {
        component: &'static str,
        pointer: &'static [Seg],
    },
}

/// One step into a JSON payload.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Seg {
    Field(&'static str),
    Index(usize),
}

/// A transform member that is three floats.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Vector {
    Position,
    Scale,
}

impl Vector {
    pub(crate) const fn get(self, transform: &Transform3D) -> [f32; 3] {
        match self {
            Self::Position => transform.position,
            Self::Scale => transform.scale,
        }
    }

    pub(crate) const fn set(self, transform: &mut Transform3D, value: [f32; 3]) {
        match self {
            Self::Position => transform.position = value,
            Self::Scale => transform.scale = value,
        }
    }
}

/// A transform member that is one float.
///
/// Only the Z rotation, deliberately. A gameplay script should not be asked to
/// assemble a quaternion by hand, and a third of a 3D rotation API is worse
/// than none of one.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Scalar {
    RotationZ,
}

impl Scalar {
    pub(crate) fn get(self, transform: &Transform3D) -> f32 {
        match self {
            Self::RotationZ => transform.rotation_z_radians(),
        }
    }

    pub(crate) fn set(self, transform: &mut Transform3D, value: f32) {
        match self {
            Self::RotationZ => transform.set_rotation_z_radians(value),
        }
    }
}

/// What `this` offers beyond the script's own fields.
pub(crate) const THIS: &[(&str, Node)] = &[
    ("transform", Node::Group(TRANSFORM, TRANSFORM_MEMBERS)),
    ("sprite", Node::Group(SPRITE, SPRITE_MEMBERS)),
    ("ui_image", Node::Group(UI_IMAGE, UI_IMAGE_MEMBERS)),
    ("entity", Node::Handle(Handle::Own)),
];

/// What a script reaches *through* a reference.
///
/// The same data members as `this`, and not `entity` itself: `a.entity` would
/// be `a`, which is a path that says nothing. So one entity reaches another's
/// transform and sprite exactly as it reaches its own, which is the property
/// that makes a reference worth having.
pub(crate) const THROUGH_REFERENCE: &[(&str, Node)] = &[
    ("transform", Node::Group(TRANSFORM, TRANSFORM_MEMBERS)),
    ("sprite", Node::Group(SPRITE, SPRITE_MEMBERS)),
    ("ui_image", Node::Group(UI_IMAGE, UI_IMAGE_MEMBERS)),
];

/// Finds the leaf a path names, given the path's parts after `this`.
pub(crate) fn leaf(parts: &[&str]) -> Option<Leaf> {
    leaf_in(THIS, parts)
}

/// The same, for a path rooted at a reference rather than at `this`.
pub(crate) fn leaf_through_reference(parts: &[&str]) -> Option<Leaf> {
    leaf_in(THROUGH_REFERENCE, parts)
}

/// Finds the handle a path names, when it names one rather than a number.
pub(crate) fn handle(parts: &[&str]) -> Option<Handle> {
    let [name] = parts else {
        return None;
    };
    THIS.iter().find_map(|(known, node)| match node {
        Node::Handle(handle) if known == name => Some(*handle),
        _ => None,
    })
}

fn leaf_in(root: &'static [(&'static str, Node)], parts: &[&str]) -> Option<Leaf> {
    let mut members = root;
    let mut steps = parts.iter();
    loop {
        let step = steps.next()?;
        let node = &members.iter().find(|(name, _)| name == step)?.1;
        match node {
            Node::Leaf(leaf) => {
                // A leaf with path left over is not this leaf: `position.x.y`
                // names nothing, and answering it would be worse than refusing.
                return steps.next().is_none().then_some(*leaf);
            }
            Node::Group(_, nested) => members = nested,
            Node::Handle(handle) => {
                // A handle with path left over pivots to what it names, so
                // `this.entity.transform.position.x` resolves — the analyzer
                // accepts it, so the host has to answer it.
                //
                // `Own` names the same entity this call is already about, so
                // the members change and the entity does not. A handle naming
                // something else would need the host to switch entity too, and
                // this match is where whoever adds one will be made to notice.
                match handle {
                    Handle::Own => members = THROUGH_REFERENCE,
                }
            }
        }
    }
}

impl Leaf {
    /// Reads the number this leaf names, or `None` when the entity has no such
    /// component or the payload does not hold one there.
    pub(crate) fn read(self, transform: Option<&Transform3D>, components: &Json) -> Option<f64> {
        Some(match self {
            Self::TransformAxis(vector, index) => f64::from(vector.get(transform?)[index]),
            Self::TransformScalar(scalar) => f64::from(scalar.get(transform?)),
            Self::Component { component, pointer } => {
                follow(components.get(component)?, pointer)?.as_f64()?
            }
        })
    }
}

/// Walks a JSON payload by pointer.
pub(crate) fn follow<'a>(value: &'a Json, pointer: &[Seg]) -> Option<&'a Json> {
    pointer.iter().try_fold(value, |value, step| match step {
        Seg::Field(name) => value.get(name),
        Seg::Index(index) => value.get(index),
    })
}

/// Walks a JSON payload by pointer, for writing.
pub(crate) fn follow_mut<'a>(value: &'a mut Json, pointer: &[Seg]) -> Option<&'a mut Json> {
    pointer.iter().try_fold(value, |value, step| match step {
        Seg::Field(name) => value.get_mut(name),
        Seg::Index(index) => value.get_mut(index),
    })
}
