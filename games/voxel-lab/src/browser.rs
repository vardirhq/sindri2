use std::time::Duration;

use glam::Vec3;
use sindri_assets::{AssetBytes, AssetDecoder, TextureAssetDecoder};
use sindri_core::{AssetId, SceneDocument, World};
use sindri_desktop::{AppContext, DesktopApp, Flow, WindowConfig};
use sindri_platform::{InputEvent, Key};
use sindri_render::{
    Bloom, ClearOperations, DepthTarget, ExtractedFrame, FrameCamera, FrameEncodeError, FramePass,
    FramePlanError, FrameRenderers, FrameTarget, GlyphRenderer, Lighting, RenderLayer, RenderStage,
    ShapeRenderer, SpriteBatchRenderer, TextRenderer, Texture2D, TextureError, TextureId,
    TextureRegistry, TexturedCubeRenderer, UvRect, UvRectError, Viewport, encode_lit_frame,
    encode_prepared_frame, look_at, orthographic_projection,
};
use sindri_scene::{EnvironmentComponent, VoxelRenderError, VoxelTexture, environment_of};
use sindri_voxel::{SectionCoord, VoxelCoord, VoxelFace, VoxelId};
use thiserror::Error;

use crate::{
    VoxelLabRuntime, VoxelLabStats,
    camera_control::{CameraControls, LabCamera},
};

const SCENE_JSON: &str = include_str!("../assets/voxel-lab.scene.json");
const USER_TOP: &[u8] = include_bytes!("../assets/textures/user-top.png");
const USER_SIDE_A: &[u8] = include_bytes!("../assets/textures/user-side-a.png");
const USER_DIRT: &[u8] = include_bytes!("../assets/textures/user-dirt.png");
const WORLD_TOPS: &[u8] = include_bytes!("../assets/textures/world-blocks-top.png");
const WORLD_SIDES: &[u8] = include_bytes!("../assets/textures/world-blocks-side.png");

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
    material_textures: BrowserMaterials,
    depth: DepthTarget,
    cubes: TexturedCubeRenderer,
    sprites: SpriteBatchRenderer,
    text: TextRenderer,
    glyphs: GlyphRenderer,
    shapes: ShapeRenderer,
    bloom: Bloom,
    environment: EnvironmentComponent,
    camera: CameraControls,
}

impl DesktopApp for VoxelLabApp {
    type Error = VoxelLabError;

    fn create(context: &AppContext<'_>) -> Result<Self, Self::Error> {
        let mut textures = TextureRegistry::new(context.device(), context.queue());
        let material_textures = load_authored_materials(context, &mut textures)?;
        let document =
            SceneDocument::from_json(SCENE_JSON).expect("embedded Voxel Lab scene parses");
        let world = World::from_scene(&document)
            .expect("embedded Voxel Lab scene loads")
            .world;
        let environment = environment_of(&world)
            .expect("Voxel Lab environment is valid")
            .expect("Voxel Lab authors an environment");
        let mut bloom = Bloom::new(context.device(), context.format());
        bloom.resize(context.device(), context.width(), context.height());
        let mut camera = CameraControls::default();
        camera.set_viewport_height(context.height());
        Ok(Self {
            lab: VoxelLabRuntime::new(),
            textures,
            material_textures,
            depth: DepthTarget::new(context.device(), context.width(), context.height()),
            cubes: TexturedCubeRenderer::new(context.device(), Bloom::SCENE_FORMAT),
            sprites: SpriteBatchRenderer::new(context.device(), Bloom::SCENE_FORMAT),
            text: TextRenderer::new(),
            glyphs: GlyphRenderer::new(context.device(), Bloom::SCENE_FORMAT),
            shapes: ShapeRenderer::new(context.device(), Bloom::SCENE_FORMAT),
            bloom,
            environment,
            camera,
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

    fn resize(&mut self, context: &AppContext<'_>) -> Result<(), Self::Error> {
        self.camera.set_viewport_height(context.height());
        self.depth
            .resize(context.device(), context.width(), context.height());
        self.bloom
            .resize(context.device(), context.width(), context.height());
        Ok(())
    }

    fn render(
        &mut self,
        context: &AppContext<'_>,
        view: &wgpu::TextureView,
    ) -> Result<(), Self::Error> {
        self.camera.set_viewport_height(context.height());
        let camera_view = self.camera.camera();
        #[allow(clippy::cast_possible_truncation)]
        let focus = VoxelCoord::new(
            camera_view.focus.x.floor() as i32,
            0,
            camera_view.focus.z.floor() as i32,
        )
        .section();
        let material_textures = self.material_textures;
        let lab = self.lab.frame(focus, &move |voxel, face| {
            browser_texture(material_textures, voxel, face)
        })?;
        update_stats(lab.stats, focus);

        let camera = camera(context, camera_view);
        let mut extracted = ExtractedFrame::new(
            Viewport::new(context.width(), context.height()),
            ClearOperations {
                color: self.environment.background.map(f64::from),
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
        self.cubes.set_lighting(self.environment.world_lighting());
        self.cubes.set_fog(self.environment.fog_settings());
        self.cubes
            .set_shadows(context.device(), self.environment.shadow_settings());
        self.cubes
            .set_ambient_occlusion(if self.environment.ambient_occlusion.enabled {
                self.environment.ambient_occlusion.strength
            } else {
                0.0
            });
        let renderers = FrameRenderers {
            cube: &mut self.cubes,
            sprites: &mut self.sprites,
            text: &mut self.text,
            glyphs: &mut self.glyphs,
            shapes: &mut self.shapes,
            textures: &self.textures,
        };
        let target = FrameTarget {
            color: view,
            depth: &self.depth,
        };
        let post_process = self.environment.post_process_settings();
        if post_process.is_active() {
            encode_lit_frame(
                renderers,
                context.device(),
                context.queue(),
                &mut encoder,
                target,
                &prepared,
                Lighting {
                    bloom: &mut self.bloom,
                    settings: post_process,
                },
            )?;
        } else {
            encode_prepared_frame(
                renderers,
                context.device(),
                context.queue(),
                &mut encoder,
                target,
                &prepared,
            )?;
        }
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

#[derive(Clone, Copy)]
struct BrowserMaterials {
    grass_top: TextureId,
    grass_side: TextureId,
    dirt: TextureId,
    world_top: TextureId,
    world_side: TextureId,
}

fn load_authored_materials(
    context: &AppContext<'_>,
    textures: &mut TextureRegistry,
) -> Result<BrowserMaterials, VoxelLabError> {
    Ok(BrowserMaterials {
        grass_top: load_texture(context, textures, "textures/user-top.png", USER_TOP)?,
        grass_side: load_texture(context, textures, "textures/user-side-a.png", USER_SIDE_A)?,
        dirt: load_texture(context, textures, "textures/user-dirt.png", USER_DIRT)?,
        world_top: load_texture(
            context,
            textures,
            "textures/world-blocks-top.png",
            WORLD_TOPS,
        )?,
        world_side: load_texture(
            context,
            textures,
            "textures/world-blocks-side.png",
            WORLD_SIDES,
        )?,
    })
}

fn load_texture(
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

fn browser_texture(textures: BrowserMaterials, voxel: VoxelId, face: VoxelFace) -> VoxelTexture {
    match (voxel.value(), face) {
        (0, _) => unreachable!("air never produces block faces"),
        (1, VoxelFace::Top) => VoxelTexture::new(textures.grass_top, UvRect::FULL),
        (1, _) => VoxelTexture::new(textures.grass_side, UvRect::FULL),
        (2, _) => VoxelTexture::new(textures.dirt, UvRect::FULL),
        (_, VoxelFace::Top | VoxelFace::Bottom) => {
            let uv = atlas_rect(6, 1196).expect("world top atlas stone column is valid");
            VoxelTexture::new(textures.world_top, uv)
        }
        (_, _) => {
            let uv = atlas_rect(6, 1144).expect("world side atlas stone column is valid");
            VoxelTexture::new(textures.world_side, uv)
        }
    }
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
        position: eye,
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
