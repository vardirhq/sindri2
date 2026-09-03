//! Looking at the game the shape a player will.

use eframe::egui::{self, Rect};

/// A screen the game might be played on.
///
/// The Game view is otherwise whatever shape the panel happens to be, which is
/// a shape nobody plays on: a designer arranging a menu in a wide editor panel
/// has no way to find out that it runs off the side of a phone except by
/// building for a phone and looking. The overlay is authored in units that
/// depend on the aspect ratio, so the aspect ratio is the thing to be able to
/// change.
///
/// Sizes are logical pixels — the numbers a browser reports, not the physical
/// ones a device has — because that is what a scene is laid out against.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct DevicePreview {
    pub(super) name: &'static str,
    /// `None` for whatever shape the panel is, which is the default and the
    /// only honest answer while the editor window is the only screen.
    pub(super) size: Option<(f32, f32)>,
}

impl DevicePreview {
    /// The shapes worth checking, widest first.
    ///
    /// A short list on purpose. Every entry is a shape that behaves
    /// differently — a wide desktop, a squarer laptop, a tall phone, a phone
    /// turned sideways, a tablet — rather than a catalogue of handsets that
    /// differ by a few pixels and tell a designer nothing new.
    pub(super) const ALL: [Self; 6] = [
        Self {
            name: "Free",
            size: None,
        },
        Self {
            name: "Desktop 16:9",
            size: Some((1920.0, 1080.0)),
        },
        Self {
            name: "Laptop 16:10",
            size: Some((1440.0, 900.0)),
        },
        Self {
            name: "Tablet portrait",
            size: Some((820.0, 1180.0)),
        },
        Self {
            name: "Phone portrait",
            size: Some((390.0, 844.0)),
        },
        Self {
            name: "Phone landscape",
            size: Some((844.0, 390.0)),
        },
    ];

    /// How wide this is for each unit tall, or `None` when it is free.
    pub(super) fn aspect(self) -> Option<f32> {
        self.size
            .filter(|(_, height)| *height > 0.0)
            .map(|(width, height)| width / height)
    }

    /// The largest rectangle of this shape that fits inside `available`.
    ///
    /// Centred, because a preview pinned to a corner reads as a bug in the
    /// layout rather than as a choice about the screen.
    pub(super) fn fit(self, available: Rect) -> Rect {
        let Some(aspect) = self.aspect() else {
            return available;
        };
        if available.width() <= 0.0 || available.height() <= 0.0 {
            return available;
        }
        let (width, height) = if available.width() / available.height() > aspect {
            (available.height() * aspect, available.height())
        } else {
            (available.width(), available.width() / aspect)
        };
        Rect::from_center_size(available.center(), eframe::egui::vec2(width, height))
    }

    /// What to say about the shape, for someone deciding whether to trust it.
    pub(super) fn note(self) -> String {
        match self.size {
            Some((width, height)) => format!("{width:.0}×{height:.0}"),
            None => "the panel's own shape".to_owned(),
        }
    }
}

impl Default for DevicePreview {
    fn default() -> Self {
        Self::ALL[0]
    }
}

use crate::ui::theme::{color, metric, text};
use crate::ui::widgets::toolbar;

impl super::EditorApp {
    /// The Game view's own strip: which screen it is standing in for.
    ///
    /// Its own row rather than something painted over the view, for the reason
    /// the Game view has no chrome at all — a control drawn across what the
    /// player would see makes it a picture of something else.
    pub(super) fn game_tools(&mut self, ui: &mut egui::Ui) {
        toolbar::strip(ui, metric::TOOLBAR_HEIGHT, |ui| {
            ui.label(
                egui::RichText::new("Screen")
                    .size(text::NOTE)
                    .color(color::TEXT_FAINT),
            );
            ui.add_space(metric::GAP);
            egui::ComboBox::from_id_salt("game-device")
                .selected_text(self.game_device.name)
                .show_ui(ui, |ui| {
                    for device in DevicePreview::ALL {
                        ui.selectable_value(&mut self.game_device, device, device.name);
                    }
                });
            if let Some((width, height)) = self.game_device.size {
                ui.add_space(metric::GROUP_GAP);
                toolbar::readout(ui, "size", &format!("{width:.0}×{height:.0}"), true);
                ui.add_space(metric::GROUP_GAP);
                ui.label(
                    egui::RichText::new(
                        "The overlay is as wide as the aspect, so a narrow screen \
                         is the one to arrange against",
                    )
                    .size(text::NOTE)
                    .color(color::TEXT_FAINT),
                );
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::DevicePreview;
    use eframe::egui::{Pos2, Rect, vec2};

    fn available() -> Rect {
        Rect::from_min_size(Pos2::new(10.0, 20.0), vec2(800.0, 400.0))
    }

    /// A tall shape in a wide panel is limited by the height, and the space it
    /// cannot use is on both sides rather than one.
    #[test]
    fn a_phone_in_a_wide_panel_is_centred_and_as_tall_as_it_can_be() {
        let phone = DevicePreview::ALL[4];
        let rect = phone.fit(available());
        assert!((rect.height() - 400.0).abs() < 1.0e-3, "{rect:?}");
        let aspect = phone.aspect().expect("a phone has a shape");
        assert!((rect.width() - 400.0 * aspect).abs() < 1.0e-3, "{rect:?}");
        assert!((rect.center() - available().center()).length() < 1.0e-3);
    }

    /// A wide shape in a wide panel is limited by the width instead.
    #[test]
    fn a_desktop_in_a_shorter_panel_is_limited_by_its_width() {
        let desktop = DevicePreview::ALL[1];
        let rect = desktop.fit(Rect::from_min_size(Pos2::ZERO, vec2(800.0, 800.0)));
        assert!((rect.width() - 800.0).abs() < 1.0e-3, "{rect:?}");
        assert!((rect.height() - 450.0).abs() < 1.0e-3, "{rect:?}");
    }

    /// Free takes everything, which is what it means.
    #[test]
    fn a_free_shape_is_the_panel() {
        assert_eq!(DevicePreview::default().fit(available()), available());
        assert_eq!(DevicePreview::default().aspect(), None);
    }

    /// A panel with no area cannot be divided, and must not produce a rect with
    /// a negative or infinite side.
    #[test]
    fn an_empty_panel_stays_empty() {
        let empty = Rect::from_min_size(Pos2::ZERO, vec2(0.0, 0.0));
        for device in DevicePreview::ALL {
            let rect = device.fit(empty);
            assert!(
                rect.width().is_finite() && rect.height().is_finite(),
                "{device:?}"
            );
            assert!(rect.width() >= 0.0 && rect.height() >= 0.0, "{device:?}");
        }
    }
}
