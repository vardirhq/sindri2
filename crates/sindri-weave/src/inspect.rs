//! What a devtools panel shows about one element: every rule that styles it,
//! strongest first, which of their declarations apply, and what it ends up
//! with.
//!
//! Asked of the same element tree and cascade that styling runs, so what the
//! panel says is what was drawn.

use sindri_core::{EntityId, World};
use weave::{Computed, Origin, Specificity, Stylesheet, Viewport};

use crate::UiStates;
use crate::tree::elements;

/// One rule that styles the inspected element.
#[derive(Clone, Debug, PartialEq)]
pub struct InspectedRule {
    /// Which of the project's stylesheets it is in, in the order they apply.
    pub sheet: usize,
    pub origin: Origin,
    pub specificity: Specificity,
    /// Each declaration, and whether it is the one that applies: `false`
    /// when a stronger rule, or a later stylesheet, sets the same property.
    pub declarations: Vec<(String, String, bool)>,
}

/// Everything the cascade decided about one element.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Inspection {
    /// Strongest first: the later stylesheet before the earlier, and within
    /// one, the order its cascade ranks them.
    pub rules: Vec<InspectedRule>,
    /// Every property that applies, inherited ones included, as authored.
    pub computed: Vec<(String, String)>,
    /// The custom properties in scope, `--name` and value.
    pub variables: Vec<(String, String)>,
}

/// Inspects `entity` as styled by `stylesheets` at `viewport` in `states`.
/// `None` for an entity that is not in the world.
#[must_use]
pub fn inspect(
    world: &World,
    stylesheets: &[Stylesheet],
    viewport: Viewport,
    states: &UiStates,
    entity: EntityId,
) -> Option<Inspection> {
    let tree = elements(world, states);
    let node = tree.nodes.iter().position(|node| node.entity == entity)?;
    let mut inspection = Inspection::default();
    let mut computed: Vec<(String, String)> = Vec::new();
    for (sheet, stylesheet) in stylesheets.iter().enumerate() {
        // Parents before children, as styling runs, so inheritance and
        // `var()` see what they would.
        let mut styles: Vec<Computed> = Vec::with_capacity(tree.nodes.len());
        for (position, current) in tree.nodes.iter().enumerate() {
            let parent = current.parent.and_then(|parent| styles.get(parent));
            styles.push(weave::cascade(
                stylesheet, &tree, position, viewport, parent,
            ));
            if position == node {
                break;
            }
        }
        if let Some(style) = styles.get(node) {
            for (property, value) in style.iter() {
                computed.retain(|(held, _)| held != property);
                computed.push((property.to_owned(), value.to_owned()));
            }
        }
        let matched = weave::matched(stylesheet, &tree, node, viewport);
        // A later stylesheet's win beats this one's.
        for earlier in &mut inspection.rules {
            for (property, _, won) in &mut earlier.declarations {
                let retaken = matched.iter().any(|rule| {
                    rule.declarations
                        .iter()
                        .any(|(later, _, applies)| *applies && later == property)
                });
                if retaken {
                    *won = false;
                }
            }
        }
        let listed = matched.into_iter().map(|rule| InspectedRule {
            sheet,
            origin: stylesheet.rules[rule.rule].origin.clone(),
            specificity: rule.specificity,
            declarations: rule.declarations,
        });
        inspection.rules.splice(0..0, listed);
    }
    computed.sort();
    let (variables, computed): (Vec<_>, Vec<_>) = computed
        .into_iter()
        .partition(|(property, _)| property.starts_with("--"));
    inspection.computed = computed;
    inspection.variables = variables;
    Some(inspection)
}
