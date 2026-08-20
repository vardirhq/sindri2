//! The one description of what a script can reach.
//!
//! Two things need this and they must never disagree. The **analyzer** needs
//! types, so `this.transfrom.position.x` is a compile error and an editor can
//! one day complete after a dot. The **host** needs accessors, so the same path
//! reaches a real transform at runtime. Written twice, they drift: a path the
//! analyzer accepts and the host does not is a clean compile followed by
//! `UnknownPath` at frame one, which is the worst of both.
//!
//! So they are written once, here, and `environment` and `WorldHost` are both
//! derived from these tables. `surface_agrees_with_the_host` walks the
//! description and asserts the host answers every path in it, so the two cannot
//! be shipped disagreeing even if someone adds a third reader.
//!
//! Widening the surface means adding a row. That is the point: it should be one
//! edit, visible in a diff, and not something that can be half-done.

use sindri_core::Transform3D;

/// The host type naming an entity's transform.
pub(crate) const TRANSFORM: &str = "Transform";
/// The host type naming three floats.
pub(crate) const VEC3: &str = "Vec3";
/// The member of `this` that reaches the entity's transform.
pub(crate) const TRANSFORM_MEMBER: &str = "transform";

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

/// A host function, by how many numbers it takes.
#[derive(Clone, Copy, Debug)]
pub(crate) enum HostFunction {
    Unary(fn(f64) -> f64),
    Binary(fn(f64, f64) -> f64),
}

pub(crate) const VECTORS: &[(&str, Vector)] =
    &[("position", Vector::Position), ("scale", Vector::Scale)];

pub(crate) const SCALARS: &[(&str, Scalar)] = &[("rotation_z", Scalar::RotationZ)];

pub(crate) const AXES: &[(&str, usize)] = &[("x", 0), ("y", 1), ("z", 2)];

/// The entire standard library.
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

#[cfg(test)]
mod tests {
    use decay_ir::Path;
    use decay_runtime::{Host, Value};
    use decay_semantic::{Environment, ExternalSymbol, Type};
    use sindri_core::{EntityData, Transform3D, World};

    use crate::{WorldHost, environment};

    /// Every path the analyzer will accept, the host answers.
    ///
    /// Walked out of the *environment* rather than the tables, and that
    /// direction is the point. A path the host can reach but nothing describes
    /// is merely unreachable; a path the analyzer accepts and the host cannot
    /// answer is a clean compile followed by `UnknownPath` at frame one, which
    /// is the failure this module exists to make impossible. Deriving the walk
    /// from the description means a member added to the environment alone
    /// fails here rather than in someone's scene.
    #[test]
    fn the_host_answers_every_path_the_analyzer_accepts() {
        let mut world = World::default();
        let entity = world.spawn(EntityData {
            transform_3d: Some(Transform3D::default()),
            ..EntityData::default()
        });

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
                    "{dotted} ends in `{}`, and the host only carries numbers",
                    terminal.display_name()
                );

                let mut host = WorldHost::new(&mut world, entity);
                assert!(
                    matches!(host.load(&path), Ok(Some(Value::Number(_)))),
                    "the analyzer accepts {dotted} and the host cannot read it"
                );
                let mut host = WorldHost::new(&mut world, entity);
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

    /// Expands a described member into every complete path under it, with the
    /// type each one ends in.
    fn walk(
        environment: &Environment,
        parts: Vec<String>,
        symbol: &ExternalSymbol,
    ) -> Vec<(Vec<String>, Type)> {
        let ExternalSymbol::Value(ty) = symbol else {
            // A method is checked by the other test, which calls it.
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

    /// Every function the analyzer offers is one the host actually performs,
    /// with the arity it promised.
    #[test]
    fn every_described_function_is_one_the_host_performs() {
        let mut world = World::default();
        let entity = world.spawn(EntityData::default());
        let environment = environment();

        for (name, symbol) in environment.globals() {
            let ExternalSymbol::Function(signature) = symbol else {
                continue;
            };
            let args = vec![Value::Number(1.0); signature.params.len()];
            let mut host = WorldHost::new(&mut world, entity);
            assert!(
                matches!(
                    host.call(&Path(vec![name.to_owned()]), &args),
                    Ok(Some(Value::Number(_)))
                ),
                "the host does not perform `{name}`, which the environment offers"
            );
        }
    }
}
