//! Cascade resolution between parsed Weave rules and Sindri presentation data.
//!
//! Parsing answers what the stylesheet says. This module answers the separate
//! question of which declaration wins for one entity. Keeping that answer as a
//! computed style gives later layout work one stable input instead of teaching
//! every property about selector specificity and source order.

use std::collections::{BTreeMap, btree_map};

use weave::{Stylesheet, Viewport};

#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct ComputedStyle {
    declarations: BTreeMap<String, String>,
}

impl ComputedStyle {
    #[must_use]
    pub(super) fn resolve(
        stylesheet: &Stylesheet,
        id: &str,
        classes: &[&str],
        component_types: &[&str],
        viewport: Viewport,
    ) -> Self {
        let mut winners: BTreeMap<String, (u8, usize, String)> = BTreeMap::new();
        for (source_order, rule) in stylesheet
            .rules
            .iter()
            .enumerate()
            .filter(|(_, rule)| {
                rule.applies_with_classes(id, classes, component_types, viewport)
            })
        {
            let specificity = rule.selector.specificity();
            for (property, value) in &rule.declarations {
                let replace = winners.get(property).is_none_or(
                    |(current_specificity, current_order, _)| {
                        (specificity, source_order) >= (*current_specificity, *current_order)
                    },
                );
                if replace {
                    winners.insert(
                        property.clone(),
                        (specificity, source_order, value.clone()),
                    );
                }
            }
        }

        Self {
            declarations: winners
                .into_iter()
                .map(|(property, (_, _, value))| (property, value))
                .collect(),
        }
    }

    #[must_use]
    pub(super) fn get(&self, property: &str) -> Option<&str> {
        self.declarations.get(property).map(String::as_str)
    }

    pub(super) fn into_declarations(self) -> btree_map::IntoIter<String, String> {
        self.declarations.into_iter()
    }
}

#[cfg(test)]
mod tests {
    use weave::{Viewport, parse};

    use super::ComputedStyle;

    const DESKTOP: Viewport = Viewport {
        width: 1_440.0,
        height: 900.0,
    };

    #[test]
    fn specificity_and_source_order_are_settled_once() {
        let stylesheet = parse(
            r#"
                sindri.ui.button { width: 100px; height: 40px; }
                .action { width: 200px; }
                .action { height: 48px; }
                #save { width: 300px; }
            "#,
        )
        .expect("stylesheet parses");

        let style = ComputedStyle::resolve(
            &stylesheet,
            "save",
            &["action"],
            &["sindri.ui.button"],
            DESKTOP,
        );

        assert_eq!(style.get("width"), Some("300px"));
        assert_eq!(style.get("height"), Some("48px"));
    }

    #[test]
    fn inactive_media_rules_never_enter_the_computed_style() {
        let stylesheet = parse(
            r#"
                #panel { width: 600px; }
                @media (orientation: portrait) {
                    #panel { width: 90vw; }
                }
            "#,
        )
        .expect("stylesheet parses");

        let style = ComputedStyle::resolve(&stylesheet, "panel", &[], &[], DESKTOP);
        assert_eq!(style.get("width"), Some("600px"));
    }
}
