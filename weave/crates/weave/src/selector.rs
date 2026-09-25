//! Selectors: which elements a rule is about, and how strongly it claims them.
//!
//! The same shape as CSS, because the point of Weave is that someone who has
//! styled a web page already knows it: element names, `#id`, `.class`,
//! state pseudo-classes, compound selectors such as `button.primary:hover`,
//! descendant (`.menu text`) and child (`.menu > button`) combinators, and
//! selector lists (`h1, h2`). Specificity is CSS's: IDs, then classes and
//! states, then element names.

use crate::ParseError;

/// The interaction states an element can be in, as pseudo-classes match them.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct States(u8);

impl States {
    pub const NONE: Self = Self(0);
    /// The pointer is over it.
    pub const HOVER: Self = Self(1);
    /// It is being pressed. `:active` in CSS; `:pressed` is accepted too.
    pub const ACTIVE: Self = Self(1 << 1);
    /// It has keyboard or gamepad focus.
    pub const FOCUS: Self = Self(1 << 2);
    /// It does not respond to input.
    pub const DISABLED: Self = Self(1 << 3);
    /// A toggle that is on, or the chosen option.
    pub const CHECKED: Self = Self(1 << 4);

    #[must_use]
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    #[must_use]
    pub const fn with(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }
}

/// How strongly a selector claims an element: IDs, then classes and states,
/// then element names. Compared in that order, as in CSS.
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct Specificity(pub u16, pub u16, pub u16);

/// One element's worth of conditions, with nothing between them:
/// `button.primary:hover`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Compound {
    /// An element name, or `None` for any element (`*`, or none written).
    pub element: Option<String>,
    pub id: Option<String>,
    pub classes: Vec<String>,
    pub states: States,
}

/// What joins two compounds.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Combinator {
    /// Whitespace: anywhere inside.
    Descendant,
    /// `>`: directly inside.
    Child,
}

/// A complex selector: compounds joined by combinators, read left to right.
#[derive(Clone, Debug, PartialEq)]
pub struct Selector {
    pub compounds: Vec<Compound>,
    /// One fewer than `compounds`: `combinators[i]` joins `compounds[i]` and
    /// `compounds[i + 1]`.
    pub combinators: Vec<Combinator>,
}

/// What a selector is matched against: one element of a tree.
#[derive(Clone, Copy, Debug)]
pub struct Element<'a> {
    pub id: &'a str,
    pub classes: &'a [String],
    /// Every name the element answers to, such as `text` and
    /// `sindri.ui.text`.
    pub names: &'a [String],
    pub states: States,
}

/// A tree of elements, addressed by index. The host owns it; Weave only
/// walks it.
pub trait Tree {
    fn element(&self, node: usize) -> Element<'_>;
    fn parent(&self, node: usize) -> Option<usize>;
}

impl Compound {
    fn matches(&self, element: Element<'_>) -> bool {
        self.element
            .as_ref()
            .is_none_or(|name| element.names.iter().any(|known| known == name))
            && self.id.as_ref().is_none_or(|id| id == element.id)
            && self
                .classes
                .iter()
                .all(|class| element.classes.iter().any(|known| known == class))
            && element.states.contains(self.states)
    }

    fn specificity(&self) -> Specificity {
        let states = u16::try_from(self.states.0.count_ones()).unwrap_or(u16::MAX);
        Specificity(
            u16::from(self.id.is_some()),
            u16::try_from(self.classes.len())
                .unwrap_or(u16::MAX)
                .saturating_add(states),
            u16::from(self.element.is_some()),
        )
    }

    fn is_empty(&self) -> bool {
        self.element.is_none() && self.id.is_none() && self.classes.is_empty()
    }
}

impl Selector {
    /// A selector naming one ID.
    #[must_use]
    pub fn id(id: impl Into<String>) -> Self {
        Self::single(Compound {
            id: Some(id.into()),
            ..Compound::default()
        })
    }

    /// A selector naming one class.
    #[must_use]
    pub fn class(class: impl Into<String>) -> Self {
        Self::single(Compound {
            classes: vec![class.into()],
            ..Compound::default()
        })
    }

    /// A selector naming one element.
    #[must_use]
    pub fn element(name: impl Into<String>) -> Self {
        Self::single(Compound {
            element: Some(name.into()),
            ..Compound::default()
        })
    }

    fn single(compound: Compound) -> Self {
        Self {
            compounds: vec![compound],
            combinators: Vec::new(),
        }
    }

    #[must_use]
    pub fn specificity(&self) -> Specificity {
        self.compounds.iter().map(Compound::specificity).fold(
            Specificity::default(),
            |total, part| {
                Specificity(
                    total.0.saturating_add(part.0),
                    total.1.saturating_add(part.1),
                    total.2.saturating_add(part.2),
                )
            },
        )
    }

    /// Whether the element at `node` is one this selector is about.
    ///
    /// Read right to left, as browsers do: the last compound is the element
    /// itself, and each combinator walks up the tree from there.
    #[must_use]
    pub fn matches(&self, tree: &impl Tree, node: usize) -> bool {
        self.compounds
            .len()
            .checked_sub(1)
            .is_some_and(|last| self.match_from(tree, node, last))
    }

    fn match_from(&self, tree: &impl Tree, node: usize, index: usize) -> bool {
        if !self.compounds[index].matches(tree.element(node)) {
            return false;
        }
        let Some(previous) = index.checked_sub(1) else {
            return true;
        };
        match self.combinators[previous] {
            Combinator::Child => tree
                .parent(node)
                .is_some_and(|parent| self.match_from(tree, parent, previous)),
            Combinator::Descendant => {
                let mut ancestor = tree.parent(node);
                // Bounded, so a malformed tree with a cycle cannot hang the
                // cascade; real hierarchies are far shallower.
                for _ in 0..256 {
                    let Some(current) = ancestor else {
                        return false;
                    };
                    if self.match_from(tree, current, previous) {
                        return true;
                    }
                    ancestor = tree.parent(current);
                }
                false
            }
        }
    }
}

/// Parses a selector list, `a, b > c`, into its selectors.
pub(crate) fn parse_list(text: &str) -> Result<Vec<Selector>, ParseError> {
    text.split(',')
        .map(|part| parse_selector(part.trim()))
        .collect()
}

fn parse_selector(text: &str) -> Result<Selector, ParseError> {
    let invalid = || ParseError::InvalidSelector(text.to_owned());
    let chars: Vec<char> = text.chars().collect();
    let mut position = 0;
    let mut compounds = Vec::new();
    let mut combinators = Vec::new();
    loop {
        let compound = parse_compound(&chars, &mut position, text)?;
        if compound.is_empty() && compound.states.is_empty() && !starts_universal(&chars, position)
        {
            return Err(invalid());
        }
        compounds.push(compound);
        let mut saw_space = false;
        while chars.get(position).is_some_and(|c| c.is_whitespace()) {
            saw_space = true;
            position += 1;
        }
        match chars.get(position) {
            None => break,
            Some('>') => {
                position += 1;
                while chars.get(position).is_some_and(|c| c.is_whitespace()) {
                    position += 1;
                }
                combinators.push(Combinator::Child);
            }
            Some(_) if saw_space => combinators.push(Combinator::Descendant),
            Some(_) => return Err(invalid()),
        }
    }
    Ok(Selector {
        compounds,
        combinators,
    })
}

/// Whether the compound just parsed was written as `*`, which is empty but
/// still a selector.
fn starts_universal(chars: &[char], position: usize) -> bool {
    chars[..position]
        .iter()
        .rev()
        .find(|c| !c.is_whitespace())
        .is_some_and(|c| *c == '*')
}

fn parse_compound(
    chars: &[char],
    position: &mut usize,
    text: &str,
) -> Result<Compound, ParseError> {
    let mut compound = Compound::default();
    if chars.get(*position) == Some(&'*') {
        *position += 1;
    } else if chars.get(*position).is_some_and(|c| is_name_start(*c)) {
        let mut name = take_name(chars, position);
        // `sindri.ui.text`, the long form of an element name written before
        // Weave had short ones, is read as one name rather than as the
        // element `sindri` with classes `ui` and `text`.
        if name == "sindri" {
            while chars.get(*position) == Some(&'.')
                && chars.get(*position + 1).is_some_and(|c| is_name_start(*c))
            {
                *position += 1;
                name.push('.');
                name.push_str(&take_name(chars, position));
            }
        }
        compound.element = Some(name);
    }
    loop {
        match chars.get(*position) {
            Some('#') => {
                *position += 1;
                let id = take_name(chars, position);
                if id.is_empty() {
                    return Err(ParseError::InvalidSelector(text.to_owned()));
                }
                compound.id = Some(id);
            }
            Some('.') => {
                *position += 1;
                let class = take_name(chars, position);
                if class.is_empty() {
                    return Err(ParseError::InvalidSelector(text.to_owned()));
                }
                compound.classes.push(class);
            }
            Some(':') => {
                *position += 1;
                let name = take_name(chars, position);
                let state = match name.as_str() {
                    "hover" => States::HOVER,
                    "active" | "pressed" => States::ACTIVE,
                    "focus" => States::FOCUS,
                    "disabled" => States::DISABLED,
                    "checked" => States::CHECKED,
                    _ => return Err(ParseError::UnsupportedPseudoClass(name)),
                };
                compound.states = compound.states.with(state);
            }
            _ => return Ok(compound),
        }
    }
}

const fn is_name_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_' || c == '-' || !c.is_ascii()
}

fn take_name(chars: &[char], position: &mut usize) -> String {
    let start = *position;
    while chars
        .get(*position)
        .is_some_and(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-' || !c.is_ascii())
    {
        *position += 1;
    }
    chars[start..*position].iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One element of the test tree: id, classes, names, states, parent.
    type Node = (String, Vec<String>, Vec<String>, States, Option<usize>);

    /// A tiny tree: a menu holding a row holding a button with text in it.
    struct Menu {
        nodes: Vec<Node>,
    }

    impl Tree for Menu {
        fn element(&self, node: usize) -> Element<'_> {
            let (id, classes, names, states, _) = &self.nodes[node];
            Element {
                id,
                classes,
                names,
                states: *states,
            }
        }

        fn parent(&self, node: usize) -> Option<usize> {
            self.nodes[node].4
        }
    }

    fn strings(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_owned()).collect()
    }

    fn menu() -> Menu {
        Menu {
            nodes: vec![
                (
                    "menu".into(),
                    strings(&["menu"]),
                    strings(&["shape"]),
                    States::NONE,
                    None,
                ),
                (
                    "row".into(),
                    strings(&["row"]),
                    strings(&["layout"]),
                    States::NONE,
                    Some(0),
                ),
                (
                    "play".into(),
                    strings(&["primary"]),
                    strings(&["button", "shape"]),
                    States::HOVER,
                    Some(1),
                ),
                (
                    "label".into(),
                    Vec::new(),
                    strings(&["text", "sindri.ui.text"]),
                    States::NONE,
                    Some(2),
                ),
            ],
        }
    }

    fn matches(selector: &str, node: usize) -> bool {
        let list = parse_list(selector).expect("selector parses");
        list.iter().any(|selector| selector.matches(&menu(), node))
    }

    #[test]
    fn compound_selectors_need_every_part() {
        assert!(matches("button.primary:hover", 2));
        assert!(!matches("button.primary:active", 2));
        assert!(!matches("text.primary", 2));
    }

    #[test]
    fn descendant_and_child_combinators_walk_up() {
        assert!(matches(".menu text", 3));
        assert!(matches(".menu button", 2));
        assert!(!matches(".menu > button", 2), "the row is between them");
        assert!(matches(".row > button > text", 3));
        assert!(matches(".menu > .row", 1));
    }

    #[test]
    fn long_element_names_still_read_as_one_name() {
        assert!(matches("sindri.ui.text", 3));
        assert!(matches(".menu sindri.ui.text", 3));
    }

    #[test]
    fn a_list_matches_if_any_member_does() {
        assert!(matches("image, text", 3));
        assert!(!matches("image, slider", 3));
    }

    #[test]
    fn specificity_is_ids_then_classes_and_states_then_names() {
        let specificity = |text: &str| parse_list(text).unwrap()[0].specificity();
        assert!(specificity("#play") > specificity(".menu .row button.primary:hover"));
        assert!(specificity("button:hover") > specificity("button"));
        assert_eq!(specificity(".menu > button.primary"), Specificity(0, 2, 1));
        assert_eq!(specificity("*"), Specificity(0, 0, 0));
    }

    #[test]
    fn mistakes_are_named() {
        assert_eq!(
            parse_list("button:hovered"),
            Err(ParseError::UnsupportedPseudoClass("hovered".into()))
        );
        assert!(matches!(
            parse_list(".menu >"),
            Err(ParseError::InvalidSelector(_))
        ));
    }
}
