//! Opening the editor's window.

use eframe::egui;

use super::EditorApp;

pub fn run() -> eframe::Result {
    let options = eframe::NativeOptions {
        renderer: eframe::Renderer::Wgpu,
        viewport: egui::ViewportBuilder::default()
            .with_title("Sindri Editor")
            .with_inner_size([1_440.0, 1_024.0])
            .with_min_inner_size([1_100.0, 720.0])
            // Hidden until there is something to show. A launch that opens the
            // welcome window would otherwise flash an empty editor up behind
            // it, and the first frame is where the editor learns which of the
            // two this launch is: the preferences it decides from live in
            // eframe's storage, which does not exist until the app is built.
            //
            // eframe paints a hidden window directly, ten times a second, so
            // that a `Visible` command still reaches it. That is what makes
            // this safe rather than a window that can never be shown again.
            .with_visible(false),
        ..Default::default()
    };
    eframe::run_native(
        "Sindri Editor",
        options,
        Box::new(|context| Ok(Box::new(EditorApp::new(context)))),
    )
}
