//! The devtools panel: what Weave did to the selected element.
//!
//! A browser's Styles pane, for the scene. The box model as drawn, every rule
//! that matched with the declarations a stronger rule overrode struck
//! through, each with the file and line it was written on, and what the
//! element ends up with. Clicking a declaration's value edits it, and the
//! change is written into the stylesheet at that line, which then reloads:
//! the file stays the one source of truth, as it would after a save.

use eframe::egui::{self, Align2, Color32, FontId, Id, Sense, Stroke, StrokeKind, Vec2};
use sindri_core::EntityId;
use sindri_scene::UiHierarchy;
use sindri_weave::{InspectedRule, Inspection};

use crate::native::EditorApp;
use crate::ui::icons;
use crate::ui::theme::{color, metric, text};
use crate::ui::widgets::{panel, section};

/// Everything the panel shows, worked out before the inspector draws.
pub(in crate::native) struct StylesView {
    inspection: Inspection,
    /// The element's size as drawn, and its padding and margin, in overlay
    /// units: top, right, bottom, left for the sides.
    size: [f32; 2],
    padding: [f32; 4],
    margin: [f32; 4],
    /// How many of the stylesheet's pixels one overlay unit is at the
    /// viewport inspected, so the numbers read as the stylesheet wrote them.
    pixels_per_unit: f32,
}

/// A value changed in the panel, to be written where its rule was written.
pub(in crate::native) struct StyleEdit {
    pub file: String,
    pub line: usize,
    pub selector: String,
    pub property: String,
    pub value: String,
}

const SECTION: &str = "inspector-weave-cascade";
/// The declaration being edited, if any, and the text typed so far.
const EDITING: &str = "inspector-weave-editing";

#[derive(Clone)]
struct Editing {
    declaration: Id,
    draft: String,
    focused: bool,
}

impl EditorApp {
    /// What the panel shows about `entity`, or `None` when there is nothing
    /// to show: no stylesheets, not a UI element, or the section is closed —
    /// which spares resolving the scene every frame for a panel nobody reads.
    pub(in crate::native) fn styles_view(
        &mut self,
        ctx: &egui::Context,
        entity: EntityId,
    ) -> Option<StylesView> {
        if self.styles.is_empty() || !is_ui(&self.world, entity) {
            return None;
        }
        let open = ctx.data(|data| data.get_temp::<bool>(Id::new(SECTION)).unwrap_or(true));
        let states = sindri_weave::pointer_states(
            &self.world,
            self.screen_ui.hovered(),
            self.screen_ui.active(),
        );
        let (inspection, viewport) = self.styles.inspect(&self.world, entity, &states)?;
        let mut view = StylesView {
            inspection,
            size: [0.0; 2],
            padding: [0.0; 4],
            margin: [0.0; 4],
            pixels_per_unit: viewport.height / 2.0,
        };
        if !open {
            return Some(view);
        }
        let drawn = self.styles.drawn(&self.world)?;
        let data = drawn.get(entity)?;
        let own = data.transform_3d.unwrap_or_default().scale_2d();
        view.size = UiHierarchy::of(&drawn, self.scene.components())
            .ok()
            .and_then(|hierarchy| hierarchy.placement(entity))
            .map_or(own, |placed| placed.size_or(own));
        let sides = |field: &str| -> [f32; 4] {
            let mut sides = [0.0; 4];
            let stored = data
                .components
                .get("sindri.ui.box")
                .and_then(|payload| payload.get(field))
                .and_then(serde_json::Value::as_array);
            for (side, value) in sides.iter_mut().zip(stored.into_iter().flatten()) {
                #[allow(clippy::cast_possible_truncation)]
                let read = value.as_f64().unwrap_or(0.0) as f32;
                *side = read;
            }
            sides
        };
        view.padding = sides("padding");
        view.margin = sides("margin");
        Some(view)
    }
}

impl EditorApp {
    /// Writes a value edited in the panel into its stylesheet. The styles
    /// reload from the file, so what the scene shows is what was saved.
    pub(in crate::native) fn write_style(&mut self, edit: Option<StyleEdit>) {
        let Some(edit) = edit else {
            return;
        };
        match self.styles.set_declaration(
            &edit.file,
            edit.line,
            &edit.selector,
            &edit.property,
            &edit.value,
        ) {
            Ok(()) => self.console.info(format!(
                "{}:{}: {} set to {}",
                edit.file, edit.line, edit.property, edit.value
            )),
            Err(error) => self.console.error(error),
        }
    }
}

fn is_ui(world: &sindri_core::World, entity: EntityId) -> bool {
    world.get(entity).is_some_and(|data| {
        data.components
            .keys()
            .any(|name| name.starts_with("sindri.ui.") || name == "weave.style")
    })
}

/// Draws the panel for a UI element in a styled project, and returns the
/// value edited in it, if one was committed this frame.
pub(in crate::native) fn styles_section(ui: &mut egui::Ui, view: &StylesView) -> Option<StyleEdit> {
    let open = section::component(ui, Id::new(SECTION), icons::STYLESHEET, "Styles", |_| {});
    if !open {
        return None;
    }
    ui.add_space(6.0);
    box_model(ui, view);
    ui.add_space(8.0);
    if view.inspection.rules.is_empty() {
        panel::note(
            ui,
            "No rule in the project's stylesheets matches this element.",
        );
    }
    let mut edit = None;
    for rule in &view.inspection.rules {
        edit = edit.or(rule_block(ui, rule));
    }
    if !view.inspection.computed.is_empty() {
        heading(ui, "Computed");
        for (property, value) in &view.inspection.computed {
            declaration(ui, property, value, true);
        }
    }
    if !view.inspection.variables.is_empty() {
        heading(ui, "Variables");
        for (property, value) in &view.inspection.variables {
            declaration(ui, property, value, true);
        }
    }
    ui.add_space(6.0);
    edit
}

fn heading(ui: &mut egui::Ui, title: &str) {
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.add_space(metric::GUTTER);
        ui.label(
            egui::RichText::new(title)
                .size(text::HEADING)
                .strong()
                .color(color::TEXT_MUTED),
        );
    });
}

/// One matched rule: its selector and where it was written, then its
/// declarations, the overridden ones struck through as a browser shows them.
fn rule_block(ui: &mut egui::Ui, rule: &InspectedRule) -> Option<StyleEdit> {
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.add_space(metric::GUTTER);
        ui.label(
            egui::RichText::new(format!("{} {{", rule.origin.selector))
                .monospace()
                .size(text::BODY)
                .color(color::FORGE_BRIGHT),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add_space(metric::GUTTER);
            let place = if rule.origin.file.is_empty() {
                format!("line {}", rule.origin.line)
            } else {
                format!("{}:{}", rule.origin.file, rule.origin.line)
            };
            ui.label(
                egui::RichText::new(place)
                    .size(text::NOTE)
                    .color(color::TEXT_FAINT),
            );
        });
    });
    let mut edit = None;
    for (property, value, applies) in &rule.declarations {
        // Only a rule read from a project file has somewhere to write to.
        if rule.origin.file.is_empty() {
            declaration(ui, property, value, *applies);
            continue;
        }
        let id = Id::new((EDITING, &rule.origin.file, rule.origin.line, property));
        if let Some(value) = editable(ui, id, property, value, *applies) {
            edit = Some(StyleEdit {
                file: rule.origin.file.clone(),
                line: rule.origin.line,
                selector: rule.origin.selector.clone(),
                property: property.clone(),
                value,
            });
        }
    }
    ui.horizontal(|ui| {
        ui.add_space(metric::GUTTER);
        ui.label(
            egui::RichText::new("}")
                .monospace()
                .size(text::BODY)
                .color(color::FORGE_BRIGHT),
        );
    });
    edit
}

fn declaration_text(text: String, applies: bool) -> egui::RichText {
    let mut line = egui::RichText::new(text)
        .monospace()
        .size(text::LABEL)
        .color(if applies {
            color::TEXT
        } else {
            color::TEXT_FAINT
        });
    if !applies {
        line = line.strikethrough();
    }
    line
}

/// A declaration whose value can be clicked and typed over, as in a
/// browser's Styles pane. Enter or clicking away commits it; Escape leaves
/// the stylesheet as it was. Returns the new value when one is committed.
fn editable(
    ui: &mut egui::Ui,
    id: Id,
    property: &str,
    value: &str,
    applies: bool,
) -> Option<String> {
    let key = Id::new(EDITING);
    let editing = ui
        .data(|data| data.get_temp::<Editing>(key))
        .filter(|editing| editing.declaration == id);
    let mut committed = None;
    ui.horizontal(|ui| {
        ui.add_space(metric::GUTTER + metric::INDENT);
        ui.spacing_mut().item_spacing.x = 0.0;
        ui.label(declaration_text(format!("{property}: "), applies));
        let Some(mut editing) = editing else {
            let shown = ui
                .add(
                    egui::Label::new(declaration_text(value.to_owned(), applies))
                        .sense(Sense::click()),
                )
                .on_hover_cursor(egui::CursorIcon::Text)
                .on_hover_text(if applies {
                    "Applies to this element. Click to change it in the stylesheet"
                } else {
                    "Overridden by a stronger rule or a later stylesheet. Click to change it"
                });
            if shown.clicked() {
                ui.data_mut(|data| {
                    data.insert_temp(
                        key,
                        Editing {
                            declaration: id,
                            draft: value.to_owned(),
                            focused: false,
                        },
                    );
                });
            }
            ui.label(declaration_text(";".to_owned(), applies));
            return;
        };
        let field = ui.add(
            egui::TextEdit::singleline(&mut editing.draft)
                .id(id)
                .font(egui::TextStyle::Monospace)
                .desired_width(ui.available_width() - metric::GUTTER),
        );
        if !editing.focused {
            field.request_focus();
            editing.focused = true;
        }
        if field.lost_focus() {
            let cancelled = ui.input(|input| input.key_pressed(egui::Key::Escape));
            let draft = editing.draft.trim();
            if !cancelled && draft != value {
                committed = Some(draft.to_owned());
            }
            ui.data_mut(|data| data.remove::<Editing>(key));
        } else {
            ui.data_mut(|data| data.insert_temp(key, editing));
        }
    });
    committed
}

fn declaration(ui: &mut egui::Ui, property: &str, value: &str, applies: bool) {
    ui.horizontal(|ui| {
        ui.add_space(metric::GUTTER + metric::INDENT);
        ui.label(declaration_text(format!("{property}: {value};"), applies))
            .on_hover_text(if applies {
                "Applies to this element"
            } else {
                "Overridden by a stronger rule or a later stylesheet"
            });
    });
}

/// The box model as browsers draw it: margin round the border, padding
/// inside it, and the content in the middle, with each side's size in the
/// stylesheet's pixels.
fn box_model(ui: &mut egui::Ui, view: &StylesView) {
    let width = (ui.available_width() - 2.0 * metric::GUTTER).max(160.0);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 132.0), Sense::hover());
    let outer = egui::Rect::from_center_size(rect.center(), Vec2::new(width, 128.0));
    let painter = ui.painter_at(rect);
    let px = |units: f32| format!("{}", (units * view.pixels_per_unit).round());
    let layers = [
        ("margin", Color32::from_rgb(92, 70, 38), view.margin, 22.0),
        ("padding", Color32::from_rgb(52, 78, 58), view.padding, 22.0),
    ];
    let mut area = outer;
    let font = FontId::monospace(text::NOTE);
    for (name, fill, sides, inset) in layers {
        painter.rect(
            area,
            0.0,
            fill,
            Stroke::new(1.0, color::LINE),
            StrokeKind::Inside,
        );
        painter.text(
            area.left_top() + Vec2::new(4.0, 2.0),
            Align2::LEFT_TOP,
            name,
            font.clone(),
            color::TEXT_FAINT,
        );
        let inner = area.shrink(inset);
        let middle = f32::midpoint;
        let [top, right, bottom, left] = sides;
        for (text, at) in [
            (
                px(top),
                egui::pos2(area.center().x, middle(area.top(), inner.top())),
            ),
            (
                px(bottom),
                egui::pos2(area.center().x, middle(inner.bottom(), area.bottom())),
            ),
            (
                px(left),
                egui::pos2(middle(area.left(), inner.left()), area.center().y),
            ),
            (
                px(right),
                egui::pos2(middle(inner.right(), area.right()), area.center().y),
            ),
        ] {
            painter.text(at, Align2::CENTER_CENTER, text, font.clone(), color::TEXT);
        }
        area = inner;
    }
    painter.rect(
        area,
        0.0,
        Color32::from_rgb(40, 64, 92),
        Stroke::new(1.0, color::LINE),
        StrokeKind::Inside,
    );
    painter.text(
        area.center(),
        Align2::CENTER_CENTER,
        format!("{} × {}", px(view.size[0]), px(view.size[1])),
        font,
        color::TEXT,
    );
}
