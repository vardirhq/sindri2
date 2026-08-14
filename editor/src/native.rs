use std::{
    f32::consts::TAU,
    fs::File,
    io::BufWriter,
    path::{Path, PathBuf},
};

use eframe::{
    egui::{
        self, Align, Color32, FontId, Layout, Pos2, Rect, Response, RichText, Sense, Shape, Stroke,
        StrokeKind, Vec2,
    },
    wgpu,
};
use glam::Vec2 as GlamVec2;
use sindri_core::{SceneDocument, SceneEntity, Transform2D, Transform3D};
use sindri_cube::{DemoScene, FrameTarget, demo_badge_texture, encode_prepared_frame};
use sindri_render::{DepthTarget, SpriteBatchRenderer, TexturedCubeRenderer, Viewport};

const SCENE_JSON: &str = include_str!("../../examples/cube/assets/demo.scene.json");
const ACCENT: Color32 = Color32::from_rgb(244, 120, 72);
const ACCENT_SOFT: Color32 = Color32::from_rgb(92, 50, 43);
const APP_BG: Color32 = Color32::from_rgb(11, 14, 20);
const PANEL_BG: Color32 = Color32::from_rgb(17, 21, 29);
const PANEL_RAISED: Color32 = Color32::from_rgb(23, 28, 38);
const BORDER: Color32 = Color32::from_rgb(42, 49, 62);
const TEXT: Color32 = Color32::from_rgb(225, 229, 237);
const TEXT_MUTED: Color32 = Color32::from_rgb(126, 136, 153);
const INITIAL_VIEWPORT_WIDTH: u32 = 960;
const INITIAL_VIEWPORT_HEIGHT: u32 = 540;

pub fn run() -> eframe::Result {
    let capture_path = capture_path_from_args();
    let options = eframe::NativeOptions {
        renderer: eframe::Renderer::Wgpu,
        viewport: egui::ViewportBuilder::default()
            .with_title("Sindri Editor")
            .with_inner_size([1_440.0, 900.0])
            .with_min_inner_size([1_080.0, 680.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Sindri Editor",
        options,
        Box::new(move |context| Ok(Box::new(EditorApp::new(context, capture_path)))),
    )
}

fn capture_path_from_args() -> Option<PathBuf> {
    let mut arguments = std::env::args().skip(1);
    while let Some(argument) = arguments.next() {
        if argument == "--capture" {
            return Some(PathBuf::from(
                arguments
                    .next()
                    .expect("--capture requires an output PNG path"),
            ));
        }
    }
    None
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum EditorMode {
    Select,
    Move,
    Rotate,
    Scale,
}

struct RuntimeViewport {
    render_state: eframe::egui_wgpu::RenderState,
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    depth: DepthTarget,
    texture_id: egui::TextureId,
    cube_renderer: TexturedCubeRenderer,
    sprite_renderer: SpriteBatchRenderer,
    scene: DemoScene,
    width: u32,
    height: u32,
}

impl RuntimeViewport {
    const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

    fn new(context: &eframe::CreationContext<'_>) -> Result<Self, String> {
        let render_state = context
            .wgpu_render_state
            .clone()
            .ok_or_else(|| "the editor requires eframe's WGPU renderer".to_owned())?;
        let (texture, view) = create_viewport_texture(
            &render_state.device,
            INITIAL_VIEWPORT_WIDTH,
            INITIAL_VIEWPORT_HEIGHT,
        );
        let texture_id = render_state.renderer.write().register_native_texture(
            &render_state.device,
            &view,
            wgpu::FilterMode::Linear,
        );
        let depth = DepthTarget::new(
            &render_state.device,
            INITIAL_VIEWPORT_WIDTH,
            INITIAL_VIEWPORT_HEIGHT,
        );
        let cube_renderer =
            TexturedCubeRenderer::new(&render_state.device, &render_state.queue, Self::FORMAT);
        let sprite_renderer = SpriteBatchRenderer::new(
            &render_state.device,
            Self::FORMAT,
            demo_badge_texture(&render_state.device, &render_state.queue),
        );
        let scene = DemoScene::load().map_err(|error| error.to_string())?;

        Ok(Self {
            render_state,
            texture,
            view,
            depth,
            texture_id,
            cube_renderer,
            sprite_renderer,
            scene,
            width: INITIAL_VIEWPORT_WIDTH,
            height: INITIAL_VIEWPORT_HEIGHT,
        })
    }

    fn render(
        &mut self,
        width: u32,
        height: u32,
        rotation: GlamVec2,
        zoom: f32,
    ) -> Result<(), String> {
        self.resize(width, height);
        debug_assert_eq!(self.texture.width(), self.width);
        debug_assert_eq!(self.texture.height(), self.height);
        let prepared = self
            .scene
            .extract_frame_with_view(Viewport::new(self.width, self.height), rotation, 1.0 / zoom)
            .map_err(|error| error.to_string())?;
        let mut encoder =
            self.render_state
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("Sindri editor runtime viewport encoder"),
                });
        encode_prepared_frame(
            &self.cube_renderer,
            &mut self.sprite_renderer,
            &self.render_state.device,
            &self.render_state.queue,
            &mut encoder,
            FrameTarget {
                color: &self.view,
                depth: &self.depth,
            },
            &prepared,
        )
        .map_err(|error| error.to_string())?;
        self.render_state.queue.submit([encoder.finish()]);
        Ok(())
    }

    fn resize(&mut self, width: u32, height: u32) {
        let width = width.max(1);
        let height = height.max(1);
        if self.width == width && self.height == height {
            return;
        }
        let (texture, view) = create_viewport_texture(&self.render_state.device, width, height);
        self.render_state
            .renderer
            .write()
            .update_egui_texture_from_wgpu_texture(
                &self.render_state.device,
                &view,
                wgpu::FilterMode::Linear,
                self.texture_id,
            );
        self.texture = texture;
        self.view = view;
        self.depth.resize(&self.render_state.device, width, height);
        self.width = width;
        self.height = height;
    }
}

fn create_viewport_texture(
    device: &wgpu::Device,
    width: u32,
    height: u32,
) -> (wgpu::Texture, wgpu::TextureView) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Sindri editor runtime viewport"),
        size: wgpu::Extent3d {
            width: width.max(1),
            height: height.max(1),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: RuntimeViewport::FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[RuntimeViewport::FORMAT],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    (texture, view)
}

struct EditorApp {
    scene: SceneDocument,
    selected: usize,
    search: String,
    mode: EditorMode,
    playing: bool,
    viewport_yaw: f32,
    viewport_pitch: f32,
    viewport_zoom: f32,
    runtime_viewport: RuntimeViewport,
    runtime_error: Option<String>,
    capture_path: Option<PathBuf>,
    capture_requested: bool,
    capture_frame_count: u8,
}

impl EditorApp {
    fn new(context: &eframe::CreationContext<'_>, capture_path: Option<PathBuf>) -> Self {
        configure_theme(&context.egui_ctx);
        let scene: SceneDocument = serde_json::from_str(SCENE_JSON)
            .expect("the embedded editor fixture must remain valid scene JSON");
        scene
            .validate()
            .expect("the embedded editor fixture must remain a valid scene");
        let selected = scene
            .entities
            .iter()
            .position(|entity| entity.id.as_str() == "checker-cube")
            .unwrap_or_default();
        let runtime_viewport = RuntimeViewport::new(context)
            .expect("the native editor must initialize its shared WGPU runtime viewport");

        Self {
            scene,
            selected,
            search: String::new(),
            mode: EditorMode::Select,
            playing: false,
            viewport_yaw: -0.62,
            viewport_pitch: 0.42,
            viewport_zoom: 1.0,
            runtime_viewport,
            runtime_error: None,
            capture_path,
            capture_requested: false,
            capture_frame_count: 0,
        }
    }

    fn handle_capture(&mut self, ui: &egui::Ui) {
        let screenshot = ui.input(|input| {
            input.events.iter().find_map(|event| match event {
                egui::Event::Screenshot { image, .. } => Some(image.clone()),
                _ => None,
            })
        });
        if let (Some(image), Some(path)) = (screenshot, self.capture_path.take()) {
            save_screenshot(&path, &image).expect("failed to save requested editor screenshot");
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    fn request_capture_after_warmup(&mut self, context: &egui::Context) {
        if self.capture_path.is_none() || self.capture_requested {
            return;
        }
        self.capture_frame_count = self.capture_frame_count.saturating_add(1);
        if self.capture_frame_count >= 3 {
            context.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
            self.capture_requested = true;
        }
    }

    fn top_bar(&mut self, ui: &mut egui::Ui) {
        egui::Panel::top("editor-top-bar")
            .exact_size(52.0)
            .frame(
                egui::Frame::new()
                    .fill(PANEL_BG)
                    .stroke(Stroke::new(1.0, BORDER)),
            )
            .show(ui, |ui| {
                ui.add_space(7.0);
                ui.horizontal(|ui| {
                    brand_mark(ui);
                    ui.add_space(8.0);
                    ui.label(RichText::new("SINDRI").strong().size(16.0).color(TEXT));
                    ui.add_space(22.0);
                    ui.label(RichText::new("demo.scene").size(13.0).color(TEXT));
                    ui.label(RichText::new("/").color(TEXT_MUTED));
                    ui.label(RichText::new("Shared 2D + 3D proof").color(TEXT_MUTED));

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let play_label = if self.playing { "Stop" } else { "Play" };
                        if accent_button(ui, play_label).clicked() {
                            self.playing = !self.playing;
                        }
                        ui.add_space(6.0);
                        compact_button(ui, "Redo");
                        compact_button(ui, "Undo");
                        ui.separator();
                        ui.label(
                            RichText::new("Saved")
                                .small()
                                .color(Color32::from_rgb(91, 194, 137)),
                        );
                    });
                });
            });
    }

    fn hierarchy_panel(&mut self, ui: &mut egui::Ui) {
        egui::Panel::left("scene-hierarchy")
            .default_size(252.0)
            .min_size(210.0)
            .max_size(360.0)
            .frame(
                egui::Frame::new()
                    .fill(PANEL_BG)
                    .stroke(Stroke::new(1.0, BORDER)),
            )
            .show(ui, |ui| {
                panel_heading(ui, "SCENE", "8 entities");
                ui.add_space(8.0);
                ui.add_sized(
                    [ui.available_width(), 30.0],
                    egui::TextEdit::singleline(&mut self.search).hint_text("Filter entities..."),
                );
                ui.add_space(10.0);

                let needle = self.search.trim().to_lowercase();
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.label(RichText::new("SCENE ROOT").small().color(TEXT_MUTED));
                    ui.add_space(4.0);
                    for (index, entity) in self.scene.entities.iter().enumerate() {
                        let name = entity_name(entity);
                        if !needle.is_empty() && !name.to_lowercase().contains(&needle) {
                            continue;
                        }
                        let kind = entity_kind(entity);
                        let selected = index == self.selected;
                        let response = hierarchy_row(ui, kind, &name, selected);
                        if response.clicked() {
                            self.selected = index;
                        }
                    }
                });
            });
    }

    fn inspector_panel(&mut self, ui: &mut egui::Ui) {
        egui::Panel::right("entity-inspector")
            .default_size(318.0)
            .min_size(280.0)
            .max_size(430.0)
            .frame(
                egui::Frame::new()
                    .fill(PANEL_BG)
                    .stroke(Stroke::new(1.0, BORDER)),
            )
            .show(ui, |ui| {
                panel_heading(ui, "INSPECTOR", "Entity");
                ui.add_space(12.0);
                if let Some(entity) = self.scene.entities.get_mut(self.selected) {
                    inspector_identity(ui, entity);
                    ui.add_space(10.0);
                    if let Some(transform) = &mut entity.transform_3d {
                        transform_3d_card(ui, transform);
                    }
                    if let Some(transform) = &mut entity.transform_2d {
                        transform_2d_card(ui, transform);
                    }
                    ui.add_space(10.0);
                    components_card(ui, entity);
                    ui.add_space(10.0);
                    ui.add_sized(
                        [ui.available_width(), 34.0],
                        egui::Button::new(RichText::new("+  Add component").color(TEXT_MUTED)),
                    );
                }
            });
    }

    fn status_bar(ui: &mut egui::Ui) {
        egui::Panel::bottom("editor-status")
            .exact_size(28.0)
            .frame(
                egui::Frame::new()
                    .fill(PANEL_BG)
                    .stroke(Stroke::new(1.0, BORDER)),
            )
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("Ready")
                            .small()
                            .color(Color32::from_rgb(91, 194, 137)),
                    );
                    ui.separator();
                    ui.label(RichText::new("wgpu 30").small().color(TEXT_MUTED));
                    ui.label(RichText::new("Vulkan").small().color(TEXT_MUTED));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.label(
                            RichText::new("8 entities  ·  2 cameras  ·  60 FPS")
                                .small()
                                .color(TEXT_MUTED),
                        );
                    });
                });
            });
    }

    fn viewport(&mut self, ui: &mut egui::Ui) {
        let context = ui.ctx().clone();
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(APP_BG))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    mode_button(ui, &mut self.mode, EditorMode::Select, "Select");
                    mode_button(ui, &mut self.mode, EditorMode::Move, "Move");
                    mode_button(ui, &mut self.mode, EditorMode::Rotate, "Rotate");
                    mode_button(ui, &mut self.mode, EditorMode::Scale, "Scale");
                    ui.separator();
                    ui.label(RichText::new("Runtime").small().color(ACCENT));
                    ui.label(RichText::new("Perspective").small().color(TEXT_MUTED));
                    ui.label(RichText::new("Lit").small().color(TEXT_MUTED));
                });
                ui.add_space(6.0);

                let (rect, response) = ui.allocate_exact_size(ui.available_size(), Sense::drag());
                if response.dragged() {
                    let delta = response.drag_motion();
                    self.viewport_yaw = (self.viewport_yaw + delta.x * 0.008) % TAU;
                    self.viewport_pitch = (self.viewport_pitch + delta.y * 0.008).clamp(-1.1, 1.1);
                }
                if response.hovered() {
                    let zoom_delta = context.input(|input| input.smooth_scroll_delta.y);
                    self.viewport_zoom = (self.viewport_zoom + zoom_delta * 0.002).clamp(0.65, 1.8);
                }
                let pixels_per_point = context.pixels_per_point();
                let viewport_width = physical_viewport_dimension(rect.width(), pixels_per_point);
                let viewport_height = physical_viewport_dimension(rect.height(), pixels_per_point);
                self.runtime_error = self
                    .runtime_viewport
                    .render(
                        viewport_width,
                        viewport_height,
                        GlamVec2::new(self.viewport_yaw, self.viewport_pitch),
                        self.viewport_zoom,
                    )
                    .err();
                ui.painter().image(
                    self.runtime_viewport.texture_id,
                    rect,
                    Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                    Color32::WHITE,
                );
                paint_runtime_overlay(
                    ui.painter(),
                    rect,
                    &entity_name(&self.scene.entities[self.selected]),
                    self.runtime_error.as_deref(),
                );
                context.request_repaint();
            });
    }
}

impl eframe::App for EditorApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.handle_capture(ui);
        self.top_bar(ui);
        Self::status_bar(ui);
        self.hierarchy_panel(ui);
        self.inspector_panel(ui);
        self.viewport(ui);
        self.request_capture_after_warmup(ui.ctx());
    }
}

fn save_screenshot(path: &Path, image: &egui::ColorImage) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let file = File::create(path).map_err(|error| error.to_string())?;
    let width = u32::try_from(image.size[0]).map_err(|error| error.to_string())?;
    let height = u32::try_from(image.size[1]).map_err(|error| error.to_string())?;
    let mut encoder = png::Encoder::new(BufWriter::new(file), width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().map_err(|error| error.to_string())?;
    let pixels = image
        .pixels
        .iter()
        .flat_map(Color32::to_array)
        .collect::<Vec<_>>();
    writer
        .write_image_data(&pixels)
        .map_err(|error| error.to_string())
}

fn configure_theme(context: &egui::Context) {
    context.set_theme(egui::Theme::Dark);
    context.all_styles_mut(|style| {
        style.spacing.item_spacing = Vec2::new(8.0, 7.0);
        style.spacing.button_padding = Vec2::new(10.0, 5.0);
        style.visuals.dark_mode = true;
        style.visuals.panel_fill = PANEL_BG;
        style.visuals.window_fill = PANEL_RAISED;
        style.visuals.extreme_bg_color = Color32::from_rgb(12, 16, 23);
        style.visuals.faint_bg_color = Color32::from_rgb(28, 34, 45);
        style.visuals.selection.bg_fill = ACCENT_SOFT;
        style.visuals.selection.stroke = Stroke::new(1.0, ACCENT);
        style.visuals.widgets.inactive.bg_fill = PANEL_RAISED;
        style.visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, BORDER);
        style.visuals.widgets.hovered.bg_fill = Color32::from_rgb(33, 40, 52);
        style.visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, Color32::from_rgb(65, 75, 92));
        style.visuals.widgets.active.bg_fill = ACCENT_SOFT;
        style.visuals.widgets.active.bg_stroke = Stroke::new(1.0, ACCENT);
    });
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss
)]
fn physical_viewport_dimension(points: f32, pixels_per_point: f32) -> u32 {
    (points * pixels_per_point)
        .round()
        .clamp(1.0, u32::MAX as f32) as u32
}

fn brand_mark(ui: &mut egui::Ui) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(28.0), Sense::hover());
    let center = rect.center();
    let points = vec![
        Pos2::new(center.x, rect.top() + 3.0),
        Pos2::new(rect.right() - 3.0, center.y),
        Pos2::new(center.x, rect.bottom() - 3.0),
        Pos2::new(rect.left() + 3.0, center.y),
    ];
    ui.painter().add(Shape::convex_polygon(
        points,
        ACCENT,
        Stroke::new(1.0, Color32::from_rgb(255, 174, 129)),
    ));
    ui.painter().circle_filled(center, 4.0, PANEL_BG);
}

fn panel_heading(ui: &mut egui::Ui, title: &str, detail: &str) {
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        ui.label(RichText::new(title).strong().size(12.0).color(TEXT));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.label(RichText::new(detail).small().color(TEXT_MUTED));
        });
    });
}

fn compact_button(ui: &mut egui::Ui, label: &str) -> Response {
    ui.add_sized(
        [58.0, 30.0],
        egui::Button::new(RichText::new(label).small()),
    )
}

fn accent_button(ui: &mut egui::Ui, label: &str) -> Response {
    ui.add_sized(
        [76.0, 32.0],
        egui::Button::new(RichText::new(label).strong().color(Color32::WHITE)).fill(ACCENT),
    )
}

fn mode_button(ui: &mut egui::Ui, mode: &mut EditorMode, value: EditorMode, label: &str) {
    let selected = *mode == value;
    let button = egui::Button::new(RichText::new(label).small().color(if selected {
        TEXT
    } else {
        TEXT_MUTED
    }))
    .selected(selected);
    if ui.add(button).clicked() {
        *mode = value;
    }
}

fn hierarchy_row(ui: &mut egui::Ui, kind: &str, name: &str, selected: bool) -> Response {
    let text = format!("{kind}   {name}");
    ui.add_sized(
        [ui.available_width(), 30.0],
        egui::Button::new(RichText::new(text).color(if selected {
            TEXT
        } else {
            Color32::from_rgb(186, 193, 205)
        }))
        .selected(selected),
    )
}

fn inspector_identity(ui: &mut egui::Ui, entity: &mut SceneEntity) {
    ui.label(RichText::new(entity_kind(entity)).small().color(ACCENT));
    let name = entity
        .name
        .get_or_insert_with(|| entity.id.as_str().to_owned());
    ui.add_sized(
        [ui.available_width(), 34.0],
        egui::TextEdit::singleline(name).font(FontId::proportional(17.0)),
    );
    ui.label(RichText::new(entity.id.as_str()).small().color(TEXT_MUTED));
}

fn card(ui: &mut egui::Ui, title: &str, content: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::new()
        .fill(PANEL_RAISED)
        .stroke(Stroke::new(1.0, BORDER))
        .corner_radius(6)
        .inner_margin(10)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new(title).strong().size(12.0).color(TEXT));
            ui.separator();
            content(ui);
        });
}

fn transform_3d_card(ui: &mut egui::Ui, transform: &mut Transform3D) {
    card(ui, "TRANSFORM 3D", |ui| {
        vector_row(ui, "Position", &mut transform.position);
        vector_row(ui, "Scale", &mut transform.scale);
        ui.horizontal(|ui| {
            ui.label(RichText::new("Rotation").small().color(TEXT_MUTED));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(RichText::new("Quaternion").small().color(TEXT_MUTED));
            });
        });
    });
}

fn transform_2d_card(ui: &mut egui::Ui, transform: &mut Transform2D) {
    card(ui, "TRANSFORM 2D", |ui| {
        vector_row_2d(ui, "Position", &mut transform.position);
        vector_row_2d(ui, "Scale", &mut transform.scale);
        ui.horizontal(|ui| {
            ui.label(RichText::new("Rotation").small().color(TEXT_MUTED));
            ui.add(egui::DragValue::new(&mut transform.rotation_radians).speed(0.01));
        });
    });
}

fn vector_row(ui: &mut egui::Ui, label: &str, values: &mut [f32; 3]) {
    ui.label(RichText::new(label).small().color(TEXT_MUTED));
    ui.columns(3, |columns| {
        for (index, column) in columns.iter_mut().enumerate() {
            column.horizontal(|ui| {
                let axis = ["X", "Y", "Z"][index];
                let color = [
                    Color32::from_rgb(239, 92, 101),
                    Color32::from_rgb(89, 201, 135),
                    Color32::from_rgb(91, 151, 239),
                ][index];
                ui.label(RichText::new(axis).strong().small().color(color));
                ui.add(egui::DragValue::new(&mut values[index]).speed(0.05));
            });
        }
    });
}

fn vector_row_2d(ui: &mut egui::Ui, label: &str, values: &mut [f32; 2]) {
    ui.label(RichText::new(label).small().color(TEXT_MUTED));
    ui.columns(2, |columns| {
        for (index, column) in columns.iter_mut().enumerate() {
            column.horizontal(|ui| {
                let axis = ["X", "Y"][index];
                let color = [
                    Color32::from_rgb(239, 92, 101),
                    Color32::from_rgb(89, 201, 135),
                ][index];
                ui.label(RichText::new(axis).strong().small().color(color));
                ui.add(egui::DragValue::new(&mut values[index]).speed(0.05));
            });
        }
    });
}

fn components_card(ui: &mut egui::Ui, entity: &SceneEntity) {
    card(ui, "COMPONENTS", |ui| {
        if entity.components.is_empty() {
            ui.label(
                RichText::new("No registered components")
                    .small()
                    .color(TEXT_MUTED),
            );
        } else {
            for name in entity.components.keys() {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("●").small().color(ACCENT));
                    ui.label(RichText::new(component_label(name)).color(TEXT));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.label(RichText::new("v1").small().color(TEXT_MUTED));
                    });
                });
            }
        }
    });
}

fn paint_runtime_overlay(
    painter: &egui::Painter,
    rect: Rect,
    selected_name: &str,
    runtime_error: Option<&str>,
) {
    painter.rect_stroke(
        rect,
        0.0,
        Stroke::new(1.0, Color32::from_rgb(48, 58, 74)),
        StrokeKind::Inside,
    );
    let label_rect = Rect::from_min_size(rect.min + Vec2::new(14.0, 14.0), Vec2::new(250.0, 50.0));
    painter.rect_filled(label_rect, 5.0, Color32::from_black_alpha(150));
    painter.text(
        label_rect.min + Vec2::new(11.0, 9.0),
        egui::Align2::LEFT_TOP,
        selected_name,
        FontId::proportional(13.0),
        TEXT,
    );
    painter.text(
        label_rect.min + Vec2::new(11.0, 28.0),
        egui::Align2::LEFT_TOP,
        "Live runtime  ·  Drag to rotate  ·  Scroll to zoom",
        FontId::proportional(10.0),
        TEXT_MUTED,
    );
    if let Some(error) = runtime_error {
        let error_rect = Rect::from_min_size(
            Pos2::new(rect.left() + 14.0, rect.bottom() - 48.0),
            Vec2::new((rect.width() - 28.0).max(1.0), 34.0),
        );
        painter.rect_filled(error_rect, 5.0, Color32::from_rgb(82, 30, 36));
        painter.text(
            error_rect.left_center() + Vec2::new(10.0, 0.0),
            egui::Align2::LEFT_CENTER,
            error,
            FontId::proportional(11.0),
            Color32::from_rgb(255, 184, 191),
        );
    }
    paint_axis_gizmo(painter, Pos2::new(rect.left() + 38.0, rect.bottom() - 38.0));
}

fn paint_axis_gizmo(painter: &egui::Painter, origin: Pos2) {
    let axes = [
        (Vec2::new(19.0, 7.0), Color32::from_rgb(239, 92, 101), "X"),
        (Vec2::new(0.0, -22.0), Color32::from_rgb(89, 201, 135), "Y"),
        (Vec2::new(-14.0, 11.0), Color32::from_rgb(91, 151, 239), "Z"),
    ];
    painter.circle_filled(origin, 3.0, TEXT);
    for (offset, color, label) in axes {
        let end = origin + offset;
        painter.line_segment([origin, end], Stroke::new(2.0, color));
        painter.text(
            end,
            egui::Align2::CENTER_CENTER,
            label,
            FontId::proportional(10.0),
            color,
        );
    }
}

fn entity_name(entity: &SceneEntity) -> String {
    entity
        .name
        .clone()
        .unwrap_or_else(|| humanize(entity.id.as_str()))
}

fn entity_kind(entity: &SceneEntity) -> &'static str {
    if entity.components.contains_key("sindri.camera") {
        "CAM"
    } else if entity.components.contains_key("sindri.mesh") {
        "MESH"
    } else if entity.components.contains_key("sindri.sprite") {
        "SPR"
    } else {
        "ENT"
    }
}

fn component_label(name: &str) -> String {
    humanize(name.strip_prefix("sindri.").unwrap_or(name))
}

fn humanize(value: &str) -> String {
    value
        .split(['-', '_', '.'])
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().chain(chars).collect(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_scene_is_valid_and_contains_editor_selection() {
        let scene: SceneDocument = serde_json::from_str(SCENE_JSON).unwrap();
        scene.validate().unwrap();
        assert!(
            scene
                .entities
                .iter()
                .any(|entity| entity.id.as_str() == "checker-cube")
        );
    }

    #[test]
    fn labels_are_human_readable() {
        assert_eq!(humanize("checker-cube"), "Checker Cube");
        assert_eq!(component_label("sindri.sprite"), "Sprite");
    }
}
