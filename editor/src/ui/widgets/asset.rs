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
    /// Whether the browser has this file selected.
    ///
    /// Separate from [`Self::current`], which is a fact about the editor rather
    /// than about the browser. The row drew only the latter, so the open scene
    /// wore the selection band permanently and clicking anything marked
    /// nothing.
    pub selected: bool,
    /// The scene the editor currently has open, marked wherever it appears.
    pub current: bool,
    /// `Some` for a folder or a sliced image, carrying whether it is open.
    pub expanded: Option<&'a mut bool>,
    /// The name being typed, when this row is being renamed in place.
    ///
    /// The same interaction the hierarchy uses, for the same reason: a browser
    /// of forty files should let one be renamed without the eyes leaving it.
    pub editing: Option<&'a mut String>,
}

/// What a browser row reported: its response, and whether a rename finished.
pub struct AssetRow {
    pub response: Response,
    /// `Some(true)` when the name was committed, `Some(false)` when abandoned.
    pub renamed: Option<bool>,
}

/// One file as a row.
///
/// The response is the row's, and every label inside it carries the row's sense
/// rather than its own: a widget inside a sensing scope takes precedence over
/// the scope, and an ordinary egui label is selectable text, so it would answer
/// a double click by selecting a word and the row would never hear about it.
pub fn row(ui: &mut egui::Ui, entry: Entry<'_>) -> AssetRow {
    let Entry {
        icon,
        name,
        kind,
        depth,
        selected,
        current,
        expanded,
        editing,
    } = entry;
    let mut committed = false;
    let mut cancelled = false;
    // Every row answers the pointer, whether or not the editor can open what it
    // names. A row that cannot be clicked cannot be selected, and one that
    // cannot be selected has nowhere to put a right-click menu — which is how
    // rename and delete came to have no home. What a row can *do* is said on
    // hover instead of by refusing to respond.
    let sense = Sense::click();
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
                    egui::Label::new(icon.outlined().rich_text().size(14.0).color(
                        if current || selected {
                            color::FORGE
                        } else {
                            color::TEXT_FAINT
                        },
                    ))
                    .sense(sense),
                );
                let named = name_of(ui, name, editing, current || selected, sense);
                committed = named.committed;
                cancelled = named.cancelled;
                let label = named.response;
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
    tree::paint_ground(ui, ground, rect, selected, hovered, depth);
    // The open scene is a standing fact rather than a selection, so it is
    // marked in the margin instead of taking the band a selected row wears.
    if current {
        ui.painter().rect_filled(
            egui::Rect::from_min_size(
                rect.right_top() - Vec2::new(metric::SELECT_RULE, 0.0),
                Vec2::new(metric::SELECT_RULE, rect.height()),
            ),
            0.0,
            color::FORGE_DIM,
        );
    }
    AssetRow {
        response: scope.response | inner,
        renamed: match (committed, cancelled) {
            (_, true) => Some(false),
            (true, _) => Some(true),
            _ => None,
        },
    }
}

/// What a row's name reported: the widget, and how a rename ended.
struct Named {
    response: Response,
    committed: bool,
    cancelled: bool,
}

/// A row's name, as a label or as the field replacing it while it is renamed.
fn name_of(
    ui: &mut egui::Ui,
    name: &str,
    editing: Option<&mut String>,
    lit: bool,
    sense: Sense,
) -> Named {
    let Some(draft) = editing else {
        return Named {
            response: ui.add(
                egui::Label::new(RichText::new(name).size(text::LABEL).color(if lit {
                    color::TEXT
                } else {
                    color::TEXT_MUTED
                }))
                .selectable(false)
                .truncate()
                .sense(sense),
            ),
            committed: false,
            cancelled: false,
        };
    };
    let field = ui.add(
        egui::TextEdit::singleline(draft)
            .desired_width(ui.available_width() - 6.0)
            .font(egui::FontId::proportional(text::LABEL)),
    );
    // Focused the frame it appears, so renaming is one act rather than "start
    // renaming, then click the box".
    if !field.has_focus() && !field.lost_focus() {
        field.request_focus();
    }
    let cancelled = ui.input(|input| input.key_pressed(egui::Key::Escape));
    Named {
        committed: field.lost_focus() && !cancelled,
        cancelled,
        response: field,
    }
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

/// A folder in the browser's tree pane.
///
/// Answers a click: the pane it lives in used to be labels with no sense, so it
/// listed the project's folders and selected none of them.
pub fn folder(ui: &mut egui::Ui, label: &str, selected: bool, depth: usize) -> Response {
    let width = ui.available_width();
    let scope = ui.scope_builder(egui::UiBuilder::new().sense(Sense::click()), |ui| {
        ui.set_min_width(width);
        ui.set_min_height(metric::ROW_HEIGHT);
        let ground = tree::reserve(ui);
        let inner = ui
            .horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                ui.add_space(6.0 + tree::indent(depth, metric::INDENT));
                let icon = ui.add(
                    egui::Label::new(icons::FOLDER.outlined().rich_text().size(14.0).color(
                        if selected {
                            color::FORGE
                        } else {
                            color::TEXT_FAINT
                        },
                    ))
                    .sense(Sense::click()),
                );
                let label = ui.add(
                    egui::Label::new(RichText::new(label).size(text::LABEL).color(if selected {
                        color::TEXT
                    } else {
                        color::TEXT_MUTED
                    }))
                    .selectable(false)
                    .truncate()
                    .sense(Sense::click()),
                );
                icon | label
            })
            .inner;
        (inner, ground)
    });
    let (inner, ground) = scope.inner;
    let rect = scope.response.rect;
    let hovered = scope.response.hovered() || inner.hovered();
    tree::paint_ground(ui, ground, rect, selected, hovered, depth);
    scope.response | inner
}
