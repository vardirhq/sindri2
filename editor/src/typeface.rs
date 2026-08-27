//! Seeing a font before naming it in a component.
//!
//! You picked one by filename. A project holding four typefaces gave four rows
//! that differed only in what they were called, and the way to find out which
//! was which was to put one in a `sindri.ui.text` and look at the viewport.
//!
//! egui carries its own font stack, so the sample is drawn by the editor rather
//! than by the engine's text renderer: the panel is egui, and asking the scene's
//! renderer for a picture of a string to put in a dock would be a second text
//! pipeline for one label.

use std::path::{Path, PathBuf};

use eframe::egui;

/// A project font registered with egui so a sample can be drawn in it.
///
/// One at a time, and replaced rather than accumulated. Registering fonts costs
/// memory that lives as long as the context, and a project with forty of them
/// browsed one after another would keep all forty.
#[derive(Default)]
pub struct Typeface {
    path: Option<PathBuf>,
    /// Whether the last file handed over was actually a font egui could read.
    loaded: bool,
}

/// What the sample family is called inside egui.
///
/// A fixed name, because there is only ever one: the previous font's data is
/// dropped when the next arrives.
const FAMILY: &str = "sindri-project-font";

impl Typeface {
    /// Reads a font and makes it the one a sample is drawn in.
    ///
    /// Idempotent per path, so the panel can call it every frame it is showing
    /// a font without re-reading the file sixty times a second.
    pub fn show(&mut self, context: &egui::Context, path: &Path) {
        if self.path.as_deref() == Some(path) {
            return;
        }
        self.path = Some(path.to_path_buf());
        self.loaded = false;
        let Ok(bytes) = std::fs::read(path) else {
            return;
        };
        // `add_font` rather than `set_fonts`. Replacing the definitions wipes
        // every other font the context holds — the interface face and the icon
        // families the whole editor draws with — and the replacement lands a
        // frame later than the additions that were meant to survive it, so the
        // next frame panics with "material-icons-outlined is not bound to any
        // fonts". Inserting adds one family and disturbs nothing.
        context.add_font(egui::epaint::text::FontInsert::new(
            FAMILY,
            egui::FontData::from_owned(bytes),
            vec![egui::epaint::text::InsertFontFamily {
                family: egui::FontFamily::Name(FAMILY.into()),
                priority: egui::epaint::text::FontPriority::Highest,
            }],
        ));
        self.loaded = true;
    }

    /// The family a sample should be laid out in, or `None` when there is not
    /// one to lay it out in yet.
    ///
    /// Asked of the context rather than assumed, because `set_fonts` takes
    /// effect at the start of the *next* frame: laying text out in the family
    /// on the frame it was registered panics with "is not bound to any fonts",
    /// which is what it did.
    ///
    /// `None` is also the answer for a file that was not a font egui could
    /// read, and the panel says so: a `.ttf` that is actually a text file is
    /// exactly the sort of thing a preview exists to reveal, and drawing the
    /// sample in the editor's own face would hide it.
    pub fn family(&self, context: &egui::Context) -> Option<egui::FontFamily> {
        if !self.loaded {
            return None;
        }
        let family = egui::FontFamily::Name(FAMILY.into());
        context
            .fonts(|fonts| fonts.families().contains(&family))
            .then_some(family)
    }

    /// Whether a font was read and is only waiting for the next frame.
    ///
    /// The difference between "not a font" and "not yet", which are the same
    /// `None` from [`Self::family`] and different things to say.
    pub const fn pending(&self) -> bool {
        self.loaded
    }

    /// Forgets which font is being shown.
    ///
    /// The data stays registered with the context, because taking it out means
    /// replacing the definitions and that is what broke the icons. One family's
    /// worth of glyphs per font browsed is a bounded cost — the family is
    /// reused, so it is one font's data at a time, not one per file looked at.
    pub fn forget(&mut self) {
        self.path = None;
        self.loaded = false;
    }
}

/// Whether the editor can draw a sample of this file.
pub fn is_a_typeface(path: &Path) -> bool {
    let extension = path
        .extension()
        .map(|extension| extension.to_string_lossy().to_lowercase());
    matches!(extension.as_deref(), Some("ttf" | "otf"))
}

/// What a sample says.
///
/// Letters, digits and the punctuation a HUD actually uses, rather than a
/// pangram: what someone is deciding is whether this face suits a score and a
/// title, and "0 1 2 3" is the half of that a pangram leaves out.
pub const SAMPLE: &str = "The quick brown fox\n0123456789  ·  +-×÷  ?!";

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::is_a_typeface;

    /// The two egui reads. A `.woff` is a font and not one it can load, so
    /// offering a sample of it would be offering a box of tofu.
    #[test]
    fn only_the_faces_egui_reads_are_offered() {
        assert!(is_a_typeface(Path::new("fonts/Inter.ttf")));
        assert!(is_a_typeface(Path::new("fonts/Inter.OTF")));
        for name in ["fonts/Inter.woff2", "fonts/Inter-OFL.txt", "orb.png"] {
            assert!(!is_a_typeface(Path::new(name)), "{name}");
        }
    }
}
