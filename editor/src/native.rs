use std::f32::consts::TAU;

use eframe::egui::{
    self, Align, Color32, FontId, Layout, Pos2, Rect, Response, RichText, Sense, Shape, Stroke,
    StrokeKind, Vec2,
};
use sindri_core::{SceneDocument, SceneEntity, Transform2D, Transform3D};

const SCENE_JSON: &str = include_str!("../../examples/cube/assets/demo.scene.json");
const ACCENT: Color32 = Color32::from_rgb(238, 174, 62);
const ACCENT_SOFT: Color32 = Color32::from_rgb(74, 57, 31);
const APP_BG: Color32 = Color32::from_rgb(12, 15, 20);
const PANEL_BG: Color32 = Color32::from_rgb(18, 22, 28);
const PANEL_RAISED: Color32 = Color32::from_rgb(24, 29, 36);
const BORDER: Color32 = Color32::from_rgb(47, 53, 63);
const TEXT: Color32 = Color32::from_rgb(229, 226, 217);
const TEXT_MUTED: Color32 = Color32::from_rgb(137, 141, 149);
const STATUS_GOOD: Color32 = Color32::from_rgb(139, 177, 112);

pub fn run() -> eframe::Result {
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
enum ViewportTab {
    Scene,
    Game,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BottomTab {
    Project,
    Console,
}

struct EditorApp {
    scene: SceneDocument,
    selected: usize,
    search: String,
    mode: EditorMode,
    viewport_tab: ViewportTab,
    bottom_tab: BottomTab,
    playing: bool,
    viewport_yaw: f32,
    viewport_pitch: f32,
    viewport_zoom: f32,
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

        Self {
            scene,
            selected,
            search: String::new(),
            mode: EditorMode::Select,
            viewport_tab: ViewportTab::Scene,
            bottom_tab: BottomTab::Project,
            playing: false,
            viewport_yaw: -0.62,
            viewport_pitch: 0.42,
            viewport_zoom: 1.0,
        }
    }

    fn menu_bar(ui: &mut egui::Ui) {
        egui::Panel::top("editor-top-bar")
            .exact_size(30.0)
            .frame(
                egui::Frame::new()
                    .fill(PANEL_BG)
                    .stroke(Stroke::new(1.0, BORDER)),
            )
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    brand_mark(ui, 20.0);
                    ui.label(RichText::new("SINDRI").strong().size(14.0).color(TEXT));
                    ui.add_space(14.0);
                    menu_label(ui, "File");
                    menu_label(ui, "Edit");
                    menu_label(ui, "Assets");
                    menu_label(ui, "Entity");
                    menu_label(ui, "Window");
                    menu_label(ui, "Help");
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.label(RichText::new("Default").small().color(TEXT_MUTED));
                        ui.label(RichText::new("Layout").small().color(TEXT_MUTED));
                    });
                });
            });
    }

    fn tool_bar(&mut self, ui: &mut egui::Ui) {
        egui::Panel::top("editor-tool-bar")
            .exact_size(42.0)
            .frame(
                egui::Frame::new()
                    .fill(PANEL_RAISED)
                    .stroke(Stroke::new(1.0, BORDER)),
            )
            .show(ui, |ui| {
                ui.add_space(5.0);
                ui.columns(3, |columns| {
                    columns[0].horizontal(|ui| {
                        mode_button(ui, &mut self.mode, EditorMode::Select, "Select");
                        mode_button(ui, &mut self.mode, EditorMode::Move, "Move");
                        mode_button(ui, &mut self.mode, EditorMode::Rotate, "Rotate");
                        mode_button(ui, &mut self.mode, EditorMode::Scale, "Scale");
                    });
                    columns[1].with_layout(Layout::left_to_right(Align::Center), |ui| {
                        ui.add_space((ui.available_width() - 126.0).max(0.0) * 0.5);
                        if toolbar_button(ui, if self.playing { "■" } else { "▶" }, self.playing)
                            .on_hover_text(if self.playing { "Stop" } else { "Play" })
                            .clicked()
                        {
                            self.playing = !self.playing;
                        }
                        toolbar_button(ui, "Ⅱ", false).on_hover_text("Pause");
                        toolbar_button(ui, "▣", false).on_hover_text("Step");
                    });
                    columns[2].with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.label(RichText::new("Saved").small().color(STATUS_GOOD));
                        ui.separator();
                        compact_button(ui, "Redo");
                        compact_button(ui, "Undo");
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
                panel_heading(
                    ui,
                    "HIERARCHY",
                    &format!("{} entities", self.scene.entities.len()),
                );
                ui.add_space(8.0);
                ui.add_sized(
                    [ui.available_width(), 30.0],
                    egui::TextEdit::singleline(&mut self.search).hint_text("Filter entities..."),
                );
                ui.add_space(10.0);

                let needle = self.search.trim().to_lowercase();
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("demo.scene").strong().small().color(TEXT));
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.label(RichText::new("+").color(TEXT_MUTED));
                        });
                    });
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
                    ui.label(RichText::new("Ready").small().color(STATUS_GOOD));
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

    fn project_panel(&mut self, ui: &mut egui::Ui) {
        egui::Panel::bottom("project-console")
            .default_size(210.0)
            .min_size(140.0)
            .max_size(360.0)
            .frame(
                egui::Frame::new()
                    .fill(PANEL_BG)
                    .stroke(Stroke::new(1.0, BORDER)),
            )
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    panel_tab(ui, &mut self.bottom_tab, BottomTab::Project, "Project");
                    panel_tab(ui, &mut self.bottom_tab, BottomTab::Console, "Console");
                    if self.bottom_tab == BottomTab::Console {
                        ui.label(RichText::new("3").small().color(TEXT_MUTED));
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.label(RichText::new("Search").small().color(TEXT_MUTED));
                    });
                });
                ui.separator();
                match self.bottom_tab {
                    BottomTab::Project => project_browser(ui),
                    BottomTab::Console => console_view(ui),
                }
            });
    }

    fn viewport(&mut self, ui: &mut egui::Ui) {
        let context = ui.ctx().clone();
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(APP_BG))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    viewport_tab(ui, &mut self.viewport_tab, ViewportTab::Scene, "Scene");
                    viewport_tab(ui, &mut self.viewport_tab, ViewportTab::Game, "Game");
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.label(RichText::new("Gizmos").small().color(TEXT_MUTED));
                        ui.label(RichText::new("Lit").small().color(TEXT_MUTED));
                        ui.label(RichText::new("Perspective").small().color(TEXT_MUTED));
                    });
                });
                ui.separator();

                let (rect, response) = ui.allocate_exact_size(ui.available_size(), Sense::drag());
                if self.viewport_tab == ViewportTab::Scene && response.dragged() {
                    let delta = response.drag_motion();
                    self.viewport_yaw = (self.viewport_yaw + delta.x * 0.008) % TAU;
                    self.viewport_pitch = (self.viewport_pitch + delta.y * 0.008).clamp(-1.1, 1.1);
                }
                if self.viewport_tab == ViewportTab::Scene && response.hovered() {
                    let zoom_delta = context.input(|input| input.smooth_scroll_delta.y);
                    self.viewport_zoom = (self.viewport_zoom + zoom_delta * 0.002).clamp(0.65, 1.8);
                }
                match self.viewport_tab {
                    ViewportTab::Scene => paint_viewport(
                        ui.painter(),
                        rect,
                        self.viewport_yaw,
                        self.viewport_pitch,
                        self.viewport_zoom,
                        &entity_name(&self.scene.entities[self.selected]),
                    ),
                    ViewportTab::Game => paint_game_placeholder(ui.painter(), rect),
                }
            });
    }
}

impl eframe::App for EditorApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        Self::menu_bar(ui);
        self.tool_bar(ui);
        Self::status_bar(ui);
        self.inspector_panel(ui);
        self.project_panel(ui);
        self.hierarchy_panel(ui);
        self.viewport(ui);
    }
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

fn brand_mark(ui: &mut egui::Ui, size: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
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

fn menu_label(ui: &mut egui::Ui, label: &str) -> Response {
    ui.add(egui::Button::new(RichText::new(label).small().color(TEXT)).frame(false))
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
        [52.0, 28.0],
        egui::Button::new(RichText::new(label).small()),
    )
}

fn toolbar_button(ui: &mut egui::Ui, label: &str, active: bool) -> Response {
    ui.add_sized(
        [38.0, 28.0],
        egui::Button::new(RichText::new(label).strong().color(if active {
            Color32::from_rgb(24, 20, 13)
        } else {
            TEXT
        }))
        .fill(if active { ACCENT } else { PANEL_BG }),
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

fn viewport_tab(ui: &mut egui::Ui, tab: &mut ViewportTab, value: ViewportTab, label: &str) {
    let selected = *tab == value;
    let response = ui.add(
        egui::Button::new(RichText::new(label).strong().small().color(if selected {
            TEXT
        } else {
            TEXT_MUTED
        }))
        .selected(selected)
        .frame(false),
    );
    if response.clicked() {
        *tab = value;
    }
}

fn panel_tab(ui: &mut egui::Ui, tab: &mut BottomTab, value: BottomTab, label: &str) {
    let selected = *tab == value;
    let response = ui.add(
        egui::Button::new(RichText::new(label).strong().small().color(if selected {
            TEXT
        } else {
            TEXT_MUTED
        }))
        .selected(selected)
        .frame(false),
    );
    if response.clicked() {
        *tab = value;
    }
}

fn project_browser(ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        ui.allocate_ui_with_layout(
            Vec2::new(178.0, ui.available_height()),
            Layout::top_down(Align::Min),
            |ui| {
                ui.label(RichText::new("Favorites").small().color(TEXT_MUTED));
                ui.label(RichText::new("  All assets").small().color(TEXT));
                ui.add_space(6.0);
                ui.label(RichText::new("Assets").small().color(TEXT_MUTED));
                ui.label(RichText::new("  assets/").small().color(TEXT));
                ui.label(RichText::new("    scenes/").small().color(TEXT));
                ui.label(RichText::new("    textures/").small().color(TEXT));
                ui.label(RichText::new("    scripts/").small().color(TEXT));
            },
        );
        ui.separator();
        ui.vertical(|ui| {
            ui.label(RichText::new("Assets").small().color(TEXT_MUTED));
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                asset_tile(ui, "SCN", "demo.scene.json");
            });
        });
    });
}

fn asset_tile(ui: &mut egui::Ui, kind: &str, label: &str) {
    ui.allocate_ui(Vec2::new(116.0, 78.0), |ui| {
        egui::Frame::new()
            .fill(PANEL_RAISED)
            .stroke(Stroke::new(1.0, BORDER))
            .corner_radius(2)
            .inner_margin(8)
            .show(ui, |ui| {
                ui.set_min_size(Vec2::new(98.0, 60.0));
                ui.vertical_centered(|ui| {
                    ui.add_space(5.0);
                    ui.label(RichText::new(kind).strong().size(11.0).color(ACCENT));
                    ui.add_space(6.0);
                    ui.label(RichText::new(label).small().color(TEXT));
                });
            });
    });
}

fn console_view(ui: &mut egui::Ui) {
    egui::ScrollArea::vertical().show(ui, |ui| {
        console_row(ui, STATUS_GOOD, "INFO", "Editor scene loaded and validated");
        console_row(ui, STATUS_GOOD, "INFO", "wgpu adapter ready");
        console_row(
            ui,
            ACCENT,
            "TODO",
            "Runtime play mode is not connected to the editor shell yet",
        );
    });
}

fn console_row(ui: &mut egui::Ui, color: Color32, level: &str, message: &str) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(level).strong().small().color(color));
        ui.label(RichText::new(message).small().color(TEXT));
    });
    ui.separator();
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
        .corner_radius(2)
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
                    ui.label(RichText::new("|").strong().small().color(ACCENT));
                    ui.label(RichText::new(component_label(name)).color(TEXT));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.label(RichText::new("v1").small().color(TEXT_MUTED));
                    });
                });
            }
        }
    });
}

fn paint_game_placeholder(painter: &egui::Painter, rect: Rect) {
    painter.rect_filled(rect, 0.0, Color32::from_rgb(8, 11, 15));
    let card = Rect::from_center_size(rect.center(), Vec2::new(360.0, 96.0));
    painter.rect_filled(card, 3.0, PANEL_RAISED);
    painter.rect_stroke(card, 3.0, Stroke::new(1.0, BORDER), StrokeKind::Inside);
    painter.text(
        card.center_top() + Vec2::new(0.0, 24.0),
        egui::Align2::CENTER_CENTER,
        "Game preview",
        FontId::proportional(15.0),
        TEXT,
    );
    painter.text(
        card.center_bottom() - Vec2::new(0.0, 27.0),
        egui::Align2::CENTER_CENTER,
        "Available when editor play-mode integration lands",
        FontId::proportional(11.0),
        TEXT_MUTED,
    );
}

fn paint_viewport(
    painter: &egui::Painter,
    rect: Rect,
    yaw: f32,
    pitch: f32,
    zoom: f32,
    selected_name: &str,
) {
    painter.rect_filled(rect, 0.0, Color32::from_rgb(9, 13, 20));
    paint_grid(painter, rect);

    let center = Pos2::new(rect.center().x, rect.center().y - rect.height() * 0.03);
    let size = rect.width().min(rect.height()) * 0.18 * zoom;
    paint_cube(painter, center, size, yaw, pitch);

    let label_rect = Rect::from_min_size(rect.min + Vec2::new(14.0, 14.0), Vec2::new(210.0, 48.0));
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
        "Drag to orbit  ·  Scroll to zoom",
        FontId::proportional(10.0),
        TEXT_MUTED,
    );
    paint_axis_gizmo(painter, Pos2::new(rect.left() + 38.0, rect.bottom() - 38.0));
}

fn paint_grid(painter: &egui::Painter, rect: Rect) {
    let horizon = rect.top() + rect.height() * 0.48;
    let vanishing = Pos2::new(rect.center().x, horizon);
    let grid = Color32::from_rgb(27, 35, 47);
    let major = Color32::from_rgb(39, 48, 62);
    for index in -12_i16..=12 {
        let x = rect.center().x + f32::from(index) * rect.width() / 18.0;
        painter.line_segment(
            [vanishing, Pos2::new(x, rect.bottom())],
            Stroke::new(
                if index % 4 == 0 { 1.0 } else { 0.6 },
                if index % 4 == 0 { major } else { grid },
            ),
        );
    }
    for index in 0_u8..14 {
        let t = f32::from(index) / 13.0;
        let y = horizon + t.powf(1.7) * (rect.bottom() - horizon);
        painter.line_segment(
            [Pos2::new(rect.left(), y), Pos2::new(rect.right(), y)],
            Stroke::new(
                if index % 4 == 0 { 1.0 } else { 0.6 },
                if index % 4 == 0 { major } else { grid },
            ),
        );
    }
    painter.line_segment(
        [
            Pos2::new(rect.left(), horizon),
            Pos2::new(rect.right(), horizon),
        ],
        Stroke::new(1.0, Color32::from_rgb(33, 42, 56)),
    );
}

fn paint_cube(painter: &egui::Painter, center: Pos2, size: f32, yaw: f32, pitch: f32) {
    let vertices = [
        [-1.0, -1.0, -1.0],
        [1.0, -1.0, -1.0],
        [1.0, 1.0, -1.0],
        [-1.0, 1.0, -1.0],
        [-1.0, -1.0, 1.0],
        [1.0, -1.0, 1.0],
        [1.0, 1.0, 1.0],
        [-1.0, 1.0, 1.0],
    ];
    let faces = [
        ([0, 1, 2, 3], Color32::from_rgb(83, 95, 116)),
        ([4, 7, 6, 5], ACCENT),
        ([0, 4, 5, 1], Color32::from_rgb(181, 76, 55)),
        ([3, 2, 6, 7], Color32::from_rgb(255, 151, 91)),
        ([1, 5, 6, 2], Color32::from_rgb(136, 69, 60)),
        ([0, 3, 7, 4], Color32::from_rgb(106, 119, 145)),
    ];
    let (sin_yaw, cos_yaw) = yaw.sin_cos();
    let (sin_pitch, cos_pitch) = pitch.sin_cos();
    let mut projected = [Pos2::ZERO; 8];
    let mut depth = [0.0; 8];
    for (index, [x, y, z]) in vertices.into_iter().enumerate() {
        let rotated_x = x * cos_yaw + z * sin_yaw;
        let yaw_z = -x * sin_yaw + z * cos_yaw;
        let rotated_y = y * cos_pitch - yaw_z * sin_pitch;
        let rotated_z = y * sin_pitch + yaw_z * cos_pitch;
        let perspective = 1.0 / (1.0 + (rotated_z + 1.0) * 0.08);
        projected[index] = Pos2::new(
            center.x + rotated_x * size * perspective,
            center.y - rotated_y * size * perspective,
        );
        depth[index] = rotated_z;
    }
    let mut ordered_faces = faces.map(|(indices, color)| {
        let average = indices.into_iter().map(|index| depth[index]).sum::<f32>() / 4.0;
        (average, indices, color)
    });
    ordered_faces.sort_by(|left, right| left.0.total_cmp(&right.0));
    for (_, indices, color) in ordered_faces {
        let points = indices.into_iter().map(|index| projected[index]).collect();
        painter.add(Shape::convex_polygon(
            points,
            color,
            Stroke::new(1.4, Color32::from_rgb(255, 184, 140)),
        ));
    }
    let selection = Rect::from_center_size(center, Vec2::splat(size * 2.75));
    painter.rect_stroke(
        selection,
        2.0,
        Stroke::new(1.0, ACCENT),
        StrokeKind::Outside,
    );
    for corner in [
        selection.left_top(),
        selection.right_top(),
        selection.left_bottom(),
        selection.right_bottom(),
    ] {
        painter.rect_filled(
            Rect::from_center_size(corner, Vec2::splat(5.0)),
            1.0,
            ACCENT,
        );
    }
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
