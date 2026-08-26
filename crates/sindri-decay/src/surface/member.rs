//! What a component's members are, and how each one is read and written.
//!
//! A new member is an entry in the list for its component, with the two
//! accessors beside it. Adding one is a single visible edit.

use super::names::{RGBA, SPRITE_COMPONENT, VEC3};
use super::{Leaf, Node, Scalar, Seg, Vector};

const POSITION: &[(&str, Node)] = &[
    ("x", Node::Leaf(Leaf::TransformAxis(Vector::Position, 0))),
    ("y", Node::Leaf(Leaf::TransformAxis(Vector::Position, 1))),
    ("z", Node::Leaf(Leaf::TransformAxis(Vector::Position, 2))),
];

const SCALE: &[(&str, Node)] = &[
    ("x", Node::Leaf(Leaf::TransformAxis(Vector::Scale, 0))),
    ("y", Node::Leaf(Leaf::TransformAxis(Vector::Scale, 1))),
    ("z", Node::Leaf(Leaf::TransformAxis(Vector::Scale, 2))),
];

pub(crate) const TRANSFORM_MEMBERS: &[(&str, Node)] = &[
    ("position", Node::Group(VEC3, POSITION)),
    ("scale", Node::Group(VEC3, SCALE)),
    (
        "rotation_z",
        Node::Leaf(Leaf::TransformScalar(Scalar::RotationZ)),
    ),
];

/// A tint channel, by its place in the component's `[r, g, b, a]`.
const fn tint(index: usize) -> Node {
    Node::Leaf(Leaf::Component {
        component: SPRITE_COMPONENT,
        pointer: match index {
            0 => &[Seg::Field("tint"), Seg::Index(0)],
            1 => &[Seg::Field("tint"), Seg::Index(1)],
            2 => &[Seg::Field("tint"), Seg::Index(2)],
            _ => &[Seg::Field("tint"), Seg::Index(3)],
        },
    })
}

const TINT: &[(&str, Node)] = &[
    ("r", tint(0)),
    ("g", tint(1)),
    ("b", tint(2)),
    ("a", tint(3)),
];

pub(crate) const SPRITE_MEMBERS: &[(&str, Node)] = &[
    ("tint", Node::Group(RGBA, TINT)),
    (
        "layer",
        Node::Leaf(Leaf::Component {
            component: SPRITE_COMPONENT,
            pointer: &[Seg::Field("layer")],
        }),
    ),
];
