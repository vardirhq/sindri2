use std::{f32::consts::TAU, sync::Arc};

use eframe::{
    egui::{
        self, Align, Color32, FontData, FontFamily, FontId, Layout, Pos2, Rect, Response, RichText,
        Sense, Stroke, StrokeKind, TextStyle, Vec2,
    },
    wgpu,
};
use egui_material_icons::{
    MaterialIcon,
    icons::{
        ICON_3D_ROTATION, ICON_ACCOUNT_TREE, ICON_ADD, ICON_ARROW_SELECTOR_TOOL, ICON_CAMERA_ALT,
        ICON_CODE, ICON_DEPLOYED_CODE, ICON_DESCRIPTION, ICON_EXPAND_MORE, ICON_FILTER_LIST,
        ICON_FOLDER, ICON_GRID_VIEW, ICON_IMAGE, ICON_LIGHT_MODE, ICON_MORE_VERT, ICON_OPEN_WITH,
        ICON_PAUSE, ICON_PLAY_ARROW, ICON_SEARCH, ICON_SETTINGS, ICON_SKIP_NEXT, ICON_TUNE,
        ICON_VIEW_IN_AR, ICON_VIEW_LIST,
    },
};
use glam::Vec2 as GlamVec2;
use sindri_core::{SceneDocument, SceneEntity, Transform2D, Transform3D};
use sindri_cube::{
    DemoScene, FrameTarget, WorldProjection, demo_badge_texture, encode_prepared_frame,
};
use sindri_render::{DepthTarget, SpriteBatchRenderer, TexturedCubeRenderer, Viewport};

const SCENE_JSON: &str = include_str!("../../examples/cube/assets/demo.scene.json");
const INTER_FONT: &[u8] = include_bytes!("../assets/Inter.ttf");
const ACCENT: Color32 = Color32::from_rgb(246, 169, 35);
const ACCENT_BRIGHT: Color32 = Color32::from_rgb(255, 187, 54);
const ACCENT_SOFT: Color32 = Color32::from_rgb(59, 45, 20);
const APP_BG: Color32 = Color32::from_rgb(9, 12, 16);
const TOP_BG: Color32 = Color32::from_rgb(12, 15, 19);
const PANEL_BG: Color32 = Color32::from_rgb(15, 19, 23);
const PANEL_RAISED: Color32 = Color32::from_rgb(19, 24, 29);
const FIELD_BG: Color32 = Color32::from_rgb(12, 16, 20);
const BORDER: Color32 = Color32::from_rgb(39, 46, 53);
const BORDER_SUBTLE: Color32 = Color32::from_rgb(29, 35, 41);
const TEXT: Color32 = Color32::from_rgb(224, 228, 231);
const TEXT_MUTED: Color32 = Color32::from_rgb(143, 151, 159);
const TEXT_FAINT: Color32 = Color32::from_rgb(92, 101, 110);
const SUCCESS: Color32 = Color32::from_rgb(98, 202, 122);
const INITIAL_VIEWPORT_WIDTH: u32 = 960;
const INITIAL_VIEWPORT_HEIGHT: u32 = 540;

pub fn run() -> eframe::Result {
    let options = eframe::NativeOptions {
        renderer: eframe::Renderer::Wgpu,
        viewport: egui::ViewportBuilder::default()
            .with_title("Sindri Editor")
            .with_inner_size([1_440.0, 1_024.0])
            .with_min_inner_size([1_100.0, 720.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Sindri Editor",
        options,
        Box::new(|context| Ok(Box::new(EditorApp::new(context)))),
    )
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum EditorMode {
    Select,
    Move,
    Rotate,
    Scale,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WorkspaceTab {
    Scene,
    Game,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BottomTab {
    Project,
    Console,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CameraProjection {
    Perspective,
    Orthographic,
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
        projection: CameraProjection,
    ) -> Result<(), String> {
        self.resize(width, height);
        let prepared = self
            .scene
            .extract_editor_frame(
                Viewport::new(self.width, self.height),
                rotation,
                1.0 / zoom,
                match projection {
                    CameraProjection::Perspective => WorldProjection::Perspective,
                    CameraProjection::Orthographic => WorldProjection::Orthographic,
                },
            )
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
    asset_search: String,
    mode: EditorMode,
    workspace_tab: WorkspaceTab,
    bottom_tab: BottomTab,
    projection: CameraProjection,
    playing: bool,
    viewport_yaw: f32,
    viewport_pitch: f32,
    viewport_zoom: f32,
    runtime_viewport: RuntimeViewport,
    runtime_error: Option<String>,
}

impl EditorApp {
    fn new(context: &eframe::CreationContext<'_>) -> Self {
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
            asset_search: String::new(),
            mode: EditorMode::Select,
            workspace_tab: WorkspaceTab::Scene,
            bottom_tab: BottomTab::Project,
            projection: CameraProjection::Perspective,
            playing: false,
            viewport_yaw: 0.0,
            viewport_pitch: 0.0,
            viewport_zoom: 1.0,
            runtime_viewport,
            runtime_error: None,
        }
    }

    fn top_bar(&mut self, ui: &mut egui::Ui) {
        egui::Panel::top("editor-top-bar")
            .exact_size(44.0)
            .frame(
                egui::Frame::new()
                    .fill(TOP_BG)
                    .stroke(Stroke::new(1.0, BORDER)),
            )
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.x = 11.0;
                ui.horizontal_centered(|ui| {
                    ui.add_space(14.0);
                    ui.label(
                        RichText::new("SINDRI")
                            .strong()
                            .size(15.0)
                            .color(ACCENT_BRIGHT),
                    );
                    ui.add_space(8.0);
                    for menu in ["File", "Edit", "Scene", "View", "Build", "Tools", "Help"] {
                        ui.add(
                            egui::Button::new(RichText::new(menu).size(12.0).color(TEXT_MUTED))
                                .frame(false),
                        );
                    }
                    ui.add_space((ui.available_width() * 0.22).max(16.0));
                    transport_icon(ui, ICON_SKIP_NEXT, false, "Previous frame");
                    transport_icon(ui, ICON_PLAY_ARROW, self.playing, "Play");
                    transport_icon(ui, ICON_SKIP_NEXT, false, "Next frame");
                    transport_icon(ui, ICON_PAUSE, false, "Pause");
                    if play_button(ui, self.playing).clicked() {
                        self.playing = !self.playing;
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.add_space(12.0);
                        ui.label(
                            ICON_EXPAND_MORE
                                .outlined()
                                .rich_text()
                                .size(16.0)
                                .color(TEXT_FAINT),
                        );
                        ui.label(RichText::new("isogame").size(12.0).color(TEXT_MUTED));
                    });
                });
            });
    }

    fn hierarchy_panel(&mut self, ui: &mut egui::Ui) {
        egui::Panel::left("scene-hierarchy")
            .default_size(248.0)
            .min_size(210.0)
            .max_size(340.0)
            .resizable(true)
            .frame(
                egui::Frame::new()
                    .fill(PANEL_BG)
                    .stroke(Stroke::new(1.0, BORDER)),
            )
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    tool_rail(ui, &mut self.mode);
                    ui.separator();
                    ui.vertical(|ui| {
                        ui.set_width(ui.available_width());
                        panel_title(ui, "Hierarchy", Some(ICON_ADD));
                        search_field(ui, &mut self.search, "Search");
                        ui.add_space(6.0);
                        egui::ScrollArea::vertical().show(ui, |ui| {
                            hierarchy_group(ui, "World", ICON_ACCOUNT_TREE);
                            let needle = self.search.trim().to_lowercase();
                            for (index, entity) in self.scene.entities.iter().enumerate() {
                                let name = entity_name(entity);
                                if !needle.is_empty() && !name.to_lowercase().contains(&needle) {
                                    continue;
                                }
                                if hierarchy_row(
                                    ui,
                                    entity_icon(entity),
                                    &name,
                                    index == self.selected,
                                    1,
                                )
                                .clicked()
                                {
                                    self.selected = index;
                                }
                            }
                        });
                    });
                });
            });
    }

    fn inspector_panel(&mut self, ui: &mut egui::Ui) {
        egui::Panel::right("entity-inspector")
            .default_size(340.0)
            .min_size(300.0)
            .max_size(440.0)
            .resizable(true)
            .frame(
                egui::Frame::new()
                    .fill(PANEL_BG)
                    .stroke(Stroke::new(1.0, BORDER)),
            )
            .show(ui, |ui| {
                panel_title(ui, "Inspector", None);
                if let Some(entity) = self.scene.entities.get_mut(self.selected) {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        inspector_identity(ui, entity);
                        if let Some(transform) = &mut entity.transform_3d {
                            transform_3d_section(ui, transform);
                        }
                        if let Some(transform) = &mut entity.transform_2d {
                            transform_2d_section(ui, transform);
                        }
                        components_sections(ui, entity);
                        ui.add_space(10.0);
                        ui.add_sized(
                            [ui.available_width(), 31.0],
                            egui::Button::new(
                                RichText::new("Add Component").size(12.0).color(TEXT),
                            )
                            .fill(PANEL_RAISED)
                            .stroke(Stroke::new(1.0, BORDER)),
                        );
                    });
                }
            });
    }

    fn asset_panel(&mut self, ui: &mut egui::Ui) {
        egui::Panel::bottom("asset-browser")
            .default_size(226.0)
            .min_size(140.0)
            .max_size(330.0)
            .resizable(true)
            .frame(
                egui::Frame::new()
                    .fill(PANEL_BG)
                    .stroke(Stroke::new(1.0, BORDER)),
            )
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    bottom_tab(ui, &mut self.bottom_tab, BottomTab::Project, "Project");
                    bottom_tab(ui, &mut self.bottom_tab, BottomTab::Console, "Console");
                });
                ui.separator();
                match self.bottom_tab {
                    BottomTab::Project => project_browser(ui, &mut self.asset_search),
                    BottomTab::Console => console_view(ui),
                }
            });
    }

    fn status_bar(ui: &mut egui::Ui) {
        egui::Panel::bottom("editor-status")
            .exact_size(26.0)
            .frame(
                egui::Frame::new()
                    .fill(TOP_BG)
                    .stroke(Stroke::new(1.0, BORDER)),
            )
            .show(ui, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.add_space(12.0);
                    ui.label(RichText::new("●").size(9.0).color(SUCCESS));
                    ui.label(RichText::new("Renderer ready").size(11.0).color(TEXT_MUTED));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.add_space(12.0);
                        ui.label(
                            ICON_SETTINGS
                                .outlined()
                                .rich_text()
                                .size(15.0)
                                .color(TEXT_FAINT),
                        );
                        ui.separator();
                        ui.label(
                            RichText::new("0 Errors, 0 Warnings")
                                .size(11.0)
                                .color(TEXT_FAINT),
                        );
                    });
                });
            });
    }

    fn viewport(&mut self, ui: &mut egui::Ui) {
        let context = ui.ctx().clone();
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(APP_BG).inner_margin(0))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    workspace_tab(ui, &mut self.workspace_tab, WorkspaceTab::Scene, "Scene");
                    workspace_tab(ui, &mut self.workspace_tab, WorkspaceTab::Game, "Game");
                });
                ui.separator();
                if self.workspace_tab == WorkspaceTab::Game {
                    empty_game_view(ui);
                    return;
                }
                ui.horizontal(|ui| {
                    ui.add_space(8.0);
                    mode_icon(
                        ui,
                        &mut self.mode,
                        EditorMode::Select,
                        ICON_ARROW_SELECTOR_TOOL,
                        "Select",
                    );
                    mode_icon(ui, &mut self.mode, EditorMode::Move, ICON_OPEN_WITH, "Move");
                    mode_icon(
                        ui,
                        &mut self.mode,
                        EditorMode::Rotate,
                        ICON_3D_ROTATION,
                        "Rotate",
                    );
                    mode_icon(ui, &mut self.mode, EditorMode::Scale, ICON_TUNE, "Scale");
                    ui.separator();
                    icon_button(ui, ICON_VIEW_IN_AR, false, "Local coordinates");
                    icon_button(ui, ICON_LIGHT_MODE, false, "Lit shading");
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.add_space(8.0);
                        projection_button(
                            ui,
                            &mut self.projection,
                            CameraProjection::Orthographic,
                            "Ortho",
                        );
                        projection_button(
                            ui,
                            &mut self.projection,
                            CameraProjection::Perspective,
                            "Perspective",
                        );
                    });
                });
                ui.separator();
                let (rect, response) = ui.allocate_exact_size(ui.available_size(), Sense::drag());
                if response.dragged() {
                    let delta = response.drag_motion();
                    self.viewport_yaw = (self.viewport_yaw + delta.x * 0.008) % TAU;
                    self.viewport_pitch = (self.viewport_pitch + delta.y * 0.008).clamp(-1.1, 1.1);
                }
                if response.hovered() {
                    let delta = context.input(|input| input.smooth_scroll_delta.y);
                    self.viewport_zoom = (self.viewport_zoom + delta * 0.002).clamp(0.65, 1.8);
                }
                let scale = context.pixels_per_point();
                self.runtime_error = self
                    .runtime_viewport
                    .render(
                        physical_viewport_dimension(rect.width(), scale),
                        physical_viewport_dimension(rect.height(), scale),
                        GlamVec2::new(self.viewport_yaw, self.viewport_pitch),
                        self.viewport_zoom,
                        self.projection,
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
        self.top_bar(ui);
        Self::status_bar(ui);
        self.hierarchy_panel(ui);
        self.inspector_panel(ui);
        self.asset_panel(ui);
        self.viewport(ui);
    }
}

fn configure_theme(context: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "inter".to_owned(),
        Arc::new(FontData::from_static(INTER_FONT)),
    );
    fonts
        .families
        .entry(FontFamily::Proportional)
        .or_default()
        .insert(0, "inter".to_owned());
    context.set_fonts(fonts);
    egui_material_icons::initialize(context);
    context.set_theme(egui::Theme::Dark);
    context.all_styles_mut(|style| {
        style.spacing.item_spacing = Vec2::new(7.0, 6.0);
        style.spacing.button_padding = Vec2::new(8.0, 4.0);
        style.spacing.interact_size.y = 26.0;
        style
            .text_styles
            .insert(TextStyle::Body, FontId::new(13.0, FontFamily::Proportional));
        style.text_styles.insert(
            TextStyle::Button,
            FontId::new(12.0, FontFamily::Proportional),
        );
        style.visuals.panel_fill = PANEL_BG;
        style.visuals.window_fill = PANEL_RAISED;
        style.visuals.extreme_bg_color = FIELD_BG;
        style.visuals.faint_bg_color = PANEL_RAISED;
        style.visuals.selection.bg_fill = ACCENT_SOFT;
        style.visuals.selection.stroke = Stroke::new(1.0, ACCENT);
        style.visuals.widgets.inactive.bg_fill = PANEL_RAISED;
        style.visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, BORDER);
        style.visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, TEXT_MUTED);
        style.visuals.widgets.hovered.bg_fill = Color32::from_rgb(25, 31, 37);
        style.visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, Color32::from_rgb(55, 65, 74));
        style.visuals.widgets.active.bg_fill = ACCENT_SOFT;
        style.visuals.widgets.active.bg_stroke = Stroke::new(1.0, ACCENT);
    });
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss
)]
fn physical_viewport_dimension(points: f32, scale: f32) -> u32 {
    (points * scale).round().clamp(1.0, u32::MAX as f32) as u32
}

fn panel_title(ui: &mut egui::Ui, title: &str, action: Option<MaterialIcon>) {
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.add_space(8.0);
        ui.label(RichText::new(title).strong().size(12.0).color(TEXT));
        if let Some(action) = action {
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.add_space(6.0);
                icon_button(ui, action, false, "Add entity");
            });
        }
    });
    ui.add_space(3.0);
    ui.separator();
}

fn tool_rail(ui: &mut egui::Ui, mode: &mut EditorMode) {
    ui.vertical(|ui| {
        ui.set_width(38.0);
        ui.add_space(5.0);
        for (value, icon, tip) in [
            (EditorMode::Select, ICON_ARROW_SELECTOR_TOOL, "Select"),
            (EditorMode::Move, ICON_OPEN_WITH, "Move"),
            (EditorMode::Rotate, ICON_3D_ROTATION, "Rotate"),
            (EditorMode::Scale, ICON_TUNE, "Scale"),
        ] {
            if icon_button(ui, icon, *mode == value, tip).clicked() {
                *mode = value;
            }
        }
        ui.add_space(8.0);
        ui.separator();
        icon_button(ui, ICON_DEPLOYED_CODE, false, "Scene objects");
        icon_button(ui, ICON_CODE, false, "Scripts");
    });
}

fn search_field(ui: &mut egui::Ui, value: &mut String, hint: &str) {
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.add_space(8.0);
        ui.label(
            ICON_SEARCH
                .outlined()
                .rich_text()
                .size(15.0)
                .color(TEXT_FAINT),
        );
        ui.add_sized(
            [ui.available_width() - 10.0, 28.0],
            egui::TextEdit::singleline(value)
                .hint_text(hint)
                .frame(egui::Frame::NONE),
        );
    });
}

fn hierarchy_group(ui: &mut egui::Ui, label: &str, icon: MaterialIcon) {
    ui.horizontal(|ui| {
        ui.add_space(6.0);
        ui.label(
            ICON_EXPAND_MORE
                .outlined()
                .rich_text()
                .size(15.0)
                .color(TEXT_FAINT),
        );
        ui.label(icon.outlined().rich_text().size(15.0).color(TEXT_MUTED));
        ui.label(RichText::new(label).size(12.0).color(TEXT));
    });
}

fn hierarchy_row(
    ui: &mut egui::Ui,
    icon: MaterialIcon,
    name: &str,
    selected: bool,
    depth: usize,
) -> Response {
    ui.horizontal(|ui| {
        ui.add_space(9.0 + hierarchy_indent(depth, 14.0));
        ui.label(icon.outlined().rich_text().size(15.0).color(if selected {
            ACCENT_BRIGHT
        } else {
            TEXT_MUTED
        }));
        ui.add(
            egui::Button::new(RichText::new(name).size(12.0).color(if selected {
                TEXT
            } else {
                TEXT_MUTED
            }))
            .selected(selected)
            .frame(false),
        )
    })
    .response
}

fn inspector_identity(ui: &mut egui::Ui, entity: &mut SceneEntity) {
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        ui.label(
            entity_icon(entity)
                .outlined()
                .rich_text()
                .size(19.0)
                .color(TEXT_MUTED),
        );
        let name = entity
            .name
            .get_or_insert_with(|| entity.id.as_str().to_owned());
        ui.add_sized(
            [ui.available_width() - 18.0, 29.0],
            egui::TextEdit::singleline(name).font(FontId::proportional(13.0)),
        );
    });
    ui.horizontal(|ui| {
        ui.add_space(27.0);
        ui.label(RichText::new("Tag  Untagged").size(11.0).color(TEXT_FAINT));
        ui.separator();
        ui.label(RichText::new("Layer  Default").size(11.0).color(TEXT_FAINT));
    });
}

fn section_header(ui: &mut egui::Ui, icon: MaterialIcon, title: &str) {
    ui.add_space(4.0);
    ui.separator();
    ui.horizontal(|ui| {
        ui.add_space(6.0);
        ui.label(
            ICON_EXPAND_MORE
                .outlined()
                .rich_text()
                .size(15.0)
                .color(TEXT_FAINT),
        );
        ui.label(icon.outlined().rich_text().size(16.0).color(ACCENT));
        ui.label(RichText::new(title).strong().size(12.0).color(TEXT));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.add_space(5.0);
            ui.label(
                ICON_MORE_VERT
                    .outlined()
                    .rich_text()
                    .size(15.0)
                    .color(TEXT_FAINT),
            );
        });
    });
}

fn transform_3d_section(ui: &mut egui::Ui, transform: &mut Transform3D) {
    section_header(ui, ICON_OPEN_WITH, "Transform");
    vector_row(ui, "Position", &mut transform.position);
    vector_row(ui, "Scale", &mut transform.scale);
    property_label(ui, "Rotation", "Quaternion");
}

fn transform_2d_section(ui: &mut egui::Ui, transform: &mut Transform2D) {
    section_header(ui, ICON_OPEN_WITH, "Transform 2D");
    vector_row_2d(ui, "Position", &mut transform.position);
    vector_row_2d(ui, "Scale", &mut transform.scale);
    ui.horizontal(|ui| {
        ui.add_space(10.0);
        ui.label(RichText::new("Rotation").size(11.0).color(TEXT_MUTED));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.add(egui::DragValue::new(&mut transform.rotation_radians).speed(0.01));
        });
    });
}

fn components_sections(ui: &mut egui::Ui, entity: &SceneEntity) {
    for name in entity.components.keys() {
        let icon = match name.as_str() {
            "sindri.camera" => ICON_CAMERA_ALT,
            "sindri.sprite" => ICON_IMAGE,
            "sindri.mesh" => ICON_VIEW_IN_AR,
            _ => ICON_DEPLOYED_CODE,
        };
        section_header(ui, icon, &component_label(name));
        match name.as_str() {
            "sindri.camera" => {
                property_label(ui, "Projection", "Perspective");
                property_label(ui, "Field of view", "45°");
                property_label(ui, "Clipping", "0.1 — 100");
            }
            "sindri.sprite" => {
                property_label(ui, "Texture", "procedural:badge");
                property_label(ui, "Layer", "Overlay");
            }
            "sindri.mesh" => {
                property_label(ui, "Mesh", "Cube");
                property_label(ui, "Material", "Checkerboard");
                property_label(ui, "Layer", "World");
            }
            _ => property_label(ui, "Schema", "v1"),
        }
    }
}

fn property_label(ui: &mut egui::Ui, label: &str, value: &str) {
    ui.horizontal(|ui| {
        ui.add_space(10.0);
        ui.label(RichText::new(label).size(11.0).color(TEXT_MUTED));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.add_space(7.0);
            ui.label(RichText::new(value).size(11.0).color(TEXT));
        });
    });
}

fn vector_row(ui: &mut egui::Ui, label: &str, values: &mut [f32; 3]) {
    ui.horizontal(|ui| {
        ui.add_space(10.0);
        ui.add_sized(
            [50.0, 24.0],
            egui::Label::new(RichText::new(label).size(11.0).color(TEXT_MUTED)),
        );
        for (index, value) in values.iter_mut().enumerate() {
            ui.label(
                RichText::new(["X", "Y", "Z"][index])
                    .strong()
                    .size(9.0)
                    .color(TEXT_FAINT),
            );
            ui.add_sized(
                [48.0, 23.0],
                egui::DragValue::new(value).speed(0.05).max_decimals(3),
            );
        }
    });
}

fn vector_row_2d(ui: &mut egui::Ui, label: &str, values: &mut [f32; 2]) {
    ui.horizontal(|ui| {
        ui.add_space(10.0);
        ui.add_sized(
            [50.0, 24.0],
            egui::Label::new(RichText::new(label).size(11.0).color(TEXT_MUTED)),
        );
        for (index, value) in values.iter_mut().enumerate() {
            ui.label(
                RichText::new(["X", "Y"][index])
                    .strong()
                    .size(9.0)
                    .color(TEXT_FAINT),
            );
            ui.add_sized(
                [67.0, 23.0],
                egui::DragValue::new(value).speed(0.05).max_decimals(3),
            );
        }
    });
}

fn project_browser(ui: &mut egui::Ui, search: &mut String) {
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.set_width(174.0);
            for (label, selected, depth) in [
                ("Assets", true, 0),
                ("Materials", false, 1),
                ("Models", false, 1),
                ("Scenes", false, 1),
                ("Scripts", false, 1),
                ("Textures", false, 1),
            ] {
                folder_row(ui, label, selected, depth);
            }
        });
        ui.separator();
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("Assets").size(12.0).color(TEXT));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    icon_button(ui, ICON_VIEW_LIST, false, "List view");
                    icon_button(ui, ICON_GRID_VIEW, true, "Grid view");
                    icon_button(ui, ICON_FILTER_LIST, false, "Filter assets");
                    ui.add_sized(
                        [210.0, 27.0],
                        egui::TextEdit::singleline(search).hint_text("Search Assets"),
                    );
                });
            });
            ui.add_space(8.0);
            ui.horizontal_wrapped(|ui| {
                for (icon, label) in [
                    (ICON_FOLDER, "Materials"),
                    (ICON_FOLDER, "Models"),
                    (ICON_FOLDER, "Scenes"),
                    (ICON_FOLDER, "Scripts"),
                    (ICON_DESCRIPTION, "demo.scene"),
                    (ICON_VIEW_IN_AR, "checker_cube"),
                    (ICON_IMAGE, "badge"),
                    (ICON_CODE, "scene.rs"),
                ] {
                    asset_tile(ui, icon, label);
                }
            });
        });
    });
}

fn folder_row(ui: &mut egui::Ui, label: &str, selected: bool, depth: usize) {
    ui.horizontal(|ui| {
        ui.add_space(8.0 + hierarchy_indent(depth, 12.0));
        ui.label(
            ICON_FOLDER
                .outlined()
                .rich_text()
                .size(15.0)
                .color(if selected { ACCENT } else { TEXT_FAINT }),
        );
        ui.label(
            RichText::new(label)
                .size(11.0)
                .color(if selected { TEXT } else { TEXT_MUTED }),
        );
    });
}

fn asset_tile(ui: &mut egui::Ui, icon: MaterialIcon, label: &str) {
    ui.vertical(|ui| {
        ui.add_sized(
            [72.0, 54.0],
            egui::Button::new(icon.outlined().rich_text().size(27.0).color(
                if label == "demo.scene" {
                    ACCENT
                } else {
                    TEXT_MUTED
                },
            ))
            .fill(PANEL_RAISED)
            .stroke(Stroke::new(1.0, BORDER_SUBTLE)),
        );
        ui.add_sized(
            [72.0, 17.0],
            egui::Label::new(RichText::new(label).size(10.0).color(TEXT_MUTED)).truncate(),
        );
    });
}

fn console_view(ui: &mut egui::Ui) {
    ui.add_space(8.0);
    for (color, text) in [
        (SUCCESS, "Renderer initialized with Vulkan"),
        (ACCENT, "Loaded demo.scene — 8 entities"),
    ] {
        ui.horizontal(|ui| {
            ui.add_space(10.0);
            ui.label(RichText::new("●").size(9.0).color(color));
            ui.label(RichText::new(text).size(11.0).color(TEXT_MUTED));
        });
    }
}

fn workspace_tab(ui: &mut egui::Ui, current: &mut WorkspaceTab, value: WorkspaceTab, label: &str) {
    if ui
        .add(
            egui::Button::new(RichText::new(label).size(12.0).color(if *current == value {
                TEXT
            } else {
                TEXT_FAINT
            }))
            .selected(*current == value)
            .frame(false),
        )
        .clicked()
    {
        *current = value;
    }
}

fn bottom_tab(ui: &mut egui::Ui, current: &mut BottomTab, value: BottomTab, label: &str) {
    if ui
        .add(
            egui::Button::new(RichText::new(label).size(12.0).color(if *current == value {
                TEXT
            } else {
                TEXT_FAINT
            }))
            .selected(*current == value)
            .frame(false),
        )
        .clicked()
    {
        *current = value;
    }
}

fn projection_button(
    ui: &mut egui::Ui,
    current: &mut CameraProjection,
    value: CameraProjection,
    label: &str,
) {
    let selected = *current == value;
    if ui
        .add(
            egui::Button::new(RichText::new(label).size(11.0).color(if selected {
                ACCENT_BRIGHT
            } else {
                TEXT_FAINT
            }))
            .selected(selected)
            .fill(if selected { ACCENT_SOFT } else { PANEL_RAISED })
            .stroke(Stroke::new(1.0, if selected { ACCENT } else { BORDER })),
        )
        .clicked()
    {
        *current = value;
    }
}

fn mode_icon(
    ui: &mut egui::Ui,
    mode: &mut EditorMode,
    value: EditorMode,
    icon: MaterialIcon,
    tip: &str,
) {
    if icon_button(ui, icon, *mode == value, tip).clicked() {
        *mode = value;
    }
}

fn icon_button(ui: &mut egui::Ui, icon: MaterialIcon, selected: bool, tip: &str) -> Response {
    ui.add_sized(
        [28.0, 28.0],
        egui::Button::new(icon.outlined().rich_text().size(17.0).color(if selected {
            ACCENT_BRIGHT
        } else {
            TEXT_MUTED
        }))
        .selected(selected)
        .fill(if selected { ACCENT_SOFT } else { PANEL_RAISED })
        .stroke(Stroke::new(
            1.0,
            if selected { ACCENT } else { BORDER_SUBTLE },
        )),
    )
    .on_hover_text(tip)
}

fn transport_icon(ui: &mut egui::Ui, icon: MaterialIcon, selected: bool, tip: &str) {
    ui.add_sized(
        [26.0, 26.0],
        egui::Button::new(icon.outlined().rich_text().size(16.0).color(if selected {
            ACCENT
        } else {
            TEXT_FAINT
        }))
        .frame(false),
    )
    .on_hover_text(tip);
}

fn play_button(ui: &mut egui::Ui, playing: bool) -> Response {
    let text = if playing { "Stop" } else { "Play" };
    ui.add_sized(
        [68.0, 29.0],
        egui::Button::new(
            RichText::new(text)
                .strong()
                .size(12.0)
                .color(Color32::from_rgb(28, 23, 12)),
        )
        .fill(ACCENT_BRIGHT)
        .stroke(Stroke::new(1.0, ACCENT)),
    )
}

fn empty_game_view(ui: &mut egui::Ui) {
    ui.centered_and_justified(|ui| {
        ui.vertical_centered(|ui| {
            ui.label(
                ICON_PLAY_ARROW
                    .outlined()
                    .rich_text()
                    .size(34.0)
                    .color(TEXT_FAINT),
            );
            ui.label(
                RichText::new("Game preview")
                    .strong()
                    .size(15.0)
                    .color(TEXT_MUTED),
            );
            ui.label(
                RichText::new("Enter Play mode to preview the active camera")
                    .size(12.0)
                    .color(TEXT_FAINT),
            );
        });
    });
}

fn paint_runtime_overlay(
    painter: &egui::Painter,
    rect: Rect,
    selected_name: &str,
    error: Option<&str>,
) {
    painter.rect_stroke(rect, 0.0, Stroke::new(1.0, BORDER), StrokeKind::Inside);
    let label_rect = Rect::from_min_size(rect.min + Vec2::new(12.0, 12.0), Vec2::new(218.0, 42.0));
    painter.rect_filled(label_rect, 3.0, Color32::from_black_alpha(165));
    painter.text(
        label_rect.min + Vec2::new(9.0, 7.0),
        egui::Align2::LEFT_TOP,
        selected_name,
        FontId::proportional(12.0),
        TEXT,
    );
    painter.text(
        label_rect.min + Vec2::new(9.0, 24.0),
        egui::Align2::LEFT_TOP,
        "Live WGPU viewport  ·  Drag to orbit",
        FontId::proportional(10.0),
        TEXT_FAINT,
    );
    if let Some(error) = error {
        let error_rect = Rect::from_min_size(
            Pos2::new(rect.left() + 12.0, rect.bottom() - 42.0),
            Vec2::new((rect.width() - 24.0).max(1.0), 30.0),
        );
        painter.rect_filled(error_rect, 3.0, Color32::from_rgb(72, 28, 32));
        painter.text(
            error_rect.left_center() + Vec2::new(9.0, 0.0),
            egui::Align2::LEFT_CENTER,
            error,
            FontId::proportional(10.0),
            Color32::from_rgb(255, 184, 191),
        );
    }
    paint_axis_gizmo(painter, Pos2::new(rect.right() - 42.0, rect.top() + 48.0));
}

fn paint_axis_gizmo(painter: &egui::Painter, origin: Pos2) {
    for (offset, color, label) in [
        (Vec2::new(19.0, 7.0), Color32::from_rgb(239, 92, 101), "X"),
        (Vec2::new(0.0, -22.0), Color32::from_rgb(89, 201, 135), "Y"),
        (Vec2::new(-14.0, 11.0), Color32::from_rgb(91, 151, 239), "Z"),
    ] {
        let end = origin + offset;
        painter.line_segment([origin, end], Stroke::new(2.0, color));
        painter.text(
            end,
            egui::Align2::CENTER_CENTER,
            label,
            FontId::proportional(9.0),
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

fn entity_icon(entity: &SceneEntity) -> MaterialIcon {
    if entity.components.contains_key("sindri.camera") {
        ICON_CAMERA_ALT
    } else if entity.components.contains_key("sindri.mesh") {
        ICON_VIEW_IN_AR
    } else if entity.components.contains_key("sindri.sprite") {
        ICON_IMAGE
    } else {
        ICON_DEPLOYED_CODE
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

fn hierarchy_indent(depth: usize, step: f32) -> f32 {
    f32::from(u16::try_from(depth).unwrap_or(u16::MAX)) * step
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
