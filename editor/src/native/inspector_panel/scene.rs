//! What the inspector shows when nothing in the scene is selected.
//!
//! There is no surface in the editor that is about the *scene* rather than
//! about an entity in it, so `SceneMetadata.name` — a real field that
//! round-trips through a save, and reads "Gather" in the shipped scene — was
//! neither shown nor editable anywhere.
//!
//! Here, because an inspector with nothing selected already has the room and
//! was spending it on a shrug. What is being edited is still "the thing in
//! focus"; with no entity in focus, that is the scene.

use eframe::egui::{self, FontId, Stroke};

use crate::ui::icons;
use crate::ui::theme::{color, metric, radius, text};
use crate::ui::widgets::{panel, property};

/// The scene as the panel hands it over.
pub(super) struct SceneSummary<'a> {
    /// The scene's own name, which is not its file name.
    pub(super) name: &'a mut String,
    /// What the file is called, or `None` for a scene with no file yet.
    pub(super) file: Option<&'a str>,
    pub(super) entities: usize,
}

/// The scene's own panel, reporting whether its name is finished being edited.
///
/// Committed on the way out rather than as it is typed, like the stable ID
/// beside an entity: a name written every keystroke is one undo step per
/// letter.
pub(super) fn scene_section(ui: &mut egui::Ui, scene: SceneSummary<'_>) -> bool {
    let SceneSummary {
        name,
        file,
        entities,
    } = scene;
    let mut finished = false;
    egui::Frame::new()
        .fill(color::RAISED)
        .stroke(Stroke::new(1.0, color::LINE_SOFT))
        .corner_radius(radius())
        .inner_margin(egui::Margin::symmetric(8, 7))
        .outer_margin(egui::Margin::symmetric(metric::GUTTER_EDGE, 6))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 7.0;
                ui.label(
                    icons::SCENE
                        .outlined()
                        .rich_text()
                        .size(19.0)
                        .color(color::FORGE),
                );
                let field = ui.add_sized(
                    [ui.available_width(), metric::CONTROL_HEIGHT + 4.0],
                    egui::TextEdit::singleline(name)
                        .font(FontId::proportional(text::BODY + 1.0))
                        .hint_text("Unnamed scene"),
                );
                finished = field.lost_focus();
                field.on_hover_text(
                    "The scene's own name, saved with it and read by whatever loads it",
                );
            });
        });
    property::readout(
        ui,
        "File",
        file.unwrap_or("not saved anywhere yet"),
        Some("Where this scene is written, which is not what it is called"),
    );
    property::readout(ui, "Entities", &entities.to_string(), None);
    ui.add_space(10.0);
    panel::note(
        ui,
        "Pick an entity in the hierarchy, or an image in the project, to edit it here.",
    );
    finished
}
