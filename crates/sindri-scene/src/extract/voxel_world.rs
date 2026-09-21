//! Engine-owned voxel worlds as ordinary scene drawables.

use std::cell::{RefCell, RefMut};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use sindri_core::{EntityId, SpriteRef, World};
use sindri_render::{
    ExtractedFrame, FrameCamera, FrameCommand, FramePass, RenderLayer, RenderStage,
};
use sindri_voxel::{
    MeshingProfile, ResidencyConfig, SectionCoord, SectionMeshKey, VoxelCoord, VoxelFace, VoxelId,
    VoxelSource, VoxelWorld, mesh_block_section,
};

use crate::{
    TextureBindings, VoxelGeneratorDocument, VoxelMaterialDocument, VoxelRenderBridge,
    VoxelTexture, VoxelWorldComponent, compile_block_mesh,
};

use super::camera::ResolvedCameras;
use super::{SceneExtractError, SceneExtractor, transform_matrix};

const MAX_RESIDENCY_RADIUS: u32 = 8;
const MAX_HEIGHT_VARIATION: u32 = 4_096;

#[derive(Clone, Debug, Eq, PartialEq)]
struct LayeredTerrain {
    seed: u64,
    base_height: i32,
    height_variation: u32,
    surface_voxel: VoxelId,
    subsurface_voxel: VoxelId,
    deep_voxel: VoxelId,
    subsurface_depth: u32,
}

impl LayeredTerrain {
    fn from_document(generator: &VoxelGeneratorDocument) -> Self {
        let VoxelGeneratorDocument::LayeredTerrain {
            seed,
            base_height,
            height_variation,
            surface_voxel,
            subsurface_voxel,
            deep_voxel,
            subsurface_depth,
        } = generator;
        Self {
            seed: *seed,
            base_height: *base_height,
            height_variation: *height_variation,
            surface_voxel: VoxelId::new(*surface_voxel),
            subsurface_voxel: VoxelId::new(*subsurface_voxel),
            deep_voxel: VoxelId::new(*deep_voxel),
            subsurface_depth: *subsurface_depth,
        }
    }

    fn height(&self, x: i32, z: i32) -> i32 {
        if self.height_variation == 0 {
            return self.base_height;
        }
        const SCALE: i32 = 8;
        let grid_x = x.div_euclid(SCALE);
        let grid_z = z.div_euclid(SCALE);
        let offset_x = i64::from(x.rem_euclid(SCALE));
        let offset_z = i64::from(z.rem_euclid(SCALE));
        let sample = |sample_x, sample_z| {
            i64::from(
                u32::try_from(
                    terrain_hash(self.seed, sample_x, sample_z)
                        % u64::from(self.height_variation.saturating_add(1)),
                )
                .expect("terrain variation fits in u32"),
            )
        };
        let lerp = |from: i64, to: i64, offset: i64| {
            (from * (i64::from(SCALE) - offset) + to * offset) / i64::from(SCALE)
        };
        let near = lerp(
            sample(grid_x, grid_z),
            sample(grid_x.saturating_add(1), grid_z),
            offset_x,
        );
        let far = lerp(
            sample(grid_x, grid_z.saturating_add(1)),
            sample(grid_x.saturating_add(1), grid_z.saturating_add(1)),
            offset_x,
        );
        self.base_height.saturating_add(
            i32::try_from(lerp(near, far, offset_z)).expect("terrain variation fits in i32"),
        )
    }
}

fn terrain_hash(seed: u64, x: i32, z: i32) -> u64 {
    let x = u64::from(u32::from_ne_bytes(x.to_ne_bytes()));
    let z = u64::from(u32::from_ne_bytes(z.to_ne_bytes()));
    let mut value = seed ^ x.wrapping_mul(0x9e37_79b1) ^ z.wrapping_mul(0x85eb_ca77);
    value ^= value >> 30;
    value = value.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value ^= value >> 27;
    value = value.wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

impl VoxelSource for LayeredTerrain {
    fn voxel(&self, coord: VoxelCoord) -> VoxelId {
        let height = self.height(coord.x, coord.z);
        if coord.y > height {
            VoxelId::AIR
        } else if coord.y == height {
            self.surface_voxel
        } else if u32::try_from(height.saturating_sub(coord.y))
            .is_ok_and(|depth| depth <= self.subsurface_depth)
        {
            self.subsurface_voxel
        } else {
            self.deep_voxel
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
struct VoxelWorldDefinition {
    source: LayeredTerrain,
    materials: Vec<VoxelMaterialDocument>,
    render_radius: u32,
    vertical_radius: u32,
}

struct ResidentVoxelWorld {
    definition: VoxelWorldDefinition,
    texture_generation: u64,
    world: VoxelWorld<LayeredTerrain>,
    render: VoxelRenderBridge,
    resident: BTreeSet<SectionCoord>,
}

#[derive(Default)]
pub(super) struct VoxelWorldCache(RefCell<BTreeMap<EntityId, ResidentVoxelWorld>>);

impl Clone for VoxelWorldCache {
    fn clone(&self) -> Self {
        // Runtime meshes are derived state. A cloned extractor starts with an
        // empty cache rather than sharing mutable worlds with its source.
        Self::default()
    }
}

impl VoxelWorldCache {
    fn borrow_mut(&self) -> RefMut<'_, BTreeMap<EntityId, ResidentVoxelWorld>> {
        self.0.borrow_mut()
    }
}

impl fmt::Debug for VoxelWorldCache {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VoxelWorldCache")
            .field("worlds", &self.0.borrow().len())
            .finish()
    }
}

impl ResidentVoxelWorld {
    fn new(definition: VoxelWorldDefinition, texture_generation: u64) -> Self {
        let horizontal =
            i32::try_from(definition.render_radius).expect("validated voxel radius fits in i32");
        let vertical =
            i32::try_from(definition.vertical_radius).expect("validated voxel radius fits in i32");
        Self {
            world: VoxelWorld::new(
                definition.source.clone(),
                ResidencyConfig::new(horizontal, vertical, 0, 0),
            ),
            definition,
            texture_generation,
            render: VoxelRenderBridge::default(),
            resident: BTreeSet::new(),
        }
    }

    fn commands(
        &mut self,
        focus: SectionCoord,
        textures: &TextureBindings,
    ) -> Result<Vec<FrameCommand>, SceneExtractError> {
        let resolved = resolve_materials(&self.definition.materials, textures)?;
        let delta = self.world.move_focus(focus);
        for section in &delta.left {
            self.resident.remove(section);
            self.render.remove_section(*section);
        }
        self.resident.extend(delta.entered.iter().copied());

        for job in self.world.take_mesh_work() {
            if self.render.schedule(job) {
                let mesh = mesh_block_section(&self.world, job.key.section);
                let compiled = compile_block_mesh(&mesh, &|voxel: VoxelId, face: VoxelFace| {
                    resolved[&(voxel.value(), face)]
                })?;
                self.render.finish(job, compiled)?;
            }
        }

        let mut commands = self.render.take_release_commands();
        for section in &self.resident {
            commands.extend(
                self.render
                    .draw_commands(SectionMeshKey::new(*section, MeshingProfile::Block)),
            );
        }
        Ok(commands)
    }

    fn release_all(&mut self) -> Vec<FrameCommand> {
        for section in std::mem::take(&mut self.resident) {
            self.render.remove_section(section);
        }
        self.render.take_release_commands()
    }
}

impl SceneExtractor {
    pub(super) fn push_voxel_worlds(
        &self,
        world: &World,
        cameras: &ResolvedCameras,
        textures: &TextureBindings,
        frame: &mut ExtractedFrame,
    ) -> Result<(), SceneExtractError> {
        let components = self.components.query::<VoxelWorldComponent>(world)?;
        if components.is_empty() {
            return Ok(());
        }
        let camera = cameras.world.ok_or(SceneExtractError::MissingWorldCamera)?;
        let mut runtimes = self.voxel_worlds.borrow_mut();
        let active: BTreeSet<_> = components.iter().map(|(entity, _)| *entity).collect();
        let departed: Vec<_> = runtimes
            .keys()
            .filter(|entity| !active.contains(entity))
            .copied()
            .collect();
        for entity in departed {
            if let Some(mut runtime) = runtimes.remove(&entity) {
                push_commands(runtime.release_all(), 0, camera.view_projection, frame);
            }
        }

        for (entity, component) in components {
            let definition = definition(&component)?;
            let fresh = runtimes.get(&entity).is_some_and(|runtime| {
                runtime.definition == definition
                    && runtime.texture_generation == textures.generation()
            });
            if !fresh {
                if let Some(mut previous) = runtimes.remove(&entity) {
                    push_commands(
                        previous.release_all(),
                        component.layer,
                        camera.view_projection,
                        frame,
                    );
                }
                runtimes.insert(
                    entity,
                    ResidentVoxelWorld::new(definition, textures.generation()),
                );
            }
            let focus =
                SectionCoord::new(component.focus[0], component.focus[1], component.focus[2]);
            let transform = world
                .get(entity)
                .and_then(|data| data.transform_3d)
                .unwrap_or_default();
            let root = transform_matrix(transform);
            let mut commands = runtimes
                .get_mut(&entity)
                .expect("the voxel runtime was inserted above")
                .commands(focus, textures)?;
            for command in &mut commands {
                if let FrameCommand::CachedTexturedMesh { model, .. } = command {
                    *model = root * *model;
                }
            }
            push_commands(commands, component.layer, camera.view_projection, frame);
        }
        Ok(())
    }
}

fn push_commands(
    commands: Vec<FrameCommand>,
    layer: i32,
    view_projection: glam::Mat4,
    frame: &mut ExtractedFrame,
) {
    for command in commands {
        frame.push(FramePass::new(
            RenderStage::Opaque3d,
            RenderLayer(layer),
            FrameCamera { view_projection },
            command,
        ));
    }
}

fn definition(component: &VoxelWorldComponent) -> Result<VoxelWorldDefinition, SceneExtractError> {
    if component.render_radius > MAX_RESIDENCY_RADIUS
        || component.vertical_radius > MAX_RESIDENCY_RADIUS
    {
        return Err(SceneExtractError::VoxelRadiusTooLarge {
            horizontal: component.render_radius,
            vertical: component.vertical_radius,
            maximum: MAX_RESIDENCY_RADIUS,
        });
    }
    let VoxelGeneratorDocument::LayeredTerrain {
        height_variation, ..
    } = &component.generator;
    if *height_variation > MAX_HEIGHT_VARIATION {
        return Err(SceneExtractError::VoxelHeightVariationTooLarge {
            variation: *height_variation,
            maximum: MAX_HEIGHT_VARIATION,
        });
    }
    let source = LayeredTerrain::from_document(&component.generator);
    let ids: BTreeSet<_> = component
        .materials
        .iter()
        .map(|material| material.voxel)
        .collect();
    if ids.len() != component.materials.len() || ids.contains(&VoxelId::AIR.value()) {
        return Err(SceneExtractError::InvalidVoxelMaterials);
    }
    for voxel in [
        source.surface_voxel,
        source.subsurface_voxel,
        source.deep_voxel,
    ] {
        if voxel.is_air() || !ids.contains(&voxel.value()) {
            return Err(SceneExtractError::MissingVoxelMaterial(voxel.value()));
        }
    }
    Ok(VoxelWorldDefinition {
        source,
        materials: component.materials.clone(),
        render_radius: component.render_radius,
        vertical_radius: component.vertical_radius,
    })
}

fn resolve_materials(
    materials: &[VoxelMaterialDocument],
    textures: &TextureBindings,
) -> Result<BTreeMap<(u16, VoxelFace), VoxelTexture>, SceneExtractError> {
    let mut resolved = BTreeMap::new();
    for material in materials {
        for face in [
            VoxelFace::Left,
            VoxelFace::Right,
            VoxelFace::Back,
            VoxelFace::Front,
            VoxelFace::Top,
            VoxelFace::Bottom,
        ] {
            let texture = match face {
                VoxelFace::Top => &material.top,
                VoxelFace::Bottom => &material.bottom,
                VoxelFace::Left | VoxelFace::Right | VoxelFace::Back | VoxelFace::Front => {
                    &material.side
                }
            };
            let reference = SpriteRef::parse(texture)?;
            let (texture, uv) = textures.resolve_sprite(&reference);
            resolved.insert((material.voxel, face), VoxelTexture::new(texture, uv));
        }
    }
    Ok(resolved)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layered_terrain_is_solid_below_its_surface() {
        let source = LayeredTerrain::from_document(&VoxelGeneratorDocument::default());
        let height = source.height(12, -7);
        assert_eq!(
            source.voxel(VoxelCoord::new(12, height + 1, -7)),
            VoxelId::AIR
        );
        assert_eq!(
            source.voxel(VoxelCoord::new(12, height, -7)),
            VoxelId::new(1)
        );
        assert_eq!(
            source.voxel(VoxelCoord::new(12, height - 20, -7)),
            VoxelId::new(3)
        );
    }
}
