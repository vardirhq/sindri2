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
//! one edit, visible in a diff, and not something that can be half-done.

use serde_json::Value as Json;
use sindri_core::Transform3D;

pub(crate) const TRANSFORM: &str = "Transform";
pub(crate) const VEC3: &str = "Vec3";
pub(crate) const SPRITE: &str = "Sprite";
pub(crate) const RGBA: &str = "Rgba";
pub(crate) const INPUT: &str = "Input";
pub(crate) const TIME: &str = "Time";

/// The component a sprite's fields live in.
pub(crate) const SPRITE_COMPONENT: &str = "sindri.sprite";

/// A node in the surface: either a nested type, or a value a script reaches.
pub(crate) enum Node {
    /// A named type with members of its own.
    Group(&'static str, &'static [(&'static str, Node)]),
    /// A number a script can read and write.
    Leaf(Leaf),
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

const TRANSFORM_MEMBERS: &[(&str, Node)] = &[
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

const SPRITE_MEMBERS: &[(&str, Node)] = &[
    ("tint", Node::Group(RGBA, TINT)),
    (
        "layer",
        Node::Leaf(Leaf::Component {
            component: SPRITE_COMPONENT,
            pointer: &[Seg::Field("layer")],
        }),
    ),
];

/// What `this` offers beyond the script's own fields.
pub(crate) const THIS: &[(&str, Node)] = &[
    ("transform", Node::Group(TRANSFORM, TRANSFORM_MEMBERS)),
    ("sprite", Node::Group(SPRITE, SPRITE_MEMBERS)),
];

/// Finds the leaf a path names, given the path's parts after `this`.
pub(crate) fn leaf(parts: &[&str]) -> Option<Leaf> {
    let mut members = THIS;
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

/// A host function, by how many numbers it takes.
#[derive(Clone, Copy, Debug)]
pub(crate) enum HostFunction {
    Unary(fn(f64) -> f64),
    Binary(fn(f64, f64) -> f64),
}

/// The maths a script can do beyond arithmetic.
///
/// Decay has no modules and no imports, so each of these is a bare global name,
/// and every one added is a name a script can no longer use for its own.
pub(crate) const FUNCTIONS: &[(&str, HostFunction)] = &[
    ("abs", HostFunction::Unary(f64::abs)),
    ("sqrt", HostFunction::Unary(f64::sqrt)),
    ("sin", HostFunction::Unary(f64::sin)),
    ("cos", HostFunction::Unary(f64::cos)),
    ("min", HostFunction::Binary(f64::min)),
    ("max", HostFunction::Binary(f64::max)),
];

/// The name a script calls to say something into the host's log.
pub(crate) const PRINT: &str = "print";

/// A question a script asks about the keyboard.
#[derive(Clone, Copy, Debug)]
pub(crate) enum InputQuery {
    /// Two opposing keys as -1, 0, or 1.
    Axis,
    Down,
    Pressed,
    Released,
}

impl InputQuery {
    /// How many key names it takes.
    pub(crate) const fn keys(self) -> usize {
        match self {
            Self::Axis => 2,
            Self::Down | Self::Pressed | Self::Released => 1,
        }
    }

    /// Whether it answers with a number rather than a truth.
    pub(crate) const fn is_number(self) -> bool {
        matches!(self, Self::Axis)
    }
}

pub(crate) const INPUT_QUERIES: &[(&str, InputQuery)] = &[
    ("axis", InputQuery::Axis),
    ("is_down", InputQuery::Down),
    ("just_pressed", InputQuery::Pressed),
    ("just_released", InputQuery::Released),
];

/// What a script can ask about the frame it is in.
#[derive(Clone, Copy, Debug)]
pub(crate) enum TimeValue {
    /// How long this frame is, which `update` also receives as its argument.
    Delta,
    /// How long the script instance has been running.
    Elapsed,
}

pub(crate) const TIME_VALUES: &[(&str, TimeValue)] =
    &[("delta", TimeValue::Delta), ("elapsed", TimeValue::Elapsed)];

#[cfg(test)]
mod tests {
    use decay_ir::Path;
    use decay_runtime::{Host, Value};
    use decay_semantic::{Environment, ExternalSymbol, Type};
    use sindri_core::{EntityData, Transform3D, World};
    use sindri_platform::InputState;

    use crate::{ScriptContext, WorldHost, environment};

    fn context(input: &InputState) -> ScriptContext<'_> {
        ScriptContext {
            input,
            delta_seconds: 0.5,
            elapsed_seconds: 1.5,
        }
    }

    /// An entity carrying every component the surface can reach into, so a path
    /// that needs one is not refused for the entity's sake.
    fn world() -> (World, sindri_core::EntityId) {
        let mut world = World::default();
        let entity = world.spawn(EntityData {
            transform_3d: Some(Transform3D::default()),
            components: [(
                super::SPRITE_COMPONENT.to_owned(),
                serde_json::json!({
                    "texture": "procedural:checkerboard",
                    "tint": [1.0, 1.0, 1.0, 1.0],
                    "layer": 0
                }),
            )]
            .into_iter()
            .collect(),
            ..EntityData::default()
        });
        (world, entity)
    }

    /// Every path the analyzer will accept, the host answers.
    ///
    /// Walked out of the *environment* rather than the tree, and that direction
    /// is the point. A path the host can reach but nothing describes is merely
    /// unreachable; a path the analyzer accepts and the host cannot answer is a
    /// clean compile followed by `UnknownPath` at frame one, which is the
    /// failure this module exists to make impossible.
    #[test]
    fn the_host_answers_every_path_the_analyzer_accepts() {
        let (mut world, entity) = world();
        let input = InputState::default();
        let environment = environment();
        let mut checked = 0;

        for (member, symbol) in environment.this().members() {
            for (parts, terminal) in walk(
                &environment,
                vec!["this".to_owned(), member.to_owned()],
                symbol,
            ) {
                let path = Path(parts);
                let dotted = path.dotted();
                assert_eq!(
                    terminal,
                    Type::F32,
                    "{dotted} ends in `{}`, and the host only carries numbers here",
                    terminal.display_name()
                );

                let mut host = WorldHost::new(&mut world, entity, context(&input));
                assert!(
                    matches!(host.load(&path), Ok(Some(Value::Number(_)))),
                    "the analyzer accepts {dotted} and the host cannot read it"
                );
                let mut host = WorldHost::new(&mut world, entity, context(&input));
                assert_eq!(
                    host.store(&path, Value::Number(1.0)),
                    Ok(true),
                    "the analyzer accepts {dotted} and the host cannot write it"
                );
                checked += 1;
            }
        }
        assert!(checked > 0, "the surface describes nothing at all");
    }

    /// The namespaced globals -- `Input` and `Time` -- are described and reached
    /// by a different mechanism from `this`, so they get the same treatment
    /// rather than being trusted.
    #[test]
    fn the_host_answers_every_global_the_analyzer_describes() {
        let (mut world, entity) = world();
        let input = InputState::default();
        let environment = environment();
        let mut checked = 0;

        for (namespace, symbol) in environment.globals() {
            let ExternalSymbol::Value(Type::Named(type_name)) = symbol else {
                continue;
            };
            let described = environment
                .get_type(type_name)
                .unwrap_or_else(|| panic!("`{type_name}` is offered but never described"));

            for (name, member) in described.members() {
                let path = Path(vec![namespace.to_owned(), name.to_owned()]);
                let dotted = path.dotted();
                let mut host = WorldHost::new(&mut world, entity, context(&input));
                match member {
                    ExternalSymbol::Value(_) => assert!(
                        matches!(host.load(&path), Ok(Some(_))),
                        "the analyzer describes {dotted} and the host cannot read it"
                    ),
                    ExternalSymbol::Function(signature) => {
                        // Key names, because everything callable on a namespace
                        // currently takes them. A signature that stops being
                        // strings will fail here, which is the right moment to
                        // notice.
                        let args = vec![Value::String("Space".to_owned()); signature.params.len()];
                        assert!(
                            matches!(host.call(&path, &args), Ok(Some(_))),
                            "the analyzer describes {dotted} and the host does not perform it"
                        );
                    }
                }
                checked += 1;
            }
        }
        assert!(checked > 0, "no namespaced globals are described");
    }

    /// Expands a described member into every complete path under it, with the
    /// type each one ends in.
    fn walk(
        environment: &Environment,
        parts: Vec<String>,
        symbol: &ExternalSymbol,
    ) -> Vec<(Vec<String>, Type)> {
        let ExternalSymbol::Value(ty) = symbol else {
            return Vec::new();
        };
        let Type::Named(name) = ty else {
            return vec![(parts, ty.clone())];
        };
        let described = environment
            .get_type(name)
            .unwrap_or_else(|| panic!("`{name}` is named by the surface but never described"));
        described
            .members()
            .flat_map(|(field, symbol)| {
                let mut parts = parts.clone();
                parts.push(field.to_owned());
                walk(environment, parts, symbol)
            })
            .collect()
    }

    /// Every bare function the analyzer offers is one the host performs.
    #[test]
    fn every_described_function_is_one_the_host_performs() {
        let (mut world, entity) = world();
        let input = InputState::default();
        let environment = environment();

        for (name, symbol) in environment.globals() {
            let ExternalSymbol::Function(signature) = symbol else {
                continue;
            };
            let args = vec![Value::Number(1.0); signature.params.len()];
            let mut host = WorldHost::new(&mut world, entity, context(&input));
            assert!(
                matches!(host.call(&Path(vec![name.to_owned()]), &args), Ok(Some(_))),
                "the host does not perform `{name}`, which the environment offers"
            );
        }
    }
}
