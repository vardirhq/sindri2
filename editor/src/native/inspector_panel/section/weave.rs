//! Weave presentation metadata as an editor control rather than opaque JSON.
//!
//! `weave.style` deliberately is not a Sindri engine component. It is metadata
//! consumed by the optional presentation bridge, so the editor owns this small
//! authoring surface instead of registering a fake runtime component just to
//! make a generic inspector draw it.

use std::collections::BTreeMap;

use eframe::egui;
use serde_json::{Value, json};

use crate::ui::icons;
use crate::ui::theme::{color, metric, text};
use crate::ui::widgets::{
    button::{self, Intent},
    panel, property, section,
};

pub(super) const TYPE_NAME: &str = "weave.style";

/// Draws the presentation metadata every entity may opt into.
///
/// The stable ID edited above this section is already Weave's `#id` selector.
/// This surface owns only reusable `.class` roles. Keeping it outside the
/// component registry preserves the runtime boundary: a project that never
/// uses Weave still has no Weave component type in the engine.
pub(super) fn weave_style_section(ui: &mut egui::Ui, components: &mut BTreeMap<String, Value>) {
    let present = components.contains_key(TYPE_NAME);
    let mut remove_style = false;
    let open = section::component(
        ui,
        egui::Id::new("inspector-weave-style"),
        icons::STYLESHEET,
        "Weave Style",
        |ui| {
            if present
                && button::row_icon(
                    ui,
                    icons::REMOVE,
                    Intent::Danger,
                    "Remove Weave style metadata",
                )
                .clicked()
            {
                remove_style = true;
            }
        },
    );

    if remove_style {
        components.remove(TYPE_NAME);
        return;
    }
    if !open {
        return;
    }

    ui.add_space(4.0);
    panel::note(
        ui,
        "The Stable ID above is this entity's #id selector. Classes below are reusable .class selectors.",
    );
    ui.add_space(5.0);

    if !present {
        ui.horizontal(|ui| {
            ui.add_space(metric::GUTTER);
            if ui
                .button(
                    egui::RichText::new("Add Weave classes")
                        .size(text::BODY)
                        .color(color::TEXT),
                )
                .clicked()
            {
                components.insert(TYPE_NAME.to_owned(), json!({ "classes": [] }));
            }
        });
        return;
    }

    let Some(payload) = components.get_mut(TYPE_NAME) else {
        return;
    };
    let Some(classes) = classes_mut(payload) else {
        property::readout(
            ui,
            "Classes",
            "invalid metadata",
            Some(
                "weave.style.classes must be an array; remove this section and add it again to repair it",
            ),
        );
        return;
    };

    let mut remove_class = None;
    for (index, class) in classes.iter_mut().enumerate() {
        let mut value = class.as_str().unwrap_or_default().to_owned();
        property::Property::new(&format!("Class {}", index + 1)).show(ui, |ui| {
            let remove_width = 24.0;
            let field_width = (property::value_width(ui) - remove_width - 4.0).max(56.0);
            if ui
                .add_sized(
                    [field_width, metric::CONTROL_HEIGHT],
                    egui::TextEdit::singleline(&mut value).hint_text("hud-card"),
                )
                .changed()
            {
                *class = Value::String(clean_class(&value));
            }
            if ui
                .add_sized(
                    [remove_width, metric::CONTROL_HEIGHT],
                    egui::Button::new(icons::REMOVE.outlined().rich_text().size(16.0)),
                )
                .on_hover_text("Remove this class")
                .clicked()
            {
                remove_class = Some(index);
            }
        });
    }
    if let Some(index) = remove_class {
        classes.remove(index);
    }

    ui.horizontal(|ui| {
        ui.add_space(metric::GUTTER);
        if ui
            .button(
                egui::RichText::new("Add class")
                    .size(text::LABEL)
                    .color(color::TEXT_MUTED),
            )
            .clicked()
        {
            classes.push(Value::String("new-class".to_owned()));
        }
    });
}

fn classes_mut(payload: &mut Value) -> Option<&mut Vec<Value>> {
    let object = payload.as_object_mut()?;
    let classes = object.entry("classes").or_insert_with(|| json!([]));
    classes.as_array_mut()
}

fn clean_class(value: &str) -> String {
    value
        .trim()
        .trim_start_matches('.')
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::{classes_mut, clean_class};
    use serde_json::{Value, json};

    #[test]
    fn class_editor_preserves_other_weave_metadata() {
        let mut payload = json!({ "classes": ["hud-card"], "future": { "kept": true } });
        classes_mut(&mut payload)
            .expect("classes")
            .push(Value::String("danger".to_owned()));
        assert_eq!(payload["classes"], json!(["hud-card", "danger"]));
        assert_eq!(payload["future"], json!({ "kept": true }));
    }

    #[test]
    fn missing_classes_become_an_editable_empty_list() {
        let mut payload = json!({});
        assert!(classes_mut(&mut payload).expect("classes").is_empty());
        assert_eq!(payload, json!({ "classes": [] }));
    }

    #[test]
    fn class_names_accept_css_dot_spelling_but_store_the_name() {
        assert_eq!(clean_class("  .overlay-primary  "), "overlay-primary");
        assert_eq!(clean_class("hud-card extra"), "hud-card");
    }
}
