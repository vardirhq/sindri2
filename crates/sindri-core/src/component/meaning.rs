//! What a field *means*, as opposed to what shape it is.
//!
//! A field template says a component has a `texture` and that it is a string.
//! It cannot say that the string names a texture in the project, so a tool
//! drawing the component has to guess — and the editor did, from a table of
//! field names: `texture` meant the texture list, `clip` meant the audio list,
//! a colour had to be spelled `tint`.
//!
//! Guessing from a name fails in both directions. The bare-key rules matched
//! *any* component, so a game's own component with a `clip` field was offered
//! the project's audio; and a field the table had never heard of — `sheet`,
//! `icon`, `portrait` — was a text box in silence, which is what happened to
//! every component added after the table was written.
//!
//! The deeper problem is that it was a second copy of knowledge about a
//! component, living away from the component. That is the same drift
//! [`super::ComponentSchemaRegistry::check_template`] already exists to stop
//! for field lists, so meaning is declared the same way and checked the same
//! way: every path a registration describes must resolve in that component's
//! field template, or registration fails at startup.

use serde_json::Value;

/// The kind of project asset a field names.
///
/// Deliberately the kinds a project actually holds. A field naming something
/// the project does not store is not an asset field, whatever it is called.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AssetKind {
    Texture,
    Audio,
    Font,
    Script,
    Prefab,
    Profile,
    Weave,
}

impl AssetKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Texture => "texture",
            Self::Audio => "audio",
            Self::Font => "font",
            Self::Script => "script",
            Self::Prefab => "prefab",
            Self::Profile => "profile",
            Self::Weave => "weave",
        }
    }
}

/// What a field means.
///
/// Every variant answers "what would somebody need in order to edit this
/// well", which is a different question from what type it deserializes as.
#[derive(Clone, Debug, PartialEq)]
pub enum FieldMeaning {
    /// Names an asset in the project, of this kind.
    Asset(AssetKind),
    /// One of a fixed set of spellings.
    ///
    /// The spellings come from the engine's own list at the registration site,
    /// never retyped: a choice that has drifted from the enum it names is a
    /// scene that will not load.
    Choice(Vec<&'static str>),
    /// A colour as RGBA, each channel from zero to one.
    ///
    /// Worth naming rather than inferring: the old check was "four numbers
    /// under a key called `tint`", and four numbers is also a UV rect and a
    /// quaternion.
    Colour,
    /// An angle in radians, however a tool chooses to show it.
    Angle,
    /// A number with both ends bounded, the same bounds the engine validates.
    Range { min: f64, max: f64 },
    /// A bit mask over collision layers.
    Mask,
    /// Names another entity in the same scene.
    Entity,
}

impl FieldMeaning {
    /// A choice built from the engine's own list of spellings.
    pub fn choice<I>(names: I) -> Self
    where
        I: IntoIterator<Item = &'static str>,
    {
        Self::Choice(names.into_iter().collect())
    }

    /// A short name for the kind of meaning, for generated documents.
    #[must_use]
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::Asset(_) => "asset",
            Self::Choice(_) => "choice",
            Self::Colour => "colour",
            Self::Angle => "angle",
            Self::Range { .. } => "range",
            Self::Mask => "mask",
            Self::Entity => "entity",
        }
    }
}

/// One step of a field path.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Step<'a> {
    /// A named field of an object.
    Field(&'a str),
    /// A named field holding an array, descended into.
    ///
    /// Written `pieces[]`, and read as "every item of `pieces`". A template
    /// carries one exemplar item, so the path resolves against that one and
    /// the meaning applies to all of them.
    Each(&'a str),
}

/// Splits a dotted path into steps.
///
/// `pieces[].shape.shape` is three steps: every item of `pieces`, then its
/// `shape`, then that object's own `shape`.
fn steps(path: &str) -> Option<Vec<Step<'_>>> {
    if path.is_empty() {
        return None;
    }
    path.split('.')
        .map(|segment| {
            let name = segment.strip_suffix("[]");
            match name {
                Some("") => None,
                Some(name) => Some(Step::Each(name)),
                None if segment.is_empty() => None,
                None => Some(Step::Field(segment)),
            }
        })
        .collect()
}

/// What the template holds at `path`, if it holds anything.
///
/// The template is the component at rest, so the value here is what that field
/// looks like when nobody has said otherwise — which makes it two things at
/// once: the check that a path names something real, and the blank a tool uses
/// when it needs to make one. A list's exemplar item is the new item.
#[must_use]
pub fn exemplar<'a>(template: &'a Value, path: &str) -> Option<&'a Value> {
    let mut here = template;
    for step in steps(path)? {
        here = match step {
            Step::Field(name) => here.get(name)?,
            // An empty exemplar describes no item, so a path through it names
            // nothing and is refused rather than assumed.
            Step::Each(name) => here.get(name)?.as_array()?.first()?,
        };
    }
    Some(here)
}

/// Whether `path` names something the template actually has.
///
/// The check that keeps a meaning honest. A renamed field leaves its meaning
/// pointing at nothing, and this turns that into a registration error rather
/// than a picker that quietly stopped appearing.
#[must_use]
pub fn resolves(template: &Value, path: &str) -> bool {
    exemplar(template, path).is_some()
}

/// The meaning stored for `path`, matching an array path against any index.
///
/// A stored `pieces[].friction` answers for `pieces.0.friction` and
/// `pieces.7.friction` alike: the exemplar describes every item.
#[must_use]
pub fn matches(stored: &str, queried: &str) -> bool {
    let (Some(stored), Some(queried)) = (steps(stored), steps(queried)) else {
        return false;
    };
    let mut asked = queried.iter();
    for step in &stored {
        match step {
            Step::Field(name) => {
                if asked.next() != Some(&Step::Field(name)) {
                    return false;
                }
            }
            Step::Each(name) => {
                // A queried path names the item it is actually looking at, so
                // an exemplar's `pieces[]` has to answer for both the spelling
                // that means every item and the one that means the third:
                // `pieces[]` is one step, `pieces.2` is two.
                match asked.next() {
                    Some(Step::Each(queried)) if queried == name => {}
                    Some(Step::Field(queried)) if queried == name => {
                        let Some(Step::Field(index)) = asked.next() else {
                            return false;
                        };
                        if !index.bytes().all(|byte| byte.is_ascii_digit()) {
                            return false;
                        }
                    }
                    _ => return false,
                }
            }
        }
    }
    asked.next().is_none()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_path_resolves_through_objects_and_array_exemplars() {
        let template = json!({
            "pieces": [{ "shape": { "shape": "box" }, "friction": 0.5 }],
            "offset": [0.0, 0.0],
        });
        assert!(resolves(&template, "pieces[].shape.shape"));
        assert!(resolves(&template, "pieces[].friction"));
        assert!(resolves(&template, "offset"));
    }

    /// The check that makes a meaning rot loudly instead of quietly.
    #[test]
    fn a_path_naming_nothing_does_not_resolve() {
        let template = json!({ "pieces": [{ "friction": 0.5 }] });
        assert!(!resolves(&template, "pieces[].restitution"));
        assert!(!resolves(&template, "peices[].friction"));
        assert!(!resolves(&template, ""));
        assert!(!resolves(&template, "pieces[]."));
    }

    /// The exemplar is both the check and the blank: what a list's new item is
    /// made from.
    #[test]
    fn a_lists_exemplar_is_the_item_a_new_one_is_made_from() {
        let template = json!({ "pieces": [{ "friction": 0.5, "sensor": false }] });
        assert_eq!(
            exemplar(&template, "pieces[]"),
            Some(&json!({ "friction": 0.5, "sensor": false }))
        );
        assert_eq!(exemplar(&template, "pieces[].friction"), Some(&json!(0.5)));
        assert_eq!(exemplar(&template, "pieces[].restitution"), None);
    }

    /// An array with no exemplar describes no item, so nothing about one can
    /// be claimed.
    #[test]
    fn an_empty_exemplar_resolves_nothing() {
        let template = json!({ "pieces": [] });
        assert!(!resolves(&template, "pieces[].friction"));
    }

    #[test]
    fn an_exemplar_path_answers_for_every_index() {
        assert!(matches("pieces[].friction", "pieces.0.friction"));
        assert!(matches("pieces[].friction", "pieces.7.friction"));
        assert!(matches("pieces[].friction", "pieces[].friction"));
        assert!(!matches("pieces[].friction", "pieces.0.restitution"));
        assert!(!matches("offset", "pieces.0.offset"));
        // An index is a number. A field that happens to sit where one would is
        // a different field, not the exemplar's.
        assert!(!matches("pieces[].friction", "pieces.friction"));
        assert!(!matches("pieces[].friction", "pieces.name.friction"));
    }
}
