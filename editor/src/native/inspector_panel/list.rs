//! Lists somebody can actually change.
//!
//! A list of objects used to be a readout. `ValueKind::Opaque` was the honest
//! answer while nothing described what a list held: a text field over a
//! collider's pieces is a way to break a scene rather than a way to edit one.
//!
//! What changed is that the component now says. The field template carries one
//! exemplar item, so the panel knows what a piece consists of, what each of its
//! fields means, and — the part that makes adding possible — what a fresh one
//! is. A new piece is the piece the registration already describes, not one the
//! editor invents.
//!
//! Only a list of *objects* is drawn this way. A tilemap's thousand tiles and a
//! footprint's pairs of numbers stay readouts, because the exemplar says they
//! are not this shape.

use eframe::egui::{self, RichText};
use serde_json::Value;

use crate::ui::theme::{color, metric, text};

use super::rows::{At, Authored, join, value_row};

/// Whether the component says this field holds a list of objects.
///
/// Asked of the template rather than the stored value: a list that is empty
/// right now still has items it would hold, and that is precisely when somebody
/// wants to add the first one.
pub(crate) fn is_list(at: At<'_>) -> bool {
    at.exemplar()
        .and_then(Value::as_array)
        .and_then(|items| items.first())
        .is_some_and(Value::is_object)
}

/// What a fresh item of this list is.
fn blank_item(at: At<'_>) -> Option<Value> {
    at.exemplar()
        .and_then(Value::as_array)
        .and_then(|items| items.first())
        .cloned()
}

/// What the author asked to happen to the list, decided while drawing it and
/// applied after.
///
/// Applied after, because removing an item while iterating the same list is how
/// a panel edits the wrong one.
enum Change {
    Add,
    Remove(usize),
    MoveUp(usize),
}

/// One list: a heading that counts and adds, then every item.
pub(crate) fn list_rows(
    ui: &mut egui::Ui,
    at: At<'_>,
    label: &str,
    value: &mut Value,
    indent: f32,
) {
    let mut change = None;
    let count = value.as_array().map_or(0, Vec::len);

    ui.horizontal(|ui| {
        ui.add_space(metric::GUTTER + indent);
        ui.label(
            RichText::new(label)
                .size(text::LABEL)
                .color(color::TEXT_FAINT),
        );
        ui.label(
            RichText::new(format!("{count}"))
                .size(text::NOTE)
                .color(color::TEXT_FAINT),
        );
        if blank_item(at).is_some() && ui.small_button("+").on_hover_text("Add one").clicked() {
            change = Some(Change::Add);
        }
    });

    let Some(items) = value.as_array_mut() else {
        return;
    };
    for (index, item) in items.iter_mut().enumerate() {
        ui.horizontal(|ui| {
            ui.add_space(metric::GUTTER + indent + 10.0);
            ui.label(
                RichText::new(format!("{}", index + 1))
                    .size(text::LABEL)
                    .color(color::TEXT_FAINT),
            );
            // A list whose order is only ever appended to cannot be reordered
            // at all, and for a compound collider order decides nothing but
            // reads as structure, so one button that moves an item up is
            // enough to arrange it.
            if index > 0 && ui.small_button("↑").on_hover_text("Move up").clicked() {
                change = Some(Change::MoveUp(index));
            }
            if ui.small_button("−").on_hover_text("Remove").clicked() {
                change = Some(Change::Remove(index));
            }
        });
        let Value::Object(fields) = item else {
            continue;
        };
        let item_path = join(at.path, &index.to_string());
        for (key, field) in fields.iter_mut() {
            let field_path = join(&item_path, key);
            value_row(
                ui,
                at.into(&field_path),
                key,
                field,
                indent + 20.0,
                Authored::Default,
            );
        }
    }

    apply(at, value, change);
}

/// Applies what the author asked for, once the list is no longer being walked.
fn apply(at: At<'_>, value: &mut Value, change: Option<Change>) {
    apply_change(blank_item(at), value, change);
}

/// The same, with the blank already resolved, so what a list *does* can be
/// tested without drawing one.
fn apply_change(blank: Option<Value>, value: &mut Value, change: Option<Change>) {
    let Some(change) = change else {
        return;
    };
    let Some(items) = value.as_array_mut() else {
        return;
    };
    match change {
        Change::Add => {
            if let Some(blank) = blank {
                items.push(blank);
            }
        }
        // Two guards, and the second is not decoration: `len() > 1` alone let a
        // stale index through to `Vec::remove`, which panics rather than
        // declining. A list that can be emptied is also one a component can be
        // left invalid by -- the engine refuses a collider with no pieces -- so
        // the last item stays and the button that would remove it does nothing.
        Change::Remove(index) if items.len() > 1 && index < items.len() => {
            items.remove(index);
        }
        Change::MoveUp(index) if index > 0 && index < items.len() => {
            items.swap(index - 1, index);
        }
        // Asking for something the list cannot do leaves it as it was.
        Change::Remove(_) | Change::MoveUp(_) => {}
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{Change, apply_change};

    fn piece(friction: f64) -> serde_json::Value {
        json!({ "friction": friction })
    }

    /// A new item is the one the component described, not one invented here.
    #[test]
    fn adding_uses_the_schemas_own_blank() {
        let mut list = json!([piece(0.5)]);
        apply_change(Some(piece(0.25)), &mut list, Some(Change::Add));
        assert_eq!(list, json!([piece(0.5), piece(0.25)]));
    }

    /// A list with no blank cannot grow, rather than growing something wrong.
    #[test]
    fn a_list_with_no_blank_does_not_grow() {
        let mut list = json!([piece(0.5)]);
        apply_change(None, &mut list, Some(Change::Add));
        assert_eq!(list, json!([piece(0.5)]));
    }

    #[test]
    fn removing_takes_the_item_asked_for() {
        let mut list = json!([piece(0.1), piece(0.2), piece(0.3)]);
        apply_change(None, &mut list, Some(Change::Remove(1)));
        assert_eq!(list, json!([piece(0.1), piece(0.3)]));
    }

    /// The engine refuses a collider with no pieces, so the panel refuses to
    /// make one. A button that produces a scene that will not load is worse
    /// than a button that does nothing.
    #[test]
    fn the_last_item_cannot_be_removed() {
        let mut list = json!([piece(0.5)]);
        apply_change(None, &mut list, Some(Change::Remove(0)));
        assert_eq!(list, json!([piece(0.5)]), "the list emptied itself");
    }

    #[test]
    fn moving_up_swaps_with_the_one_above() {
        let mut list = json!([piece(0.1), piece(0.2)]);
        apply_change(None, &mut list, Some(Change::MoveUp(1)));
        assert_eq!(list, json!([piece(0.2), piece(0.1)]));
    }

    /// An index that is not there leaves the list alone.
    #[test]
    fn a_change_that_cannot_apply_changes_nothing() {
        let mut list = json!([piece(0.1), piece(0.2)]);
        apply_change(None, &mut list, Some(Change::MoveUp(0)));
        apply_change(None, &mut list, Some(Change::MoveUp(9)));
        apply_change(None, &mut list, Some(Change::Remove(9)));
        assert_eq!(list, json!([piece(0.1), piece(0.2)]));
    }
}
