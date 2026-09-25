//! The cascade: which declaration wins for one element, what it inherits, and
//! what its `var()`s stand for.
//!
//! The rules are CSS's, because Weave is meant to be written by people who
//! know CSS: the most specific matching selector wins, source order breaks a
//! tie, text properties and custom properties inherit from the parent
//! element, `inherit` asks for the parent's value outright, and a custom
//! property such as `--accent` is substituted wherever `var(--accent)` is
//! written.

use std::collections::BTreeMap;

use crate::{Specificity, Stylesheet, Tree, Viewport};

/// Properties an element takes from its parent when it says nothing itself.
///
/// The text properties, as in CSS: colour a panel and every label in it is
/// that colour, unless a label says otherwise.
pub const INHERITED: [&str; 9] = [
    "color",
    "font-size",
    "font-weight",
    "line-height",
    "letter-spacing",
    "text-align",
    "text-transform",
    "text-wrap",
    "font-style",
];

/// How deep `var()` may nest before it is taken to be a cycle.
const MAX_SUBSTITUTION_DEPTH: usize = 16;

/// Every property that applies to one element, after the cascade.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Computed {
    values: BTreeMap<String, String>,
}

impl Computed {
    #[must_use]
    pub fn get(&self, property: &str) -> Option<&str> {
        self.values.get(property).map(String::as_str)
    }

    /// Every property and its value, custom properties included.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.values
            .iter()
            .map(|(property, value)| (property.as_str(), value.as_str()))
    }

    /// Every property the host should apply: custom properties are values
    /// for `var()` to use, not something to draw.
    pub fn declarations(&self) -> impl Iterator<Item = (&str, &str)> {
        self.iter().filter(|(property, _)| !is_custom(property))
    }
}

fn is_custom(property: &str) -> bool {
    property.starts_with("--")
}

fn inherits(property: &str) -> bool {
    is_custom(property) || INHERITED.contains(&property)
}

/// One rule that matched an element, as a devtools panel lists it.
#[derive(Clone, Debug, PartialEq)]
pub struct Matched {
    /// The rule's index in the stylesheet.
    pub rule: usize,
    pub specificity: Specificity,
    /// Each declaration, and whether it is the one that applies: `false`
    /// when a stronger rule declares the same property.
    pub declarations: Vec<(String, String, bool)>,
}

/// Every rule that matches the element at `node`, strongest first — the
/// order devtools list them in — with which of their declarations won.
#[must_use]
pub fn matched(
    stylesheet: &Stylesheet,
    tree: &impl Tree,
    node: usize,
    viewport: Viewport,
) -> Vec<Matched> {
    let applying: Vec<(usize, Specificity)> = stylesheet
        .rules
        .iter()
        .enumerate()
        .filter(|(_, rule)| {
            rule.conditions.iter().all(|query| query.matches(viewport))
                && rule.selector.matches(tree, node)
        })
        .map(|(order, rule)| (order, rule.selector.specificity()))
        .collect();
    // The winner for each property: the strongest rule that declares it.
    let mut winners: BTreeMap<&str, (Specificity, usize)> = BTreeMap::new();
    for (order, specificity) in &applying {
        for property in stylesheet.rules[*order].declarations.keys() {
            let replace = winners
                .get(property.as_str())
                .is_none_or(|held| (*specificity, *order) >= *held);
            if replace {
                winners.insert(property, (*specificity, *order));
            }
        }
    }
    let mut listed: Vec<Matched> = applying
        .iter()
        .map(|(order, specificity)| Matched {
            rule: *order,
            specificity: *specificity,
            declarations: stylesheet.rules[*order]
                .declarations
                .iter()
                .map(|(property, value)| {
                    let won = winners
                        .get(property.as_str())
                        .is_some_and(|(_, winner)| winner == order);
                    (property.clone(), value.clone(), won)
                })
                .collect(),
        })
        .collect();
    listed.sort_by_key(|rule| std::cmp::Reverse((rule.specificity, rule.rule)));
    listed
}

/// Computes the style of the element at `node`, given its parent's.
///
/// Parents must be computed before their children, so the caller walks the
/// tree top-down and hands each child its parent's result.
#[must_use]
pub fn cascade(
    stylesheet: &Stylesheet,
    tree: &impl Tree,
    node: usize,
    viewport: Viewport,
    parent: Option<&Computed>,
) -> Computed {
    let mut winners: BTreeMap<&str, (Specificity, usize, &str)> = BTreeMap::new();
    for (order, rule) in stylesheet.rules.iter().enumerate() {
        if !rule.conditions.iter().all(|query| query.matches(viewport))
            || !rule.selector.matches(tree, node)
        {
            continue;
        }
        let specificity = rule.selector.specificity();
        for (property, value) in &rule.declarations {
            let replace = winners
                .get(property.as_str())
                .is_none_or(|(held, held_order, _)| (specificity, order) >= (*held, *held_order));
            if replace {
                winners.insert(property, (specificity, order, value));
            }
        }
    }

    let mut values: BTreeMap<String, String> = parent
        .map(|parent| {
            parent
                .values
                .iter()
                .filter(|(property, _)| inherits(property))
                .map(|(property, value)| (property.clone(), value.clone()))
                .collect()
        })
        .unwrap_or_default();
    for (property, (_, _, value)) in winners {
        let inherited = || {
            parent
                .and_then(|parent| parent.get(property))
                .map(str::to_owned)
        };
        match value.trim() {
            "inherit" => match inherited() {
                Some(value) => {
                    values.insert(property.to_owned(), value);
                }
                None => {
                    values.remove(property);
                }
            },
            // `unset` inherits for a property that inherits, which the copy
            // above has already done.
            "unset" if inherits(property) => {}
            // Back to the host's own value.
            "initial" | "unset" => {
                values.remove(property);
            }
            written => {
                values.insert(property.to_owned(), written.to_owned());
            }
        }
    }

    substitute_all(&mut values);
    Computed { values }
}

/// Replaces every `var()` with what it names. A value naming a custom
/// property nothing defined, with no fallback, is dropped: as in CSS, the
/// declaration is invalid and the property falls back to its default.
fn substitute_all(values: &mut BTreeMap<String, String>) {
    let custom: BTreeMap<String, String> = values
        .iter()
        .filter(|(property, _)| is_custom(property))
        .map(|(property, value)| (property.clone(), value.clone()))
        .collect();
    let properties: Vec<String> = values.keys().cloned().collect();
    for property in properties {
        let value = &values[&property];
        if !value.contains("var(") {
            continue;
        }
        match substitute(value, &custom, 0) {
            Some(resolved) => {
                values.insert(property, resolved);
            }
            None => {
                values.remove(&property);
            }
        }
    }
}

fn substitute(value: &str, custom: &BTreeMap<String, String>, depth: usize) -> Option<String> {
    if depth > MAX_SUBSTITUTION_DEPTH {
        return None;
    }
    let Some(start) = value.find("var(") else {
        return Some(value.to_owned());
    };
    let inner_start = start + "var(".len();
    let inner_end = matching_close(&value[inner_start..])? + inner_start;
    let inner = &value[inner_start..inner_end];
    let (name, fallback) = match split_top_level_comma(inner) {
        Some((name, fallback)) => (name.trim(), Some(fallback.trim())),
        None => (inner.trim(), None),
    };
    let replacement = match custom.get(name) {
        Some(defined) => substitute(defined, custom, depth + 1)?,
        None => substitute(fallback?, custom, depth + 1)?,
    };
    let rest = substitute(&value[inner_end + 1..], custom, depth)?;
    Some(format!("{}{replacement}{rest}", &value[..start]))
}

/// Where the parenthesis that closes the one already opened is.
fn matching_close(text: &str) -> Option<usize> {
    let mut depth = 1_usize;
    for (index, character) in text.char_indices() {
        match character {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(index);
                }
            }
            _ => {}
        }
    }
    None
}

fn split_top_level_comma(text: &str) -> Option<(&str, &str)> {
    let mut depth = 0_usize;
    for (index, character) in text.char_indices() {
        match character {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => return Some((&text[..index], &text[index + 1..])),
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Element, States, parse};

    /// A panel with a label inside it.
    struct Panel {
        label_states: States,
    }

    const PANEL: [&str; 1] = ["panel"];

    impl Tree for Panel {
        fn element(&self, node: usize) -> Element<'_> {
            static PANEL_CLASSES: std::sync::LazyLock<Vec<String>> =
                std::sync::LazyLock::new(|| PANEL.iter().map(|c| (*c).to_owned()).collect());
            static SHAPE: std::sync::LazyLock<Vec<String>> =
                std::sync::LazyLock::new(|| vec!["shape".to_owned()]);
            static TEXT: std::sync::LazyLock<Vec<String>> =
                std::sync::LazyLock::new(|| vec!["text".to_owned()]);
            if node == 0 {
                Element {
                    id: "panel",
                    classes: &PANEL_CLASSES,
                    names: &SHAPE,
                    states: States::NONE,
                }
            } else {
                Element {
                    id: "label",
                    classes: &[],
                    names: &TEXT,
                    states: self.label_states,
                }
            }
        }

        fn parent(&self, node: usize) -> Option<usize> {
            (node == 1).then_some(0)
        }
    }

    const VIEW: Viewport = Viewport {
        width: 1280.0,
        height: 720.0,
    };

    fn label(sheet: &str, states: States) -> Computed {
        let sheet = parse(sheet).expect("stylesheet parses");
        let tree = Panel {
            label_states: states,
        };
        let panel = cascade(&sheet, &tree, 0, VIEW, None);
        cascade(&sheet, &tree, 1, VIEW, Some(&panel))
    }

    #[test]
    fn text_properties_inherit_and_boxes_do_not() {
        let style = label(
            ".panel { color: #fff; width: 400px; font-size: 20px; }",
            States::NONE,
        );
        assert_eq!(style.get("color"), Some("#fff"));
        assert_eq!(style.get("font-size"), Some("20px"));
        assert_eq!(style.get("width"), None);
    }

    #[test]
    fn a_declared_value_beats_an_inherited_one() {
        let style = label(
            ".panel { color: #fff; } text { color: #000; }",
            States::NONE,
        );
        assert_eq!(style.get("color"), Some("#000"));
    }

    #[test]
    fn inherit_asks_for_the_parent_even_for_boxes() {
        let style = label(
            ".panel { width: 400px; } text { width: inherit; }",
            States::NONE,
        );
        assert_eq!(style.get("width"), Some("400px"));
    }

    #[test]
    fn variables_inherit_and_substitute_with_fallbacks() {
        let sheet = "
            .panel { --accent: #ff8800; --size: 18px; }
            text { color: var(--accent); font-size: var(--size); border-color: var(--missing, #123456); }
        ";
        let style = label(sheet, States::NONE);
        assert_eq!(style.get("color"), Some("#ff8800"));
        assert_eq!(style.get("font-size"), Some("18px"));
        assert_eq!(style.get("border-color"), Some("#123456"));
        assert!(
            style
                .declarations()
                .all(|(property, _)| !property.starts_with("--")),
            "custom properties are values, not declarations"
        );
    }

    #[test]
    fn a_variable_nobody_defined_drops_the_declaration() {
        let style = label("text { color: var(--nowhere); }", States::NONE);
        assert_eq!(style.get("color"), None);
    }

    #[test]
    fn a_cycle_of_variables_does_not_hang() {
        let style = label(
            "text { --a: var(--b); --b: var(--a); color: var(--a); }",
            States::NONE,
        );
        assert_eq!(style.get("color"), None);
    }

    #[test]
    fn states_select_rules_only_while_they_hold() {
        let sheet = "text { color: #000; } text:hover { color: #f00; }";
        assert_eq!(label(sheet, States::NONE).get("color"), Some("#000"));
        assert_eq!(label(sheet, States::HOVER).get("color"), Some("#f00"));
    }

    #[test]
    fn matched_lists_the_strongest_rule_first_and_marks_what_lost() {
        let sheet = parse(
            "text { color: #111; width: 1px; }\n\
             #label { color: #222; }\n\
             text:hover { color: #333; }\n\
             .elsewhere { color: #444; }",
        )
        .expect("stylesheet parses");
        let tree = Panel {
            label_states: States::NONE,
        };
        let rules = matched(&sheet, &tree, 1, VIEW);
        // The ID rule, then the element rule; the hover and class rules do
        // not match this label at all.
        let order: Vec<usize> = rules.iter().map(|rule| rule.rule).collect();
        assert_eq!(order, [1, 0]);
        assert_eq!(
            rules[0].declarations,
            [("color".to_owned(), "#222".to_owned(), true)]
        );
        // `text`'s colour lost to the ID; its width did not.
        assert_eq!(
            rules[1].declarations,
            [
                ("color".to_owned(), "#111".to_owned(), false),
                ("width".to_owned(), "1px".to_owned(), true),
            ]
        );
        assert_eq!(sheet.rules[rules[0].rule].origin.line, 2);
    }
}
