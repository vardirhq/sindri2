//! When an edit to a field takes effect.
//!
//! Most fields are cheap to change, and a tool should apply what is typed as
//! it is typed: a colour, a position, a label. Some are not. A voxel world
//! regenerates its terrain from its generator, and applying each keystroke of
//! `10` meant regenerating at `1` first, with the editor frozen until it was
//! done. So a component says, per field or as a whole, how an edit to it is
//! applied, and a tool honours it.
//!
//! Declared beside the component, as its meanings are, and checked against
//! its field template the same way, so a renamed field cannot leave a mode
//! pointing at nothing.

use super::{ComponentRegistryError, ComponentSchemaRegistry, SceneComponent, meaning};

/// How an edit to a field is applied.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ApplyMode {
    /// As it is made. What every field does unless it says otherwise.
    #[default]
    Instant,
    /// Once the person has stopped: a short pause in typing, Enter, moving
    /// to another field, or letting go of a drag. For a field that is costly
    /// to apply but should still feel live.
    Settled,
    /// Only when the person asks, with an Apply button, and not before. For a
    /// component whose every change is a long rebuild, where several edits
    /// are worth making and seeing the result of once.
    Manual,
}

impl ApplyMode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Instant => "instant",
            Self::Settled => "settled",
            Self::Manual => "manual",
        }
    }
}

impl ComponentSchemaRegistry {
    /// Says how edits to these fields of `T` are applied.
    ///
    /// A path is dotted as for [`Self::describe`], and the empty path is the
    /// whole component. The most specific declaration wins, so a component
    /// can be [`ApplyMode::Manual`] with one field [`ApplyMode::Instant`].
    pub fn apply_when<T: SceneComponent>(
        &mut self,
        mode: ApplyMode,
        paths: impl IntoIterator<Item = &'static str>,
    ) -> Result<(), ComponentRegistryError> {
        let registration = self
            .registrations
            .get_mut(T::TYPE_NAME)
            .ok_or(ComponentRegistryError::NotRegistered(T::TYPE_NAME))?;
        for path in paths {
            if !path.is_empty() && registration.exemplar(path).is_none() {
                return Err(ComponentRegistryError::UnknownFieldPath {
                    type_name: T::TYPE_NAME,
                    path: path.to_owned(),
                });
            }
            registration.apply.push((path.to_owned(), mode));
        }
        Ok(())
    }

    /// How an edit to the field at `path` of `type_name` is applied.
    ///
    /// `path` is the field actually edited, `materials.2.voxel`, and is
    /// covered by a declaration for it, for any field holding it, or for the
    /// whole component, the closest of those winning.
    #[must_use]
    pub fn apply_mode(&self, type_name: &str, path: &str) -> ApplyMode {
        let Some(registration) = self.registrations.get(type_name) else {
            return ApplyMode::Instant;
        };
        let segments = if path.is_empty() {
            Vec::new()
        } else {
            path.split('.').collect::<Vec<_>>()
        };
        // From the field itself outwards, so the closest declaration wins.
        for depth in (0..=segments.len()).rev() {
            let held = segments[..depth].join(".");
            let found = registration.apply.iter().rev().find(|(stored, _)| {
                if held.is_empty() {
                    stored.is_empty()
                } else {
                    !stored.is_empty() && meaning::matches(stored, &held)
                }
            });
            if let Some((_, mode)) = found {
                return *mode;
            }
        }
        ApplyMode::Instant
    }

    /// Every apply mode a component declares, in registration order, for
    /// generated documents.
    pub fn apply_modes(&self, type_name: &str) -> impl Iterator<Item = (&str, ApplyMode)> {
        self.registrations
            .get(type_name)
            .into_iter()
            .flat_map(|registration| registration.apply.iter())
            .map(|(path, mode)| (path.as_str(), *mode))
    }
}

#[cfg(test)]
mod tests {
    use serde::Deserialize;
    use serde_json::json;

    use super::ApplyMode;
    use crate::{ComponentRegistryError, ComponentSchemaRegistry, SceneComponent};

    #[derive(Deserialize)]
    #[allow(dead_code)]
    struct World {
        seed: u32,
        radius: u32,
        materials: Vec<Material>,
    }

    #[derive(Deserialize)]
    #[allow(dead_code)]
    struct Material {
        voxel: u32,
        top: String,
    }

    impl SceneComponent for World {
        const TYPE_NAME: &'static str = "test.world";
    }

    fn registry() -> ComponentSchemaRegistry {
        let mut registry = ComponentSchemaRegistry::default();
        registry
            .register_with_fields::<World>(
                "World",
                json!({ "seed": 0, "radius": 4, "materials": [{ "voxel": 1, "top": "" }] }),
            )
            .expect("registers");
        registry
    }

    #[test]
    fn a_field_says_nothing_until_something_does() {
        let registry = registry();
        assert_eq!(
            registry.apply_mode("test.world", "seed"),
            ApplyMode::Instant
        );
        assert_eq!(registry.apply_mode("unknown", "seed"), ApplyMode::Instant);
    }

    #[test]
    fn the_closest_declaration_wins() {
        let mut registry = registry();
        registry
            .apply_when::<World>(ApplyMode::Manual, [""])
            .expect("the whole component");
        registry
            .apply_when::<World>(ApplyMode::Instant, ["radius"])
            .expect("a field");
        registry
            .apply_when::<World>(ApplyMode::Settled, ["materials[].top"])
            .expect("a field of every item");
        assert_eq!(registry.apply_mode("test.world", "seed"), ApplyMode::Manual);
        assert_eq!(
            registry.apply_mode("test.world", "radius"),
            ApplyMode::Instant
        );
        assert_eq!(
            registry.apply_mode("test.world", "materials.3.top"),
            ApplyMode::Settled
        );
        assert_eq!(
            registry.apply_mode("test.world", "materials.3.voxel"),
            ApplyMode::Manual
        );
        assert_eq!(
            registry.apply_mode("test.world", "materials"),
            ApplyMode::Manual
        );
    }

    #[test]
    fn a_mode_for_a_field_that_does_not_exist_is_refused() {
        let mut registry = registry();
        assert!(matches!(
            registry.apply_when::<World>(ApplyMode::Settled, ["sead"]),
            Err(ComponentRegistryError::UnknownFieldPath { .. })
        ));
    }
}
