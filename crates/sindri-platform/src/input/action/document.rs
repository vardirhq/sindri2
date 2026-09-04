//! Reading and writing the actions a project declares.
//!
//! A list rather than an object, because the order is data: an [`ActionId`] is
//! an index into the declared order, and a format that let two saves of the
//! same file come back in different orders would hand out different ids for the
//! same project.
//!
//! [`ActionId`]: super::ActionId

use serde_json::{Map, Value as Json};

use super::binding::{Binding, Source};
use super::map::{ActionKind, ActionMap, ActionMapError};

/// The version this build writes, and the only one it reads.
pub const ACTIONS_FORMAT_VERSION: u64 = 1;

/// What a project's actions file is called.
pub const ACTIONS_SUFFIX: &str = ".actions.json";

/// Why an actions file could not be read.
///
/// Each variant names the action it was reading where it can, because "invalid
/// binding" in a file with thirty of them is a hunt rather than a fix.
#[derive(Debug, thiserror::Error, PartialEq)]
pub enum ActionsDocumentError {
    #[error("the actions file is not JSON: {0}")]
    Json(String),
    #[error("the actions file should be an object with an 'actions' list")]
    Shape,
    #[error(
        "the actions file is version {found}, and this build reads version {ACTIONS_FORMAT_VERSION}"
    )]
    Version { found: u64 },
    #[error("an entry in the actions list is not an object")]
    Entry,
    #[error("an action has no name")]
    Unnamed,
    #[error("action '{name}' has no kind; it should be one of button, axis, or vector")]
    NoKind { name: String },
    #[error("action '{name}' has kind '{kind}', which is not one of button, axis, or vector")]
    UnknownKind { name: String, kind: String },
    #[error("action '{name}' has no bindings list")]
    NoBindings { name: String },
    #[error("action '{name}' has a binding this build does not understand: {binding}")]
    Binding { name: String, binding: String },
    // `named` rather than `source`: a field of that name is what `thiserror`
    // reads as the error's cause, and this one is a piece of text from a file.
    #[error("action '{name}' names a source this build has never heard of: '{named}'")]
    UnknownSource { name: String, named: String },
    #[error("{0}")]
    Refused(#[from] ActionMapError),
}

impl ActionMap {
    /// Reads a project's declared actions.
    pub fn from_json(text: &str) -> Result<Self, ActionsDocumentError> {
        let document: Json = serde_json::from_str(text)
            .map_err(|error| ActionsDocumentError::Json(error.to_string()))?;
        let Some(object) = document.as_object() else {
            return Err(ActionsDocumentError::Shape);
        };

        // A missing version means the first version, so a file written by hand
        // before anyone thought about versioning still loads.
        let version = object
            .get("version")
            .map_or(Some(ACTIONS_FORMAT_VERSION), Json::as_u64)
            .ok_or(ActionsDocumentError::Shape)?;
        if version != ACTIONS_FORMAT_VERSION {
            return Err(ActionsDocumentError::Version { found: version });
        }

        let Some(entries) = object.get("actions").and_then(Json::as_array) else {
            return Err(ActionsDocumentError::Shape);
        };

        let mut map = Self::default();
        for entry in entries {
            let entry = entry.as_object().ok_or(ActionsDocumentError::Entry)?;
            let name = entry
                .get("name")
                .and_then(Json::as_str)
                .ok_or(ActionsDocumentError::Unnamed)?;
            let kind = read_kind(name, entry)?;
            let bindings = read_bindings(name, entry)?;
            // Straight through `declare`, so a file cannot express an action
            // that code could not: the refusals are the same either way.
            map.declare(name, kind, bindings)?;
        }
        Ok(map)
    }

    /// Writes the actions back out, in the order they were declared.
    #[must_use]
    pub fn to_json(&self) -> String {
        let actions: Vec<Json> = self
            .iter()
            .map(|action| {
                Json::Object(Map::from_iter([
                    ("name".to_owned(), Json::String(action.name.clone())),
                    (
                        "kind".to_owned(),
                        Json::String(action.kind.name().to_owned()),
                    ),
                    (
                        "bindings".to_owned(),
                        Json::Array(action.bindings.iter().map(binding_json).collect()),
                    ),
                ]))
            })
            .collect();
        let document = Json::Object(Map::from_iter([
            (
                "version".to_owned(),
                Json::Number(ACTIONS_FORMAT_VERSION.into()),
            ),
            ("actions".to_owned(), Json::Array(actions)),
        ]));
        serde_json::to_string_pretty(&document).unwrap_or_else(|_| "{}".to_owned())
    }
}

fn read_kind(name: &str, entry: &Map<String, Json>) -> Result<ActionKind, ActionsDocumentError> {
    let kind =
        entry
            .get("kind")
            .and_then(Json::as_str)
            .ok_or_else(|| ActionsDocumentError::NoKind {
                name: name.to_owned(),
            })?;
    ActionKind::from_name(kind).ok_or_else(|| ActionsDocumentError::UnknownKind {
        name: name.to_owned(),
        kind: kind.to_owned(),
    })
}

fn read_bindings(
    name: &str,
    entry: &Map<String, Json>,
) -> Result<Vec<Binding>, ActionsDocumentError> {
    let entries = entry
        .get("bindings")
        .and_then(Json::as_array)
        .ok_or_else(|| ActionsDocumentError::NoBindings {
            name: name.to_owned(),
        })?;
    entries
        .iter()
        .map(|binding| read_binding(name, binding))
        .collect()
}

/// One binding, in either of the two shapes a file may write.
///
/// A plain string for the common case, because `"key.Space"` is what somebody
/// editing this by hand wants to type and `{"simple": "key.Space"}` is
/// ceremony. Composites are objects, since they have parts to name.
fn read_binding(name: &str, binding: &Json) -> Result<Binding, ActionsDocumentError> {
    if let Some(source) = binding.as_str() {
        return Ok(Binding::Simple(read_source(name, source)?));
    }
    let Some(object) = binding.as_object() else {
        return Err(ActionsDocumentError::Binding {
            name: name.to_owned(),
            binding: binding.to_string(),
        });
    };

    if let Some(axis) = object.get("axis").and_then(Json::as_object) {
        return Ok(Binding::Axis {
            negative: part(name, axis, "negative")?,
            positive: part(name, axis, "positive")?,
        });
    }
    if let Some(vector) = object.get("vector").and_then(Json::as_object) {
        return Ok(Binding::Vector {
            up: part(name, vector, "up")?,
            down: part(name, vector, "down")?,
            left: part(name, vector, "left")?,
            right: part(name, vector, "right")?,
        });
    }
    Err(ActionsDocumentError::Binding {
        name: name.to_owned(),
        binding: binding.to_string(),
    })
}

fn part(name: &str, object: &Map<String, Json>, key: &str) -> Result<Source, ActionsDocumentError> {
    let source =
        object
            .get(key)
            .and_then(Json::as_str)
            .ok_or_else(|| ActionsDocumentError::Binding {
                name: name.to_owned(),
                binding: format!("a composite with no '{key}'"),
            })?;
    read_source(name, source)
}

/// A source name, refused rather than dropped if nothing answers to it.
///
/// Dropping it would leave an action bound to fewer things than its file says,
/// which is a control that half works for a reason no one can see in the file.
fn read_source(name: &str, source: &str) -> Result<Source, ActionsDocumentError> {
    Source::from_name(source).ok_or_else(|| ActionsDocumentError::UnknownSource {
        name: name.to_owned(),
        named: source.to_owned(),
    })
}

fn binding_json(binding: &Binding) -> Json {
    let composite = |key: &str, parts: Vec<(&str, Source)>| {
        Json::Object(Map::from_iter([(
            key.to_owned(),
            Json::Object(
                parts
                    .into_iter()
                    .map(|(name, source)| (name.to_owned(), Json::String(source.name())))
                    .collect(),
            ),
        )]))
    };
    match binding {
        Binding::Simple(source) => Json::String(source.name()),
        Binding::Axis { negative, positive } => composite(
            "axis",
            vec![("negative", *negative), ("positive", *positive)],
        ),
        Binding::Vector {
            up,
            down,
            left,
            right,
        } => composite(
            "vector",
            vec![
                ("up", *up),
                ("down", *down),
                ("left", *left),
                ("right", *right),
            ],
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::{ACTIONS_FORMAT_VERSION, ActionsDocumentError};
    use crate::input::Key;
    use crate::input::action::binding::{Binding, Source};
    use crate::input::action::map::{ActionKind, ActionMap, ActionMapError};

    const PROJECT: &str = r#"{
      "version": 1,
      "actions": [
        { "name": "move", "kind": "vector", "bindings": [
            { "vector": { "up": "key.W", "down": "key.S", "left": "key.A", "right": "key.D" } }
        ] },
        { "name": "turn", "kind": "axis", "bindings": [
            { "axis": { "negative": "key.ArrowLeft", "positive": "key.ArrowRight" } }
        ] },
        { "name": "fire", "kind": "button", "bindings": ["key.Space", "mouse.Left"] }
      ]
    }"#;

    #[test]
    fn a_project_declares_its_actions() {
        let map = ActionMap::from_json(PROJECT).expect("the project's actions load");
        assert_eq!(map.len(), 3);
        assert_eq!(
            map.kind(map.id("move").expect("move")),
            Some(ActionKind::Vector)
        );
        assert_eq!(
            map.kind(map.id("fire").expect("fire")),
            Some(ActionKind::Button)
        );
    }

    #[test]
    fn the_order_in_the_file_is_the_order_of_the_ids() {
        // A list, not an object, precisely so this holds: an id is an index, and
        // a format whose order could vary between saves would hand out
        // different ids for the same project.
        let map = ActionMap::from_json(PROJECT).expect("loads");
        assert_eq!(map.names().collect::<Vec<_>>(), ["move", "turn", "fire"]);
        assert_eq!(map.id("move").expect("move").index(), 0);
        assert_eq!(map.id("fire").expect("fire").index(), 2);
    }

    #[test]
    fn a_simple_binding_may_be_written_as_plain_text() {
        // `"key.Space"` is what somebody editing this by hand wants to type.
        let map = ActionMap::from_json(PROJECT).expect("loads");
        assert_eq!(map.conflicts(), vec![]);
        let reloaded = ActionMap::from_json(&map.to_json()).expect("reloads");
        assert_eq!(
            reloaded.names().collect::<Vec<_>>(),
            ["move", "turn", "fire"]
        );
    }

    #[test]
    fn what_is_written_is_what_comes_back() {
        // A file the editor saves must load to the same actions, or rebinding
        // through an interface quietly loses controls.
        let mut map = ActionMap::default();
        map.declare(
            "move",
            ActionKind::Vector,
            vec![Binding::Vector {
                up: Source::Key(Key::W),
                down: Source::Key(Key::S),
                left: Source::Key(Key::A),
                right: Source::Key(Key::D),
            }],
        )
        .expect("declared");
        map.declare(
            "fire",
            ActionKind::Button,
            vec![Binding::Simple(Source::Key(Key::Space))],
        )
        .expect("declared");

        let text = map.to_json();
        let reloaded = ActionMap::from_json(&text).expect("reloads");
        assert_eq!(reloaded.to_json(), text, "a second trip changes nothing");
    }

    #[test]
    fn a_source_nobody_knows_is_refused_rather_than_dropped() {
        // Dropping it would leave an action bound to fewer things than its file
        // says: a control that half works for a reason not visible in the file.
        let error = ActionMap::from_json(
            r#"{"actions":[{"name":"fire","kind":"button","bindings":["gamepad.South"]}]}"#,
        )
        .expect_err("an unknown source is refused");
        assert_eq!(
            error,
            ActionsDocumentError::UnknownSource {
                name: "fire".to_owned(),
                named: "gamepad.South".to_owned(),
            }
        );
    }

    #[test]
    fn a_file_cannot_declare_what_code_could_not() {
        // Straight through `declare`, so the refusals are the same either way
        // and a file is not a way around them.
        let error = ActionMap::from_json(
            r#"{"actions":[{"name":"move","kind":"vector","bindings":["key.W"]}]}"#,
        )
        .expect_err("a vector needs four directions");
        assert!(
            matches!(
                error,
                ActionsDocumentError::Refused(ActionMapError::Mismatched { .. })
            ),
            "{error}"
        );
    }

    #[test]
    fn a_version_this_build_cannot_read_is_said_so() {
        let error = ActionMap::from_json(r#"{"version":99,"actions":[]}"#)
            .expect_err("a future version is refused");
        assert_eq!(error, ActionsDocumentError::Version { found: 99 });
    }

    #[test]
    fn a_missing_version_is_the_first_one() {
        // A file written by hand before anyone thought about versioning still
        // loads, rather than failing on a field nobody knew to write.
        assert_eq!(ACTIONS_FORMAT_VERSION, 1);
        let map = ActionMap::from_json(r#"{"actions":[]}"#).expect("loads");
        assert!(map.is_empty());
    }

    #[test]
    fn an_unreadable_file_says_what_is_wrong_with_it() {
        assert!(matches!(
            ActionMap::from_json("not json"),
            Err(ActionsDocumentError::Json(_))
        ));
        let refused = |text: &str| ActionMap::from_json(text).expect_err("refused");
        assert_eq!(refused("[]"), ActionsDocumentError::Shape);
        assert_eq!(
            refused(r#"{"actions":[{"kind":"button","bindings":["key.W"]}]}"#),
            ActionsDocumentError::Unnamed
        );
        assert_eq!(
            refused(r#"{"actions":[{"name":"fire","bindings":["key.W"]}]}"#),
            ActionsDocumentError::NoKind {
                name: "fire".to_owned()
            }
        );
    }
}
