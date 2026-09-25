//! The devtools panel: what Weave did to the selected element.
//!
//! A browser's Styles pane, for the scene. The box model as drawn, every rule
//! that matched with the declarations a stronger rule overrode struck
//! through, each with the file and line it was written on, and what the
//! element ends up with. Read-only: the stylesheet is the place to change it,
//! and it reloads as soon as it is saved.

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

const SECTION: &str = "inspector-weave-cascade";

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

fn is_ui(world: &sindri_core::World, entity: EntityId) -> bool {
    world.get(entity).is_some_and(|data| {
        data.components
            .keys()
            .any(|name| name.starts_with("sindri.ui.") || name == "weave.style")
    })
}

/// Draws the panel for a UI element in a styled project.
pub(in crate::native) fn styles_section(ui: &mut egui::Ui, view: &StylesView) {
    let open = section::component(ui, Id::new(SECTION), icons::STYLESHEET, "Styles", |_| {});
    if !open {
        return;
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
    for rule in &view.inspection.rules {
        rule_block(ui, rule);
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
fn rule_block(ui: &mut egui::Ui, rule: &InspectedRule) {
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
    for (property, value, applies) in &rule.declarations {
        declaration(ui, property, value, *applies);
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
}

fn declaration(ui: &mut egui::Ui, property: &str, value: &str, applies: bool) {
    ui.horizontal(|ui| {
        ui.add_space(metric::GUTTER + metric::INDENT);
        let mut line = egui::RichText::new(format!("{property}: {value};"))
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
        ui.label(line).on_hover_text(if applies {
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
