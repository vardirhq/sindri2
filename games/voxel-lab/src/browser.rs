use std::time::Duration;

use glam::Vec3;
use sindri_desktop::{AppContext, DesktopApp, Flow, WindowConfig};
use sindri_platform::{InputEvent, Key};
use sindri_render::{
    ClearOperations, DepthTarget, ExtractedFrame, FrameCamera, FrameEncodeError, FramePass,
    FrameRenderers, FrameTarget, GlyphRenderer, RenderLayer, RenderStage, ShapeRenderer,
    SpriteBatchRenderer, TextRenderer, Texture2D, TextureError, TextureId, TextureRegistry,
    TexturedCubeRenderer, UvRect, Viewport, encode_prepared_frame, look_at,
    orthographic_projection,
};
use sindri_scene::{VoxelRenderError, VoxelTexture};
use sindri_voxel::{SectionCoord, VoxelCoord, VoxelFace, VoxelId};
use thiserror::Error;

use crate::{VoxelLabRuntime, VoxelLabStats};

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
    Texture(#[from] TextureError),
    #[error(transparent)]
    Voxel(#[from] VoxelRenderError),
    #[error(transparent)]
    Frame(#[from] FrameEncodeError),
}

struct VoxelLabApp {
    lab: VoxelLabRuntime,
    textures: TextureRegistry,
    material_textures: [TextureId; 4],
    depth: DepthTarget,
    cubes: TexturedCubeRenderer,
    sprites: SpriteBatchRenderer,
    text: TextRenderer,
    glyphs: GlyphRenderer,
    shapes: ShapeRenderer,
    centre: Vec3,
    orbit: f32,
}

impl DesktopApp for VoxelLabApp {
    type Error = VoxelLabError;

    fn create(context: &AppContext<'_>) -> Result<Self, Self::Error> {
        let mut textures = TextureRegistry::new(context.device(), context.queue());
        let material_textures = [
            solid_texture(context, &mut textures, "grass top", [88, 151, 66, 255])?,
            solid_texture(context, &mut textures, "grass side", [111, 110, 61, 255])?,
            solid_texture(context, &mut textures, "dirt", [110, 72, 43, 255])?,
            solid_texture(context, &mut textures, "stone", [104, 110, 118, 255])?,
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
        })
    }

    fn input(&mut self, event: InputEvent) {
        let InputEvent::KeyPressed(key) = event else {
            return;
        };
        match key {
            Key::A | Key::ArrowLeft => self.centre.x -= 2.0,
            Key::D | Key::ArrowRight => self.centre.x += 2.0,
            Key::W | Key::ArrowUp => self.centre.z -= 2.0,
            Key::S | Key::ArrowDown => self.centre.z += 2.0,
            Key::Q => self.orbit -= 0.2,
            Key::E => self.orbit += 0.2,
            Key::Space => {
                #[allow(clippy::cast_possible_truncation)]
                let (x, z) = (self.centre.x.round() as i32, self.centre.z.round() as i32);
                self.lab.dig_surface(x, z);
            }
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

fn solid_texture(
    context: &AppContext<'_>,
    textures: &mut TextureRegistry,
    label: &str,
    rgba: [u8; 4],
) -> Result<TextureId, TextureError> {
    let mut pixels = Vec::with_capacity(8 * 8 * 4);
    for y in 0..8 {
        for x in 0..8 {
            let shade = if (x + y) % 3 == 0 { 12 } else { 0 };
            pixels.extend(rgba.map(|channel| channel.saturating_add(shade)));
        }
    }
    Ok(textures.insert(Texture2D::from_rgba8(
        context.device(),
        context.queue(),
        label,
        8,
        8,
        &pixels,
    )?))
}

fn browser_texture(textures: [TextureId; 4], voxel: VoxelId, face: VoxelFace) -> VoxelTexture {
    let index = match (voxel.value(), face) {
        (1, VoxelFace::Top) => 0,
        (1, _) => 1,
        (2, _) => 2,
        _ => 3,
    };
    VoxelTexture::new(textures[index], UvRect::FULL)
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
