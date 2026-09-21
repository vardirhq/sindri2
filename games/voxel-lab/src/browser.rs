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

use crate::{
    VoxelLabRuntime, VoxelLabStats,
    camera_control::{CameraControls, LabCamera},
};

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
    camera: CameraControls,
}

impl DesktopApp for VoxelLabApp {
    type Error = VoxelLabError;

    fn create(context: &AppContext<'_>) -> Result<Self, Self::Error> {
        let mut textures = TextureRegistry::new(context.device(), context.queue());
        let material_textures = load_causeway_atlases(context, &mut textures)?;
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
            camera: CameraControls::default(),
        })
    }

    fn input(&mut self, event: InputEvent) {
        if let InputEvent::KeyPressed(key) = event {
            self.key_pressed(key);
        }
        if self.camera.input(event) {
            self.dig_centre();
        }
    }

    fn update(&mut self, _delta: Duration) -> Result<Flow, Self::Error> {
        Ok(Flow::Continue)
    }

    #[allow(clippy::cast_precision_loss)]
    fn resize(&mut self, context: &AppContext<'_>) -> Result<(), Self::Error> {
        self.camera.set_viewport_height(context.height() as f32);
        self.depth
            .resize(context.device(), context.width(), context.height());
        Ok(())
    }

    fn render(
        &mut self,
        context: &AppContext<'_>,
        view: &wgpu::TextureView,
    ) -> Result<(), Self::Error> {
        #[allow(clippy::cast_precision_loss)]
        self.camera.set_viewport_height(context.height() as f32);
        let view = self.camera.camera();
        #[allow(clippy::cast_possible_truncation)]
        let focus = VoxelCoord::new(view.focus.x.floor() as i32, 0, view.focus.z.floor() as i32)
            .section();
        let material_textures = self.material_textures;
        let lab = self.lab.frame(focus, &move |voxel, face| {
            browser_texture(material_textures, voxel, face)
        })?;
        update_stats(lab.stats, focus);

        let camera = camera(context, view);
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
        if key == Key::Space {
            self.dig_centre();
        }
    }

    fn dig_centre(&mut self) {
        let focus = self.camera.camera().focus;
        #[allow(clippy::cast_possible_truncation)]
        let (x, z) = (focus.x.round() as i32, focus.z.round() as i32);
        self.lab.dig_surface(x, z);
    }
}

fn load_causeway_atlases(
    context: &AppContext<'_>,
    textures: &mut TextureRegistry,
) -> Result<[TextureId; 2], VoxelLabError> {
    let top = causeway_atlas(context, textures, "textures/blocks-top.png", CAUSEWAY_TOPS)?;
    let side = causeway_atlas(
        context,
        textures,
        "textures/blocks-side.png",
        CAUSEWAY_SIDES,
    )?;
    Ok([top, side])
}

fn causeway_atlas(
    context: &AppContext<'_>,
    textures: &mut TextureRegistry,
    label: &str,
    bytes: &[u8],
) -> Result<TextureId, VoxelLabError> {
    let id = label.parse::<AssetId>()?;
    let asset = TextureAssetDecoder.decode(AssetBytes::new(id, bytes.to_vec()))?;
    let mut rgba = asset.rgba8().to_vec();
    for pixel in rgba.chunks_exact_mut(4) {
        // These atlases were authored for Causeway's sprite path, where a few
        // transparent edge texels are harmless. A voxel face is an opaque 3D
        // surface: preserving those texels literally punches holes through
        // terrain and reveals buried faces.
        pixel[3] = u8::MAX;
    }
    Ok(textures.insert(Texture2D::from_rgba8(
        context.device(),
        context.queue(),
        label,
        asset.width(),
        asset.height(),
        &rgba,
    )?))
}

#[allow(clippy::cast_precision_loss)]
fn atlas_rect(column: u32, width: u32) -> Result<UvRect, UvRectError> {
    let x = 2 + column * 52;
    UvRect::new(
        x as f32 / width as f32,
        2.0 / 52.0,
        48.0 / width as f32,
        48.0 / 52.0,
    )
}

fn browser_texture(textures: [TextureId; 2], voxel: VoxelId, face: VoxelFace) -> VoxelTexture {
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
fn camera(context: &AppContext<'_>, camera: LabCamera) -> FrameCamera {
    let eye = camera.focus
        + Vec3::new(
            camera.yaw.cos() * camera.pitch.cos(),
            camera.pitch.sin(),
            camera.yaw.sin() * camera.pitch.cos(),
        ) * 52.0;
    let aspect = context.width() as f32 / context.height().max(1) as f32;
    let zoom = camera.half_height;
    FrameCamera {
        view_projection: orthographic_projection(
            -zoom * aspect,
            zoom * aspect,
            -zoom,
            zoom,
            0.1,
            160.0,
        ) * look_at(eye, camera.focus, Vec3::Y),
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
