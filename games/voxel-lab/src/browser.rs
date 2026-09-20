use std::time::Duration;

use glam::Vec3;
use sindri_assets::{AssetBytes, AssetDecoder, TextureAssetDecoder};
use sindri_core::AssetId;
use sindri_desktop::{AppContext, DesktopApp, Flow, WindowConfig};
use sindri_platform::{InputEvent, Key};
use sindri_render::{
    ClearOperations, DepthTarget, ExtractedFrame, FrameCamera, FrameEncodeError, FramePass,
    FramePlanError, FrameRenderers, FrameTarget, GlyphRenderer, RenderLayer, RenderStage,
    ShapeRenderer, SpriteBatchRenderer, TextRenderer, Texture2D, TextureError, TextureId,
    TextureRegistry, TexturedCubeRenderer, UvRect, UvRectError, Viewport, encode_prepared_frame,
    look_at, orthographic_projection,
};
use sindri_scene::{VoxelRenderError, VoxelTexture};
use sindri_voxel::{SectionCoord, VoxelCoord, VoxelFace, VoxelId};
use thiserror::Error;

use crate::{VoxelLabRuntime, VoxelLabStats};

const CAUSEWAY_TOPS: &[u8] = include_bytes!("../../../game/assets/textures/blocks-top.png");
const CAUSEWAY_SIDES: &[u8] = include_bytes!("../../../game/assets/textures/blocks-side.png");

pub(super) fn run() {
    console_error_panic_hook::set_once();
    let _ = console_log::init_with_level(log::Level::Info);
    if let Err(error) = sindri_desktop::run::<VoxelLabApp>(WindowConfig::new("Voxel Lab")) {
        log::error!("{error}");
    }
}

#[derive(Debug, Error)]
enum VoxelLabError {
    #[error(transparent)]
    Asset(#[from] sindri_core::AssetIdError),
    #[error(transparent)]
    Decode(#[from] sindri_assets::AssetDecodeError),
    #[error(transparent)]
    Texture(#[from] TextureError),
    #[error(transparent)]
    Uv(#[from] UvRectError),
    #[error(transparent)]
    Voxel(#[from] VoxelRenderError),
    #[error(transparent)]
    Frame(#[from] FrameEncodeError),
    #[error(transparent)]
    FramePlan(#[from] FramePlanError),
}

#[derive(Clone, Copy)]
struct TouchPan {
    id: u64,
    start: [f32; 2],
    last: [f32; 2],
    moved: bool,
}

struct VoxelLabApp {
    lab: VoxelLabRuntime,
    textures: TextureRegistry,
    material_textures: [TextureId; 2],
    depth: DepthTarget,
    cubes: TexturedCubeRenderer,
    sprites: SpriteBatchRenderer,
    text: TextRenderer,
    glyphs: GlyphRenderer,
    shapes: ShapeRenderer,
    centre: Vec3,
    orbit: f32,
    touch: Option<TouchPan>,
}

impl DesktopApp for VoxelLabApp {
    type Error = VoxelLabError;

    fn create(context: &AppContext<'_>) -> Result<Self, Self::Error> {
        let mut textures = TextureRegistry::new(context.device(), context.queue());
        let material_textures = [
            causeway_atlas(context, &mut textures, "Causeway block tops", CAUSEWAY_TOPS)?,
            causeway_atlas(context, &mut textures, "Causeway block sides", CAUSEWAY_SIDES)?,
        ];
        Ok(Self {
            lab: VoxelLabRuntime::new(),
            textures,
            material_textures,
            depth: DepthTarget::new(context.device(), context.width(), context.height()),
            cubes: TexturedCubeRenderer::new(context.device(), context.format()),
            sprites: SpriteBatchRenderer::new(context.device(), context.format()),
            text: TextRenderer::new(),
            glyphs: GlyphRenderer::new(context.device(), context.format()),
            shapes: ShapeRenderer::new(context.device(), context.format()),
            centre: Vec3::new(0.0, 3.0, 0.0),
            orbit: 0.0,
            touch: None,
        })
    }

    fn input(&mut self, event: InputEvent) {
        match event {
            InputEvent::KeyPressed(key) => self.key_pressed(key),
            InputEvent::TouchStarted { id, x, y } if self.touch.is_none() => {
                self.touch = Some(TouchPan {
                    id,
                    start: [x, y],
                    last: [x, y],
                    moved: false,
                });
            }
            InputEvent::TouchMoved { id, x, y } => self.touch_moved(id, [x, y]),
            InputEvent::TouchEnded { id } => self.touch_ended(id),
            _ => {}
        }
    }

    fn update(&mut self, _delta: Duration) -> Result<Flow, Self::Error> {
        Ok(Flow::Continue)
    }

    fn resize(&mut self, context: &AppContext<'_>) -> Result<(), Self::Error> {
        self.depth
            .resize(context.device(), context.width(), context.height());
        Ok(())
    }

    fn render(
        &mut self,
        context: &AppContext<'_>,
        view: &wgpu::TextureView,
    ) -> Result<(), Self::Error> {
        #[allow(clippy::cast_possible_truncation)]
        let focus = VoxelCoord::new(
            self.centre.x.floor() as i32,
            0,
            self.centre.z.floor() as i32,
        )
        .section();
        let material_textures = self.material_textures;
        let lab = self.lab.frame(focus, &move |voxel, face| {
            browser_texture(material_textures, voxel, face)
        })?;
        update_stats(lab.stats, focus);

        let camera = camera(context, self.centre, self.orbit);
        let mut extracted = ExtractedFrame::new(
            Viewport::new(context.width(), context.height()),
            ClearOperations {
                color: [0.035, 0.045, 0.065, 1.0],
                depth: 1.0,
            },
        );
        for command in lab.commands {
            extracted.push(FramePass::new(
                RenderStage::Opaque3d,
                RenderLayer::WORLD,
                camera,
                command,
            ));
        }
        let prepared = extracted.prepare()?;
        let mut encoder =
            context
                .device()
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("Voxel Lab browser encoder"),
                });
        encode_prepared_frame(
            FrameRenderers {
                cube: &mut self.cubes,
                sprites: &mut self.sprites,
                text: &mut self.text,
                glyphs: &mut self.glyphs,
                shapes: &mut self.shapes,
                textures: &self.textures,
            },
            context.device(),
            context.queue(),
            &mut encoder,
            FrameTarget {
                color: view,
                depth: &self.depth,
            },
            &prepared,
        )?;
        context.queue().submit([encoder.finish()]);
        Ok(())
    }
}

impl VoxelLabApp {
    fn key_pressed(&mut self, key: Key) {
        match key {
            Key::A | Key::ArrowLeft => self.centre.x -= 2.0,
            Key::D | Key::ArrowRight => self.centre.x += 2.0,
            Key::W | Key::ArrowUp => self.centre.z -= 2.0,
            Key::S | Key::ArrowDown => self.centre.z += 2.0,
            Key::Q => self.orbit -= 0.2,
            Key::E => self.orbit += 0.2,
            Key::Space => self.dig_centre(),
            _ => {}
        }
    }

    fn touch_moved(&mut self, id: u64, at: [f32; 2]) {
        let Some(mut touch) = self.touch.filter(|touch| touch.id == id) else {
            return;
        };
        let delta = [at[0] - touch.last[0], at[1] - touch.last[1]];
        let from_start = [at[0] - touch.start[0], at[1] - touch.start[1]];
        if from_start[0].hypot(from_start[1]) > 6.0 {
            touch.moved = true;
        }
        touch.last = at;
        self.touch = Some(touch);

        let yaw = std::f32::consts::FRAC_PI_4 + self.orbit;
        let right = Vec3::new(-yaw.sin(), 0.0, yaw.cos());
        let forward = Vec3::new(yaw.cos(), 0.0, yaw.sin());
        self.centre += right * (-delta[0] * 0.035) + forward * (-delta[1] * 0.035);
    }

    fn touch_ended(&mut self, id: u64) {
        let Some(touch) = self.touch.filter(|touch| touch.id == id) else {
            return;
        };
        self.touch = None;
        if !touch.moved {
            self.dig_centre();
        }
    }

    fn dig_centre(&mut self) {
        #[allow(clippy::cast_possible_truncation)]
        let (x, z) = (self.centre.x.round() as i32, self.centre.z.round() as i32);
        self.lab.dig_surface(x, z);
    }
}

fn causeway_atlas(
    context: &AppContext<'_>,
    textures: &mut TextureRegistry,
    label: &str,
    bytes: &[u8],
) -> Result<TextureId, VoxelLabError> {
    let id = label.parse::<AssetId>()?;
    let asset = TextureAssetDecoder.decode(AssetBytes::new(id, bytes.to_vec()))?;
    Ok(textures.insert(Texture2D::from_rgba8(
        context.device(),
        context.queue(),
        label,
        asset.width(),
        asset.height(),
        asset.rgba8(),
    )?))
}

fn atlas_rect(column: u32, width: u32) -> Result<UvRect, UvRectError> {
    let x = 2 + column * 52;
    UvRect::new(
        x as f32 / width as f32,
        2.0 / 52.0,
        48.0 / width as f32,
        48.0 / 52.0,
    )
}

fn browser_texture(
    textures: [TextureId; 2],
    voxel: VoxelId,
    face: VoxelFace,
) -> VoxelTexture {
    let top = face == VoxelFace::Top;
    let column = match voxel.value() {
        1 => 0,
        2 => 4,
        _ => 6,
    };
    let (texture, width) = if top {
        (textures[0], 1196)
    } else {
        (textures[1], 1144)
    };
    let uv = atlas_rect(column, width).expect("Causeway atlas columns are valid");
    VoxelTexture::new(texture, uv)
}

#[allow(clippy::cast_precision_loss)]
fn camera(context: &AppContext<'_>, centre: Vec3, orbit: f32) -> FrameCamera {
    let yaw = std::f32::consts::FRAC_PI_4 + orbit;
    let pitch = std::f32::consts::FRAC_PI_6;
    let eye = centre
        + Vec3::new(
            yaw.cos() * pitch.cos(),
            pitch.sin(),
            yaw.sin() * pitch.cos(),
        ) * 52.0;
    let aspect = context.width() as f32 / context.height().max(1) as f32;
    let zoom = 20.0;
    FrameCamera {
        view_projection: orthographic_projection(
            -zoom * aspect,
            zoom * aspect,
            -zoom,
            zoom,
            0.1,
            160.0,
        ) * look_at(eye, centre, Vec3::Y),
    }
}

fn update_stats(stats: VoxelLabStats, focus: SectionCoord) {
    let Some(document) = web_sys::window().and_then(|window| window.document()) else {
        return;
    };
    let Some(element) = document.get_element_by_id("voxel-stats") else {
        return;
    };
    element.set_text_content(Some(&format!(
        "section [{}, {}, {}] · resident {} · meshes {} · remeshes {} · triangles {} · uploads {} · releases {}",
        focus.x,
        focus.y,
        focus.z,
        stats.resident_sections,
        stats.mesh_jobs,
        stats.remeshes,
        stats.triangles,
        stats.uploads,
        stats.releases
    )));
}
