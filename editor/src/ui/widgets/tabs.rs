//! Tabs: how every docked panel is named, selected, and picked up.
//!
//! There is one strip in the editor and every group wears it, because a tab is
//! no longer a choice between two fixed views — it is what a panel *is* once
//! the arrangement became something the user drags into shape. So the tab is
//! drawn as a surface selector: an icon, a name, a lit ground, and a rule along
//! the bottom that reads as the active surface continuing into the view
//! underneath it. The centre gets the same shape at a larger size, because what
//! separates a viewport from a side panel is emphasis, not kind.

use eframe::egui::{
    self, Align, Align2, FontId, Layout, Pos2, Rect, Response, Sense, UiBuilder, Vec2,
};
use egui_material_icons::MaterialIcon;

use crate::ui::theme::{color, hairline, metric, text};

/// How prominent a strip of tabs is.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Weight {
    /// The centre, where the work is.
    Primary,
    /// A docked side or bottom group.
    Secondary,
}

/// One height for every strip, so a workspace tab and a panel header sit on the
/// same baseline across the window. What separates the two weights is emphasis,
/// not size.
const STRIP_HEIGHT: f32 = metric::HEADER_HEIGHT;

impl Weight {
    const fn font(self) -> f32 {
        match self {
            Self::Primary => text::BODY,
            Self::Secondary => text::HEADING,
        }
    }
}

/// A strip of tabs with the baseline rule drawn across the whole width.
///
/// The rule is the strip's, not each tab's: drawn per tab it stopped where the
/// last label did, which made a row of tabs look like it was missing something.
///
/// Returns the strip's own rectangle, which is what a dock drop is resolved
/// against: a tab released over a strip joins that group, and the caller cannot
/// work out where the strip ended up without being told.
pub fn strip(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui)) -> Rect {
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, STRIP_HEIGHT), Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0.0, color::HEADER);
    painter.hline(rect.x_range(), rect.bottom() - 0.5, hairline());
    let mut content = ui.new_child(
        UiBuilder::new()
            .max_rect(rect)
            .layout(Layout::left_to_right(Align::Center)),
    );
    content.spacing_mut().item_spacing.x = 0.0;
    add(&mut content);
    rect
}

/// One tab. Returns its response so the caller decides what selecting means.
///
/// Senses dragging as well as clicking, because a tab is also the handle the
/// panel is moved by. `travelling` dims the one being dragged, so the strip
/// shows where it came from while the pointer carries a copy of it.
pub fn tab(
    ui: &mut egui::Ui,
    weight: Weight,
    selected: bool,
    icon: Option<MaterialIcon>,
    label: &str,
    travelling: bool,
) -> Response {
    let font = FontId::proportional(weight.font());
    // Measured rather than guessed from character count: the tab is painted, so
    // nothing else would stop a long name running past its own ground.
    // Laid out with the placeholder colour so the tint below is the one that
    // takes effect: a galley built with a real colour keeps it.
    let galley = ui
        .painter()
        .layout_no_wrap(label.to_owned(), font, egui::Color32::PLACEHOLDER);
    let icon_width = if icon.is_some() { 20.0 } else { 0.0 };
    let width = galley.size().x + icon_width + 26.0;
    let (rect, response) = ui.allocate_exact_size(
        Vec2::new(width, ui.available_height()),
        Sense::click_and_drag(),
    );
    let painter = ui.painter_at(rect);
    let hovered = response.hovered();
    if selected {
        painter.rect_filled(rect, 0.0, color::PANEL);
        // The lit rule sits on the baseline the strip drew, so the active tab
        // reads as joined to the surface below it.
        painter.rect_filled(
            Rect::from_min_size(
                Pos2::new(rect.left(), rect.bottom() - 2.0),
                Vec2::new(rect.width(), 2.0),
            ),
            0.0,
            color::FORGE,
        );
    } else if hovered {
        painter.rect_filled(rect, 0.0, color::RAISED);
    }
    let mut foreground = match (selected, hovered) {
        (true, _) => color::TEXT,
        (false, true) => color::TEXT_MUTED,
        (false, false) => color::TEXT_FAINT,
    };
    if travelling {
        foreground = foreground.gamma_multiply(0.4);
    }
    let mut cursor = rect.left() + 13.0;
    if let Some(icon) = icon {
        painter.text(
            Pos2::new(cursor, rect.center().y),
            Align2::LEFT_CENTER,
            icon.outlined().codepoint,
            FontId::new(15.0, icon.outlined().font_family()),
            if selected { color::FORGE } else { foreground },
        );
        cursor += icon_width;
    }
    painter.galley(
        Pos2::new(cursor, rect.center().y - galley.size().y / 2.0),
        galley,
        foreground,
    );
    response
}
