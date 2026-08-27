//! Rows that hang off other rows: the scene tree, and anything shaped like it.
//!
//! The hierarchy used to be a horizontal layout with an `add_space` for the
//! indent and a `selectable` button for the name, which meant selection was a
//! tinted rectangle around the *text* rather than the row, and depth was
//! readable only by measuring. A scene tree is the panel an author reads
//! fastest and most often, so the row is painted: a band across the whole
//! width, a rule marking the selected one, and a guide per level of nesting so
//! a child three deep can be traced back to its parent without counting pixels.

use eframe::egui::layers::ShapeIdx;
use eframe::egui::{self, Rect, Response, RichText, Sense, Shape, UiBuilder, Vec2, epaint};
use egui_material_icons::MaterialIcon;

use crate::ui::icons;
use crate::ui::theme::{color, metric, radius_tight, text};

/// The three independent things a tree row can report.
///
/// Selecting and folding are separate answers because they are separate acts:
/// a row that selected itself when its chevron was pressed would make folding
/// a subtree impossible without changing what is being edited.
pub struct TreeRow {
    pub select: Response,
    /// The region a drag can be released onto, which includes the chevron.
    pub drop: Response,
    pub toggle: Option<Response>,
    /// What happened to a name being typed into this row, if one was.
    pub rename: Option<Rename>,
}

/// How a rename on a row ended.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Rename {
    /// The draft is the new name.
    Committed,
    /// Escape: the name is what it was.
    Cancelled,
}

/// What hangs under a row, which decides whether it gets a fold control.
///
/// An enum rather than a `has_children` flag beside an `expanded` flag: those
/// two booleans have a combination that means nothing — collapsed with no
/// children — and a caller has to remember which order they go in.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Children {
    /// Nothing hangs under this row.
    #[default]
    None,
    /// Children, currently showing.
    Expanded,
    /// Children, currently folded away.
    Collapsed,
}

impl Children {
    /// How a row with `count` children folded to `collapsed` reads.
    pub fn of(count: usize, collapsed: bool) -> Self {
        match (count, collapsed) {
            (0, _) => Self::None,
            (_, true) => Self::Collapsed,
            (_, false) => Self::Expanded,
        }
    }

    const fn folds(self) -> bool {
        !matches!(self, Self::None)
    }

    const fn open(self) -> bool {
        matches!(self, Self::Expanded)
    }
}

/// How one row is drawn.
#[derive(Clone, Copy, Debug, Default)]
pub struct RowStyle {
    pub selected: bool,
    pub depth: usize,
    pub children: Children,
    /// A row that is listed but is not itself a thing to act on.
    pub dimmed: bool,
    /// A row for something that is still there and is switched off.
    ///
    /// Struck through rather than dimmed, because dim already means "nothing
    /// here to act on" and a switched-off entity is very much something to act
    /// on — it is the thing you would switch back on.
    pub struck: bool,
}

/// How far in a row at this depth starts.
pub fn indent(depth: usize, step: f32) -> f32 {
    f32::from(u16::try_from(depth).unwrap_or(u16::MAX)) * step
}

/// One row of a tree: a band, a chevron, an icon, and a name.
pub fn row(ui: &mut egui::Ui, icon: MaterialIcon, name: &str, style: RowStyle) -> TreeRow {
    row_named(ui, icon, name, style, None)
}

/// The same row, with its name being typed into.
///
/// Renaming happens on the row rather than in a dialog or only in the
/// inspector, because the row is where the name is: an author looking at a list
/// of forty entities should be able to fix one without their eyes leaving it.
/// `editing` carries the draft; the row reports whether it was committed.
pub fn row_named(
    ui: &mut egui::Ui,
    icon: MaterialIcon,
    name: &str,
    style: RowStyle,
    editing: Option<&mut String>,
) -> TreeRow {
    let width = ui.available_width();
    let mut committed = false;
    let mut cancelled = false;
    let builder = UiBuilder::new().sense(Sense::click_and_drag());
    let scope = ui.scope_builder(builder, |ui| {
        ui.set_min_width(width);
        ui.set_min_height(metric::ROW_HEIGHT);
        // Reserved before the contents so the band can be painted underneath
        // them once the row knows whether it is hovered.
        let ground = reserve(ui);
        let inner = ui
            .horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 3.0;
                ui.add_space(6.0 + indent(style.depth, metric::INDENT));
                let toggle = chevron(ui, style);
                // The icon carries the row's sense rather than hovering only:
                // a widget inside a sensing scope takes precedence over the
                // scope, so a plain label would be a dead patch mid-row.
                let icon = ui.add(
                    egui::Label::new(icon.outlined().rich_text().size(15.0).color(
                        if style.selected {
                            color::FORGE_BRIGHT
                        } else {
                            color::TEXT_FAINT
                        },
                    ))
                    .sense(Sense::click_and_drag()),
                );
                let label = if let Some(draft) = editing {
                    let field = ui.add(
                        egui::TextEdit::singleline(draft)
                            .desired_width(ui.available_width() - 6.0)
                            .font(egui::FontId::proportional(text::BODY)),
                    );
                    // Focused the frame it appears, so renaming is one act
                    // rather than "start renaming, then click the box".
                    if !field.has_focus() && !field.lost_focus() {
                        field.request_focus();
                    }
                    committed = field.lost_focus()
                        && !ui.input(|input| input.key_pressed(egui::Key::Escape));
                    cancelled = ui.input(|input| input.key_pressed(egui::Key::Escape));
                    field
                } else {
                    let mut text = RichText::new(name).size(text::BODY).color(
                        match (style.selected, style.dimmed) {
                            (true, _) => color::TEXT,
                            (false, true) => color::TEXT_FAINT,
                            (false, false) => color::TEXT_MUTED,
                        },
                    );
                    if style.struck {
                        text = text.strikethrough();
                    }
                    ui.add(
                        egui::Label::new(text)
                            .selectable(false)
                            .sense(Sense::click_and_drag()),
                    )
                };
                (icon | label, toggle)
            })
            .inner;
        (inner.0, inner.1, ground)
    });
    let (name_response, toggle, ground) = scope.inner;
    let rect = scope.response.rect;
    let hovered = scope.response.hovered() || name_response.hovered();
    paint_ground(ui, ground, rect, style.selected, hovered, style.depth);
    // The scope's own sense sits below the widgets inside it, so the name
    // answers for itself and the rest of the row answers for the scope.
    let select = scope.response | name_response;
    let drop = toggle
        .clone()
        .map_or_else(|| select.clone(), |toggle| select.clone() | toggle);
    TreeRow {
        select,
        drop,
        toggle,
        rename: match (committed, cancelled) {
            (_, true) => Some(Rename::Cancelled),
            (true, _) => Some(Rename::Committed),
            _ => None,
        },
    }
}

/// The fold control, or the space one would take.
///
/// A row with nothing under it keeps the width rather than shifting its name
/// left, so names line up down the column whatever each row happens to carry.
fn chevron(ui: &mut egui::Ui, style: RowStyle) -> Option<Response> {
    if !style.children.folds() {
        ui.add_space(15.0);
        return None;
    }
    let glyph = if style.children.open() {
        icons::EXPANDED
    } else {
        icons::COLLAPSED
    };
    Some(
        ui.add(
            egui::Button::new(
                glyph
                    .outlined()
                    .rich_text()
                    .size(15.0)
                    .color(color::TEXT_FAINT),
            )
            .frame(false)
            .min_size(Vec2::new(15.0, metric::ROW_HEIGHT - 3.0)),
        )
        .on_hover_text(if style.children.open() {
            "Collapse children"
        } else {
            "Expand children"
        }),
    )
}

/// Space held in the paint list for a row's ground, before the row knows what
/// state it is in.
///
/// A row's background has to be painted under its contents and decided after
/// them, because whether it is hovered is only known once it has been laid out.
/// egui answers this with reserved shape slots; both list widgets use the same
/// two.
#[derive(Clone, Copy)]
pub struct Ground {
    band: ShapeIdx,
    guides: ShapeIdx,
}

/// Reserves the ground of a row about to be drawn.
pub fn reserve(ui: &egui::Ui) -> Ground {
    Ground {
        band: ui.painter().add(Shape::Noop),
        guides: ui.painter().add(Shape::Noop),
    }
}

/// Fills in a reserved ground now that the row's state is known.
pub fn paint_ground(
    ui: &egui::Ui,
    ground: Ground,
    rect: Rect,
    selected: bool,
    hovered: bool,
    depth: usize,
) {
    ui.painter()
        .set(ground.band, band_shape(rect, selected, hovered));
    ui.painter().set(ground.guides, guide_shape(rect, depth));
}

/// The band behind a row: selected, hovered, or nothing at all.
fn band_shape(rect: Rect, selected: bool, hovered: bool) -> Shape {
    let fill = match (selected, hovered) {
        (true, _) => color::EMBER,
        (false, true) => color::EMBER_FAINT,
        (false, false) => return Shape::Noop,
    };
    let mut shapes = vec![Shape::rect_filled(rect, radius_tight(), fill)];
    if selected {
        // The rule is what makes a selected row unmistakable at a glance in a
        // list of forty. A fill alone is a shade of grey away from a hover.
        shapes.push(Shape::rect_filled(
            Rect::from_min_size(rect.min, Vec2::new(metric::SELECT_RULE, rect.height())),
            0.0,
            color::FORGE,
        ));
    }
    Shape::Vec(shapes)
}

/// One hairline per level of nesting, so a deep child can be traced upwards.
fn guide_shape(rect: Rect, depth: usize) -> Shape {
    if depth == 0 {
        return Shape::Noop;
    }
    let stroke = epaint::Stroke::new(1.0, color::LINE_SOFT);
    Shape::Vec(
        (0..depth)
            .map(|level| {
                let x = rect.left() + 12.0 + indent(level, metric::INDENT);
                Shape::line_segment(
                    [egui::pos2(x, rect.top()), egui::pos2(x, rect.bottom())],
                    stroke,
                )
            })
            .collect(),
    )
}

/// A heading that a group of rows hangs under.
///
/// Not a row: nothing selects it and nothing folds it. It is the label for a
/// region of the tree, drawn small and quiet so the rows under it are what the
/// eye lands on.
pub fn group(ui: &mut egui::Ui, icon: MaterialIcon, label: &str) -> Response {
    let width = ui.available_width();
    let scope = ui.scope_builder(UiBuilder::new().sense(Sense::hover()), |ui| {
        ui.set_min_width(width);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 5.0;
            ui.add_space(6.0);
            let icon = ui.add(
                egui::Label::new(
                    icon.outlined()
                        .rich_text()
                        .size(13.0)
                        .color(color::TEXT_FAINT),
                )
                .sense(Sense::hover()),
            );
            let label = ui.add(
                egui::Label::new(
                    RichText::new(label.to_uppercase())
                        .size(text::NOTE)
                        .color(color::TEXT_FAINT),
                )
                .selectable(false)
                .sense(Sense::hover()),
            );
            icon | label
        })
        .inner
    });
    scope.response | scope.inner
}
