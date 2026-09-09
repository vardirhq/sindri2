//! What a field's choice decides *besides itself*.
//!
//! Most choices decide only their own value: a text element's `wrap` is one of
//! three words and the element holds the same fields whichever it is. A few
//! decide the shape of what holds them. A camera's `projection` is the plain
//! case — a perspective one frames with a field of view, an orthographic one
//! with a vertical size, and they are two payloads rather than one payload with
//! a word changed. A collider piece's `shape` is the same thing one level down.
//!
//! For those, the choice's spellings are not enough to edit it. Writing
//! `circle` over `box` leaves half extents behind and no radius, which is a
//! payload the schema refuses: a control that looks like it works and does not.
//! So the editor did not offer one, and a camera's projection was the single
//! exception, switched by a hand-written rule that knew the camera's two field
//! names.
//!
//! What that rule knew is now something a component says. A registration names
//! the tag and what each variant holds, and switching is the same operation for
//! every tagged field:
//!
//! - the fields belonging to the variant being left are dropped,
//! - the fields the arriving variant needs are filled in from its template,
//! - and everything else is kept, because a field both variants have is a field
//!   the author set and the switch has no opinion about.
//!
//! Every variant is checked at registration by building the component it would
//! produce and decoding it, so a variant that cannot be chosen safely is a
//! startup error rather than a scene that will not load.

use serde_json::{Map, Value};

/// One field that decides what else its object holds.
#[derive(Clone, Debug)]
pub(super) struct TaggedField {
    /// Where the tag lives, as a dotted path: `projection`, or
    /// `pieces[].shape.shape`.
    path: String,
    /// The tag's own field name, which is the last step of that path. Kept
    /// beside it because a switch writes the tag into an object, and the object
    /// knows nothing about the path that led to it.
    tag: String,
    /// What each spelling makes the object hold.
    variants: Vec<(&'static str, Value)>,
}

use super::{
    ComponentRegistryError, ComponentSchemaRegistry, FieldMeaning, SceneComponent, meaning,
    validate_payload,
};

impl ComponentSchemaRegistry {
    /// Says what each spelling of a choice makes the component hold.
    ///
    /// `tag_path` names the field that carries the spelling — `projection`, or
    /// `pieces[].shape.shape` — and each variant's template is what the object
    /// holding that tag consists of when the tag says that word.
    ///
    /// The path must already be described as a [`FieldMeaning::Choice`], and
    /// the two lists must agree exactly: a spelling nothing describes is one an
    /// editor would offer and write badly, and a variant no spelling names is
    /// one nobody can pick. Both are registration errors, so the pair cannot
    /// drift the way the editor's own table did.
    ///
    /// Each variant is then *tried*: it is written into a copy of the field
    /// template and the result is decoded as the component. A variant that
    /// would produce a payload the engine rejects fails here, at startup, and
    /// not under somebody's cursor.
    pub fn describe_variants<T: SceneComponent>(
        &mut self,
        tag_path: &'static str,
        variants: impl IntoIterator<Item = (&'static str, Value)>,
    ) -> Result<(), ComponentRegistryError> {
        let registration = self
            .registrations
            .get_mut(T::TYPE_NAME)
            .ok_or(ComponentRegistryError::NotRegistered(T::TYPE_NAME))?;
        let Some(template) = registration.fields.as_ref() else {
            return Err(ComponentRegistryError::DescribedWithoutFields(T::TYPE_NAME));
        };
        let Some((parent, tag)) = meaning::split_leaf(tag_path).filter(|_| {
            // The tag has to be a field the component actually has, for the
            // same reason a meaning does: a renamed one must stop the build
            // rather than stop appearing.
            meaning::resolves(template, tag_path)
        }) else {
            return Err(ComponentRegistryError::UnknownFieldPath {
                type_name: T::TYPE_NAME,
                path: tag_path.to_owned(),
            });
        };
        let spellings = registration
            .meanings
            .iter()
            .find(|(stored, meaning)| {
                matches!(meaning, FieldMeaning::Choice(_)) && meaning::matches(stored, tag_path)
            })
            .map(|(_, meaning)| match meaning {
                FieldMeaning::Choice(spellings) => spellings.clone(),
                _ => Vec::new(),
            })
            .ok_or(ComponentRegistryError::VariantsWithoutChoice {
                type_name: T::TYPE_NAME,
                path: tag_path.to_owned(),
            })?;

        let variants: Vec<(&'static str, Value)> = variants.into_iter().collect();
        for (name, described) in &variants {
            if !spellings.contains(name) {
                return Err(ComponentRegistryError::UnknownVariant {
                    type_name: T::TYPE_NAME,
                    path: tag_path.to_owned(),
                    variant: (*name).to_owned(),
                });
            }
            if !described.is_object() {
                return Err(ComponentRegistryError::InvalidVariantTemplate {
                    type_name: T::TYPE_NAME,
                    variant: (*name).to_owned(),
                });
            }
        }
        for spelling in &spellings {
            if !variants.iter().any(|(name, _)| name == spelling) {
                return Err(ComponentRegistryError::UndescribedVariant {
                    type_name: T::TYPE_NAME,
                    path: tag_path.to_owned(),
                    variant: (*spelling).to_owned(),
                });
            }
        }
        for (name, _) in &variants {
            let mut candidate = template.clone();
            let Some(holder) = meaning::at_mut(&mut candidate, parent) else {
                return Err(ComponentRegistryError::UnknownFieldPath {
                    type_name: T::TYPE_NAME,
                    path: parent.to_owned(),
                });
            };
            apply(&variants, tag, name, holder);
            validate_payload::<T>(&candidate).map_err(|source| {
                ComponentRegistryError::VariantMismatch {
                    type_name: T::TYPE_NAME,
                    variant: (*name).to_owned(),
                    source,
                }
            })?;
        }
        registration.variants.push(TaggedField {
            path: tag_path.to_owned(),
            tag: tag.to_owned(),
            variants,
        });
        Ok(())
    }

    /// The variants of the tag at `path`, if the component declared any.
    ///
    /// `path` names the tag actually being looked at, so a piece's shape is
    /// asked for by index — `pieces.2.shape.shape` — and answered by the
    /// exemplar path the registration stored.
    #[must_use]
    pub fn variants(&self, type_name: &str, path: &str) -> Option<&[(&'static str, Value)]> {
        self.registrations
            .get(type_name)?
            .variants
            .iter()
            .find(|tagged| meaning::matches(&tagged.path, path))
            .map(|tagged| tagged.variants.as_slice())
    }

    /// Every path this component declared variants for.
    ///
    /// For a tool that has a payload rather than a field, and needs to know
    /// which of its fields decide the shape of the rest.
    pub fn variant_tags(&self, type_name: &str) -> impl Iterator<Item = &str> {
        self.registrations
            .get(type_name)
            .into_iter()
            .flat_map(|registration| registration.variants.iter())
            .map(|tagged| tagged.path.as_str())
    }

    /// Rewrites `holder` for the variant `chosen`, if that is a variant.
    ///
    /// `holder` is the object the tag belongs to, not the tag: switching a
    /// piece's shape is a change to the piece. Returns whether it wrote, which
    /// is false for a path with no variants and for a spelling none of them
    /// has — a caller offering a choice the registration describes gets
    /// neither.
    pub fn switch_variant(
        &self,
        type_name: &str,
        path: &str,
        chosen: &str,
        holder: &mut Value,
    ) -> bool {
        let Some(registration) = self.registrations.get(type_name) else {
            return false;
        };
        let Some(tagged) = registration
            .variants
            .iter()
            .find(|tagged| meaning::matches(&tagged.path, path))
        else {
            return false;
        };
        apply(&tagged.variants, &tagged.tag, chosen, holder)
    }
}

/// The switch itself, in one place because registration and editing must agree.
///
/// If checking a variant at startup and choosing it later did not do the same
/// thing, the check would be proving something about a payload nobody writes.
fn apply(variants: &[(&'static str, Value)], tag: &str, chosen: &str, holder: &mut Value) -> bool {
    let Some(arriving) = variants
        .iter()
        .find(|(name, _)| *name == chosen)
        .and_then(|(_, template)| template.as_object())
    else {
        return false;
    };
    let Some(fields) = holder.as_object_mut() else {
        return false;
    };
    drop_fields_of_variants_being_left(variants, arriving, chosen, fields);
    for (key, value) in arriving {
        // What the author already set is what they meant. A field both
        // variants have — a camera's near plane — is not part of the switch,
        // so it keeps its value rather than returning to a default.
        fields.entry(key.clone()).or_insert_with(|| value.clone());
    }
    fields.insert(tag.to_owned(), Value::String(chosen.to_owned()));
    true
}

/// Removes the fields that belonged to a variant this object is no longer.
///
/// Only those. Dropping every key the arriving variant does not name would
/// also drop whatever else the payload carried, and a switch is not a licence
/// to throw away fields nobody described.
fn drop_fields_of_variants_being_left(
    variants: &[(&'static str, Value)],
    arriving: &Map<String, Value>,
    chosen: &str,
    fields: &mut Map<String, Value>,
) {
    for (name, template) in variants {
        if *name == chosen {
            continue;
        }
        let Some(leaving) = template.as_object() else {
            continue;
        };
        for key in leaving.keys() {
            if !arriving.contains_key(key) {
                fields.remove(key);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use serde::Deserialize;
    use serde_json::json;

    use crate::component::{
        ComponentRegistryError, ComponentSchemaRegistry, FieldMeaning, SceneComponent,
    };

    /// A component shaped like the ones this exists for: a tag, the field each
    /// spelling brings with it, and a field neither of them decides.
    #[derive(Debug, Deserialize)]
    #[serde(tag = "lens")]
    #[allow(dead_code)]
    enum Eye {
        #[serde(rename = "wide")]
        Wide { spread: f32, near: f32 },
        #[serde(rename = "long")]
        Long { reach: f32, near: f32 },
    }

    impl SceneComponent for Eye {
        const TYPE_NAME: &'static str = "game.eye";
    }

    const LENSES: [&str; 2] = ["wide", "long"];

    fn registry() -> ComponentSchemaRegistry {
        let mut registry = ComponentSchemaRegistry::default();
        registry
            .register_with_default::<Eye>(
                "Eye",
                json!({ "lens": "wide", "spread": 90.0, "near": 0.1 }),
            )
            .unwrap();
        registry
            .describe::<Eye>([("lens", FieldMeaning::choice(LENSES))])
            .unwrap();
        registry
    }

    fn described(registry: &mut ComponentSchemaRegistry) -> Result<(), ComponentRegistryError> {
        registry.describe_variants::<Eye>(
            "lens",
            [
                ("wide", json!({ "spread": 90.0, "near": 0.1 })),
                ("long", json!({ "reach": 12.0, "near": 0.1 })),
            ],
        )
    }

    /// The whole point: the word and the fields it needs are written together,
    /// and the fields of the variant being left go with it.
    #[test]
    fn switching_writes_the_fields_the_arriving_variant_has() {
        let mut registry = registry();
        described(&mut registry).unwrap();
        let mut eye = json!({ "lens": "wide", "spread": 90.0, "near": 0.1 });

        assert!(registry.switch_variant("game.eye", "lens", "long", &mut eye));
        assert_eq!(eye["lens"], json!("long"));
        assert!(eye.get("spread").is_none(), "the wide lens took its field");
        assert_eq!(eye["reach"], json!(12.0));
        registry.validate_payload("game.eye", &eye).unwrap();
    }

    /// A field both variants have is a field the author set, not part of the
    /// switch. Returning it to a default would quietly undo their work.
    #[test]
    fn a_field_both_variants_have_keeps_its_value() {
        let mut registry = registry();
        described(&mut registry).unwrap();
        let mut eye = json!({ "lens": "wide", "spread": 90.0, "near": 5.0 });
        registry.switch_variant("game.eye", "lens", "long", &mut eye);
        assert_eq!(eye["near"], json!(5.0));
    }

    /// Switching away and back keeps a value that is still there rather than
    /// overwriting it with the template's.
    #[test]
    fn switching_back_keeps_what_was_already_there() {
        let mut registry = registry();
        described(&mut registry).unwrap();
        let mut eye = json!({ "lens": "long", "reach": 12.0, "spread": 30.0, "near": 0.1 });
        registry.switch_variant("game.eye", "lens", "wide", &mut eye);
        assert_eq!(eye["spread"], json!(30.0));
    }

    /// A switch drops the fields of the variant being left, and only those. A
    /// payload carrying something no variant named keeps it: this is a change
    /// of shape, not a licence to throw away fields nobody described.
    #[test]
    fn a_field_no_variant_names_is_left_alone() {
        let mut registry = registry();
        described(&mut registry).unwrap();
        let mut eye = json!({ "lens": "wide", "spread": 90.0, "near": 0.1, "note": "keep me" });
        registry.switch_variant("game.eye", "lens", "long", &mut eye);
        assert_eq!(eye["note"], json!("keep me"));
    }

    #[test]
    fn a_spelling_no_variant_has_writes_nothing() {
        let mut registry = registry();
        described(&mut registry).unwrap();
        let before = json!({ "lens": "wide", "spread": 90.0, "near": 0.1 });
        let mut eye = before.clone();
        assert!(!registry.switch_variant("game.eye", "lens", "fisheye", &mut eye));
        assert_eq!(eye, before, "a spelling nothing describes changes nothing");
        assert!(!registry.switch_variant("game.eye", "elsewhere", "long", &mut eye));
        assert_eq!(eye, before, "and neither does a path with no variants");
    }

    /// The check that makes the picker trustworthy: a variant that cannot be
    /// chosen safely stops the build, rather than waiting under a cursor.
    #[test]
    fn a_variant_that_does_not_decode_is_refused() {
        let mut registry = registry();
        let refused = registry.describe_variants::<Eye>(
            "lens",
            [
                ("wide", json!({ "spread": 90.0, "near": 0.1 })),
                // A long lens with no reach is not a lens the engine loads.
                ("long", json!({ "near": 0.1 })),
            ],
        );
        assert!(matches!(
            refused,
            Err(ComponentRegistryError::VariantMismatch { variant, .. }) if variant == "long"
        ));
    }

    /// The two lists are one list. A spelling with nothing to write for it is a
    /// dropdown entry that breaks the payload.
    #[test]
    fn a_spelling_with_no_variant_is_refused() {
        let mut registry = registry();
        let refused = registry
            .describe_variants::<Eye>("lens", [("wide", json!({ "spread": 90.0, "near": 0.1 }))]);
        assert!(matches!(
            refused,
            Err(ComponentRegistryError::UndescribedVariant { variant, .. }) if variant == "long"
        ));
    }

    /// And the other direction: a variant nobody can pick.
    #[test]
    fn a_variant_no_spelling_names_is_refused() {
        let mut registry = registry();
        let refused = registry.describe_variants::<Eye>(
            "lens",
            [
                ("wide", json!({ "spread": 90.0, "near": 0.1 })),
                ("long", json!({ "reach": 12.0, "near": 0.1 })),
                ("fisheye", json!({ "bulge": 1.0 })),
            ],
        );
        assert!(matches!(
            refused,
            Err(ComponentRegistryError::UnknownVariant { variant, .. }) if variant == "fisheye"
        ));
    }

    /// Variants exist so a choice can be written. A path nothing offers as a
    /// choice has nothing to pick them by.
    #[test]
    fn variants_on_a_field_that_is_not_a_choice_are_refused() {
        let mut registry = registry();
        let refused =
            registry.describe_variants::<Eye>("near", [("wide", json!({ "spread": 90.0 }))]);
        assert!(matches!(
            refused,
            Err(ComponentRegistryError::VariantsWithoutChoice { .. })
        ));
    }

    #[test]
    fn variants_on_a_field_that_does_not_exist_are_refused() {
        let mut registry = registry();
        let refused =
            registry.describe_variants::<Eye>("lense", [("wide", json!({ "spread": 90.0 }))]);
        assert!(matches!(
            refused,
            Err(ComponentRegistryError::UnknownFieldPath { .. })
        ));
    }
}
