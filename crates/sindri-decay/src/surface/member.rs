//! What a component's members are, and how each one is read and written.
//!
//! A new member is an entry in the list for its component, with the two
//! accessors beside it. Adding one is a single visible edit.

use super::names::{RGBA, SHAPE_COMPONENT, SPRITE_COMPONENT, UI_IMAGE_COMPONENT, VEC3};
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

/// A tint channel of `component`, by its place in its `[r, g, b, a]`.
///
/// Taken as a parameter because a world sprite and a UI image hold the same
/// four numbers in two different components, and a script reaching the wrong
/// one would silently write a payload nothing draws.
const fn tint(component: &'static str, index: usize) -> Node {
    Node::Leaf(Leaf::Component {
        component,
        pointer: match index {
            0 => &[Seg::Field("tint"), Seg::Index(0)],
            1 => &[Seg::Field("tint"), Seg::Index(1)],
            2 => &[Seg::Field("tint"), Seg::Index(2)],
            _ => &[Seg::Field("tint"), Seg::Index(3)],
        },
    })
}

const TINT: &[(&str, Node)] = &[
    ("r", tint(SPRITE_COMPONENT, 0)),
    ("g", tint(SPRITE_COMPONENT, 1)),
    ("b", tint(SPRITE_COMPONENT, 2)),
    ("a", tint(SPRITE_COMPONENT, 3)),
];

const UI_IMAGE_TINT: &[(&str, Node)] = &[
    ("r", tint(UI_IMAGE_COMPONENT, 0)),
    ("g", tint(UI_IMAGE_COMPONENT, 1)),
    ("b", tint(UI_IMAGE_COMPONENT, 2)),
    ("a", tint(UI_IMAGE_COMPONENT, 3)),
];

/// One channel of a shape's fill.
///
/// Spelled out per field rather than taking the field name as a parameter: the
/// pointer arrays are `'static` only because their contents are literals, and a
/// parameter makes them a temporary that cannot outlive the call.
const fn shape_fill(index: usize) -> Node {
    Node::Leaf(Leaf::Component {
        component: SHAPE_COMPONENT,
        pointer: match index {
            0 => &[Seg::Field("fill"), Seg::Index(0)],
            1 => &[Seg::Field("fill"), Seg::Index(1)],
            2 => &[Seg::Field("fill"), Seg::Index(2)],
            _ => &[Seg::Field("fill"), Seg::Index(3)],
        },
    })
}

/// One channel of a shape's stroke.
const fn shape_stroke(index: usize) -> Node {
    Node::Leaf(Leaf::Component {
        component: SHAPE_COMPONENT,
        pointer: match index {
            0 => &[Seg::Field("stroke"), Seg::Index(0)],
            1 => &[Seg::Field("stroke"), Seg::Index(1)],
            2 => &[Seg::Field("stroke"), Seg::Index(2)],
            _ => &[Seg::Field("stroke"), Seg::Index(3)],
        },
    })
}

const SHAPE_FILL: &[(&str, Node)] = &[
    ("r", shape_fill(0)),
    ("g", shape_fill(1)),
    ("b", shape_fill(2)),
    ("a", shape_fill(3)),
];

const SHAPE_STROKE: &[(&str, Node)] = &[
    ("r", shape_stroke(0)),
    ("g", shape_stroke(1)),
    ("b", shape_stroke(2)),
    ("a", shape_stroke(3)),
];

/// A shape has two colours where a sprite has one, and two numbers a sprite
/// has no equivalent of.
///
/// `sweep_turns` is the reason this is worth having at all: a cooldown ring, a
/// charge meter and a boss's health arc are one float a script already holds,
/// and drawing them from a sprite would need a frame of art per step.
/// `stroke_width` is how something pulses without changing size.
pub(crate) const SHAPE_MEMBERS: &[(&str, Node)] = &[
    ("fill", Node::Group(RGBA, SHAPE_FILL)),
    ("stroke", Node::Group(RGBA, SHAPE_STROKE)),
    (
        "stroke_width",
        Node::Leaf(Leaf::Component {
            component: SHAPE_COMPONENT,
            pointer: &[Seg::Field("stroke_width")],
        }),
    ),
    (
        "sweep_turns",
        Node::Leaf(Leaf::Component {
            component: SHAPE_COMPONENT,
            pointer: &[Seg::Field("sweep_turns")],
        }),
    ),
    (
        "sweep_start",
        Node::Leaf(Leaf::Component {
            component: SHAPE_COMPONENT,
            pointer: &[Seg::Field("sweep_start")],
        }),
    ),
    (
        "dashes",
        Node::Leaf(Leaf::Component {
            component: SHAPE_COMPONENT,
            pointer: &[Seg::Field("dashes")],
        }),
    ),
    (
        "layer",
        Node::Leaf(Leaf::Component {
            component: SHAPE_COMPONENT,
            pointer: &[Seg::Field("layer")],
        }),
    ),
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

/// The same two members, one component along.
///
/// A HUD fades and re-stacks exactly the way a world sprite does, so the
/// members are the same; what differs is which component they are stored in,
/// which is the whole of the difference between the two kinds of entity.
pub(crate) const UI_IMAGE_MEMBERS: &[(&str, Node)] = &[
    ("tint", Node::Group(RGBA, UI_IMAGE_TINT)),
    (
        "layer",
        Node::Leaf(Leaf::Component {
            component: UI_IMAGE_COMPONENT,
            pointer: &[Seg::Field("layer")],
        }),
    ),
];
