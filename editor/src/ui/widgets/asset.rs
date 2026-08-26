//! What a file looks like in the project browser, as a row and as a tile.
//!
//! The browser's job is not to reproduce a directory listing — the operating
//! system already has one — it is to say what each file *is to the editor*: the
//! scene that is open, an image that has been sliced, a script the scene runs.
//! So a row carries a kind badge, marks the open scene, and says on hover
//! whether there is anything to do with it; and a tile is a preview plate with
//! a caption rather than a large button with a small icon in the middle.

use eframe::egui::{self, Align, Layout, Response, RichText, Sense, Stroke, Vec2};
use egui_material_icons::MaterialIcon;

use crate::ui::icons;
use crate::ui::theme::{color, metric, radius, text};

use super::tree;

/// Everything a browser row draws about one file.
pub struct Entry<'a> {
    pub icon: MaterialIcon,
    pub name: &'a str,
    /// What kind of file this is, shown at the end of the row.
    pub kind: &'a str,
    pub depth: usize,
    /// The scene the editor currently has open.
    pub current: bool,
    /// Whether there is anything the editor can do with this file.
    pub actionable: bool,
    /// `Some` for a sliced image, carrying whether its parts are showing.
    pub expanded: Option<&'a mut bool>,
}

/// One file as a row.
///
/// The response is the row's, and every label inside it carries the row's sense
/// rather than its own: a widget inside a sensing scope takes precedence over
/// the scope, and an ordinary egui label is selectable text, so it would answer
/// a double click by selecting a word and the row would never hear about it.
pub fn row(ui: &mut egui::Ui, entry: Entry<'_>) -> Response {
    let Entry {
        icon,
        name,
        kind,
        depth,
        current,
        actionable,
        expanded,
    } = entry;
    let sense = if actionable {
        Sense::click()
    } else {
        Sense::hover()
    };
    let width = ui.available_width();
    let scope = ui.scope_builder(egui::UiBuilder::new().sense(sense), |ui| {
        ui.set_min_width(width);
        ui.set_min_height(metric::ROW_HEIGHT);
        let ground = tree::reserve(ui);
        let inner = ui
            .horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 3.0;
                ui.add_space(6.0 + tree::indent(depth, metric::INDENT));
                match expanded {
                    Some(expanded) => {
                        let triangle = ui.add(
                            egui::Label::new(
                                if *expanded {
                                    icons::EXPANDED
                                } else {
                                    icons::COLLAPSED
                                }
                                .outlined()
                                .rich_text()
                                .size(14.0)
                                .color(color::TEXT_FAINT),
                            )
                            .sense(Sense::click()),
                        );
                        if triangle.clicked() {
                            *expanded = !*expanded;
                        }
                    }
                    // The width a triangle would take, so names line up whether
                    // or not their image is sliced.
                    None => ui.add_space(14.0),
                }
                let icon = ui.add(
                    egui::Label::new(icon.outlined().rich_text().size(14.0).color(if current {
                        color::FORGE
                    } else {
                        color::TEXT_FAINT
                    }))
                    .sense(sense),
                );
                let label = ui.add(
                    egui::Label::new(RichText::new(name).size(text::LABEL).color(if current {
                        color::TEXT
                    } else {
                        color::TEXT_MUTED
                    }))
                    .selectable(false)
                    .truncate()
                    .sense(sense),
                );
                let badge = ui
                    .with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.add_space(metric::GUTTER);
                        ui.add(
                            egui::Label::new(
                                RichText::new(kind)
                                    .size(text::NOTE)
                                    .color(color::TEXT_FAINT.gamma_multiply(0.85)),
                            )
                            .selectable(false)
                            .sense(sense),
                        )
                    })
                    .inner;
                icon | label | badge
            })
            .inner;
        (inner, ground)
    });
    let (inner, ground) = scope.inner;
    let rect = scope.response.rect;
    let hovered = scope.response.hovered() || inner.hovered();
    tree::paint_ground(ui, ground, rect, current, hovered && actionable, depth);
    scope.response | inner
}

/// One file as a tile: a plate to look at, and a name under it.
pub fn tile(
    ui: &mut egui::Ui,
    icon: MaterialIcon,
    name: &str,
    current: bool,
    preview: Option<egui::TextureId>,
) -> Response {
    let size = Vec2::new(70.0, 78.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let painter = ui.painter_at(rect);
    let plate = egui::Rect::from_min_size(rect.min, Vec2::new(size.x, 52.0));
    let hovered = response.hovered();
    painter.rect_filled(
        plate,
        radius(),
        if current { color::EMBER } else { color::RAISED },
    );
    painter.rect_stroke(
        plate,
        radius(),
        Stroke::new(
            1.0,
            match (current, hovered) {
                (true, _) => color::FORGE,
                (false, true) => color::LINE,
                (false, false) => color::LINE_SOFT,
            },
        ),
        egui::StrokeKind::Inside,
    );
    // A real picture wherever there is one to show: a grid of identical glyphs
    // is the reason the list view is the default.
    if let Some(texture) = preview {
        painter.image(
            texture,
            plate.shrink(5.0),
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            egui::Color32::WHITE,
        );
    } else {
        painter.text(
            plate.center(),
            egui::Align2::CENTER_CENTER,
            icon.outlined().codepoint,
            egui::FontId::new(24.0, icon.outlined().font_family()),
            if current {
                color::FORGE
            } else {
                color::TEXT_FAINT
            },
        );
    }
    painter.text(
        egui::Pos2::new(rect.center().x, plate.bottom() + 4.0),
        egui::Align2::CENTER_TOP,
        elide(name, 12),
        egui::FontId::proportional(text::NOTE),
        if current {
            color::TEXT
        } else {
            color::TEXT_MUTED
        },
    );
    response
}

/// A file name cut to fit a tile's caption.
///
/// Painted text does not wrap or truncate itself, and a long name drawn under a
/// 70-pixel tile ran across its neighbours.
fn elide(name: &str, limit: usize) -> String {
    if name.chars().count() <= limit {
        return name.to_owned();
    }
    let kept: String = name.chars().take(limit.saturating_sub(1)).collect();
    format!("{kept}…")
}

/// A folder in the browser's tree, which is a listing rather than a control.
pub fn folder(ui: &mut egui::Ui, label: &str, selected: bool, depth: usize) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        ui.add_space(6.0 + tree::indent(depth, metric::INDENT));
        ui.label(
            icons::FOLDER
                .outlined()
                .rich_text()
                .size(14.0)
                .color(if selected {
                    color::FORGE
                } else {
                    color::TEXT_FAINT
                }),
        );
        ui.add(
            egui::Label::new(RichText::new(label).size(text::LABEL).color(if selected {
                color::TEXT
            } else {
                color::TEXT_MUTED
            }))
            .selectable(false)
            .truncate(),
        );
    });
}
