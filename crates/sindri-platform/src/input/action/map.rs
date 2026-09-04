//! The actions a project declares, and what each is bound to.

use std::collections::BTreeMap;

use super::binding::{Binding, Source};

/// What shape of answer an action gives.
///
/// Declared rather than inferred from the bindings, because the declaration is
/// the contract gameplay is written against: a `move` that is a vector stays a
/// vector when someone rebinds it from WASD to a stick, and a script asking for
/// its `x` keeps compiling.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ActionKind {
    /// Down or not.
    Button,
    /// One number, normally -1 to 1.
    Axis,
    /// Two, normally inside the unit circle.
    Vector,
}

impl ActionKind {
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Button => "button",
            Self::Axis => "axis",
            Self::Vector => "vector",
        }
    }

    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "button" => Some(Self::Button),
            "axis" => Some(Self::Axis),
            "vector" => Some(Self::Vector),
            _ => None,
        }
    }
}

/// One action, resolved.
///
/// An index rather than a name, handed out by the map when it is built. The
/// point is that looking up an action can fail *once*, at load, instead of
/// silently every frame: a name nobody declared is caught while the project is
/// opening rather than becoming a control that does nothing all game.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ActionId(usize);

impl ActionId {
    #[must_use]
    pub const fn index(self) -> usize {
        self.0
    }
}

#[derive(Clone, Debug)]
pub(super) struct Action {
    pub(super) name: String,
    pub(super) kind: ActionKind,
    pub(super) bindings: Vec<Binding>,
}

/// What went wrong building a map.
#[derive(Debug, thiserror::Error, PartialEq)]
pub enum ActionMapError {
    #[error("two actions are both named '{name}'")]
    Duplicate { name: String },
    #[error("action '{name}' is a {kind} but is bound to {binding}, which cannot make one")]
    Mismatched {
        name: String,
        kind: &'static str,
        binding: &'static str,
    },
    #[error("action '{name}' has no bindings, so nothing can ever trigger it")]
    Unbound { name: String },
}

/// Every action a project declares.
#[derive(Clone, Debug, Default)]
pub struct ActionMap {
    actions: Vec<Action>,
    by_name: BTreeMap<String, ActionId>,
}

impl ActionMap {
    /// Adds an action, refusing anything that could not work.
    ///
    /// The refusals are the point. An action bound to something that cannot
    /// produce its kind, or bound to nothing at all, is a control that will
    /// never respond -- and finding that out while playing, rather than while
    /// loading, is the experience this whole module exists to avoid.
    pub fn declare(
        &mut self,
        name: &str,
        kind: ActionKind,
        bindings: Vec<Binding>,
    ) -> Result<ActionId, ActionMapError> {
        if self.by_name.contains_key(name) {
            return Err(ActionMapError::Duplicate {
                name: name.to_owned(),
            });
        }
        if bindings.is_empty() {
            return Err(ActionMapError::Unbound {
                name: name.to_owned(),
            });
        }
        for binding in &bindings {
            if !suits(kind, binding) {
                return Err(ActionMapError::Mismatched {
                    name: name.to_owned(),
                    kind: kind.name(),
                    binding: shape_of(binding),
                });
            }
        }

        let id = ActionId(self.actions.len());
        self.actions.push(Action {
            name: name.to_owned(),
            kind,
            bindings,
        });
        self.by_name.insert(name.to_owned(), id);
        Ok(id)
    }

    /// The action of this name, if it was declared.
    #[must_use]
    pub fn id(&self, name: &str) -> Option<ActionId> {
        self.by_name.get(name).copied()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.actions.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.actions.is_empty()
    }

    /// Every action's name, in the order they were declared.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.actions.iter().map(|action| action.name.as_str())
    }

    #[must_use]
    pub fn kind(&self, id: ActionId) -> Option<ActionKind> {
        self.actions.get(id.index()).map(|action| action.kind)
    }

    #[must_use]
    pub fn name(&self, id: ActionId) -> Option<&str> {
        self.actions
            .get(id.index())
            .map(|action| action.name.as_str())
    }

    pub(super) fn iter(&self) -> impl Iterator<Item = &Action> {
        self.actions.iter()
    }

    /// Replaces what an action is bound to, as a rebinding interface does.
    ///
    /// The same refusals as declaring: rebinding is not an escape hatch around
    /// the checks, or a person could rebind `move` to a single key and have a
    /// direction that only ever points one way.
    pub fn rebind(&mut self, id: ActionId, bindings: Vec<Binding>) -> Result<(), ActionMapError> {
        let Some(action) = self.actions.get(id.index()) else {
            return Ok(());
        };
        let (name, kind) = (action.name.clone(), action.kind);
        if bindings.is_empty() {
            return Err(ActionMapError::Unbound { name });
        }
        for binding in &bindings {
            if !suits(kind, binding) {
                return Err(ActionMapError::Mismatched {
                    name,
                    kind: kind.name(),
                    binding: shape_of(binding),
                });
            }
        }
        self.actions[id.index()].bindings = bindings;
        Ok(())
    }

    /// Actions that would both answer to the same source.
    ///
    /// For a rebinding interface to show before it commits, rather than after
    /// someone discovers that jumping also opens the map. Reported as pairs
    /// with the source they share, in a stable order.
    #[must_use]
    pub fn conflicts(&self) -> Vec<(&str, &str, Source)> {
        let mut found = Vec::new();
        for (index, action) in self.actions.iter().enumerate() {
            for other in self.actions.iter().skip(index + 1) {
                for source in action.bindings.iter().flat_map(Binding::sources) {
                    if other
                        .bindings
                        .iter()
                        .flat_map(Binding::sources)
                        .any(|theirs| theirs == source)
                    {
                        found.push((action.name.as_str(), other.name.as_str(), source));
                    }
                }
            }
        }
        found
    }
}

/// Whether a binding can make the kind of value an action promises.
const fn suits(kind: ActionKind, binding: &Binding) -> bool {
    match (kind, binding) {
        // A button reads one source: held or not.
        (ActionKind::Button, Binding::Simple(_))
        // An axis is either one measured source or two opposed pressed ones.
        | (ActionKind::Axis, Binding::Simple(_) | Binding::Axis { .. })
        // A vector needs four, or it is not a direction.
        | (ActionKind::Vector, Binding::Vector { .. }) => true,
        _ => false,
    }
}

const fn shape_of(binding: &Binding) -> &'static str {
    match binding {
        Binding::Simple(_) => "one source",
        Binding::Axis { .. } => "an opposed pair",
        Binding::Vector { .. } => "four directions",
    }
}

#[cfg(test)]
mod tests {
    use super::{ActionKind, ActionMap, ActionMapError};
    use crate::input::action::binding::{Binding, Source};
    use crate::input::{Key, MouseButton};

    fn wasd() -> Binding {
        Binding::Vector {
            up: Source::Key(Key::W),
            down: Source::Key(Key::S),
            left: Source::Key(Key::A),
            right: Source::Key(Key::D),
        }
    }

    #[test]
    fn an_action_is_found_by_name_once_and_by_id_thereafter() {
        let mut map = ActionMap::default();
        let id = map
            .declare("move", ActionKind::Vector, vec![wasd()])
            .expect("declared");
        assert_eq!(map.id("move"), Some(id));
        assert_eq!(map.name(id), Some("move"));
        assert_eq!(map.kind(id), Some(ActionKind::Vector));
    }

    #[test]
    fn a_name_nobody_declared_is_nothing() {
        // The whole reason an id exists: this fails once, where a project is
        // loaded, rather than every frame in silence.
        let map = ActionMap::default();
        assert_eq!(map.id("jump"), None);
    }

    #[test]
    fn an_action_bound_to_nothing_is_refused() {
        let mut map = ActionMap::default();
        assert_eq!(
            map.declare("jump", ActionKind::Button, vec![]),
            Err(ActionMapError::Unbound {
                name: "jump".to_owned()
            })
        );
    }

    #[test]
    fn a_vector_cannot_be_bound_to_one_key() {
        // A direction from a single key points one way for ever, which is not
        // a direction. Refused where it is written rather than discovered by
        // walking into a wall.
        let mut map = ActionMap::default();
        let error = map
            .declare(
                "move",
                ActionKind::Vector,
                vec![Binding::Simple(Source::Key(Key::W))],
            )
            .expect_err("a vector needs four directions");
        assert!(
            matches!(error, ActionMapError::Mismatched { .. }),
            "{error}"
        );
    }

    #[test]
    fn a_button_cannot_be_bound_to_a_direction() {
        let mut map = ActionMap::default();
        assert!(
            map.declare("jump", ActionKind::Button, vec![wasd()])
                .is_err()
        );
    }

    #[test]
    fn two_actions_of_one_name_are_refused() {
        let mut map = ActionMap::default();
        map.declare(
            "jump",
            ActionKind::Button,
            vec![Binding::Simple(Source::Key(Key::Space))],
        )
        .expect("declared");
        assert_eq!(
            map.declare(
                "jump",
                ActionKind::Button,
                vec![Binding::Simple(Source::Key(Key::Enter))]
            ),
            Err(ActionMapError::Duplicate {
                name: "jump".to_owned()
            })
        );
    }

    #[test]
    fn rebinding_keeps_the_kind_it_promised() {
        let mut map = ActionMap::default();
        let id = map
            .declare("move", ActionKind::Vector, vec![wasd()])
            .expect("declared");
        assert!(
            map.rebind(id, vec![Binding::Simple(Source::Key(Key::W))])
                .is_err(),
            "a rebind is not a way around the checks"
        );
        assert!(
            map.rebind(
                id,
                vec![Binding::Vector {
                    up: Source::Key(Key::ArrowUp),
                    down: Source::Key(Key::ArrowDown),
                    left: Source::Key(Key::ArrowLeft),
                    right: Source::Key(Key::ArrowRight),
                }]
            )
            .is_ok()
        );
    }

    #[test]
    fn two_actions_on_one_key_are_reported_before_anyone_plays_them() {
        let mut map = ActionMap::default();
        map.declare(
            "jump",
            ActionKind::Button,
            vec![Binding::Simple(Source::Key(Key::Space))],
        )
        .expect("declared");
        map.declare(
            "fire",
            ActionKind::Button,
            vec![
                Binding::Simple(Source::Key(Key::Space)),
                Binding::Simple(Source::MouseButton(MouseButton::Left)),
            ],
        )
        .expect("declared");

        let conflicts = map.conflicts();
        assert_eq!(conflicts.len(), 1, "{conflicts:?}");
        assert_eq!(conflicts[0].0, "jump");
        assert_eq!(conflicts[0].1, "fire");
        assert_eq!(conflicts[0].2, Source::Key(Key::Space));
    }

    #[test]
    fn separate_keys_do_not_conflict() {
        let mut map = ActionMap::default();
        map.declare(
            "jump",
            ActionKind::Button,
            vec![Binding::Simple(Source::Key(Key::Space))],
        )
        .expect("declared");
        map.declare("move", ActionKind::Vector, vec![wasd()])
            .expect("declared");
        assert!(map.conflicts().is_empty());
    }
}
