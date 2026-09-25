//! Engine-owned voxel worlds as ordinary scene drawables.

use std::cell::{RefCell, RefMut};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use sindri_core::{EntityId, SceneComponent, World};
use sindri_render::{
    ExtractedFrame, FrameCamera, FrameCommand, FramePass, RenderLayer, RenderStage,
};
use sindri_voxel::{
    MeshingProfile, ResidencyConfig, SectionCoord, SectionMeshKey, VoxelFace, VoxelId, VoxelWorld,
    mesh_block_section_with_materials,
};

use crate::{
    TextureBindings, TileSetBindings, VoxelRenderBridge, VoxelWorldComponent, compile_block_mesh,
};

use super::camera::{ResolvedCamera, ResolvedCameras};
use super::frustum::aabb_in_view;
use super::voxel_appearance::{
    Appearance, BlockMaterials, Resolved, block_palette, resolve_appearance,
};
use super::voxel_source::{Palette, SceneTerrain, terrain_source};
use super::{SceneExtractError, SceneExtractor, transform_matrix};

pub(super) const MAX_RESIDENCY_RADIUS: u32 = 8;

#[derive(Clone, Debug, PartialEq)]
struct VoxelWorldDefinition {
    source: SceneTerrain,
    appearance: Appearance,
    render_radius: u32,
    vertical_radius: u32,
}

struct ResidentVoxelWorld {
    definition: VoxelWorldDefinition,
    /// What the materials resolved to when this world was meshed.
    ///
    /// Compared instead of the bindings' generation, which moves when any
    /// texture anywhere in the project is bound: a sprite loading or
    /// hot-reloading elsewhere rebuilt the whole voxel world. Only a change to
    /// what these faces actually draw with needs their meshes compiled again.
    resolved: Resolved,
    /// Which faces each voxel hides, derived from its blocks.
    materials: BlockMaterials,
    world: VoxelWorld<SceneTerrain>,
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
    fn new(definition: VoxelWorldDefinition, resolved: Resolved) -> Self {
        let horizontal =
            i32::try_from(definition.render_radius).expect("validated voxel radius fits in i32");
        let vertical =
            i32::try_from(definition.vertical_radius).expect("validated voxel radius fits in i32");
        Self {
            world: VoxelWorld::new(
                definition.source.clone(),
                ResidencyConfig::new(horizontal, vertical, 0, 0),
            ),
            materials: BlockMaterials::of(&definition.appearance),
            definition,
            resolved,
            render: VoxelRenderBridge::default(),
            resident: BTreeSet::new(),
        }
    }

    fn commands(
        &mut self,
        focus: SectionCoord,
        local_view_projection: glam::Mat4,
        seconds: f32,
    ) -> Result<Vec<FrameCommand>, SceneExtractError> {
        let resolved = &self.resolved;
        let delta = self.world.move_focus(focus);
        for section in &delta.left {
            self.resident.remove(section);
            self.render.remove_section(*section);
        }
        self.resident.extend(delta.entered.iter().copied());

        for job in self.world.take_mesh_work() {
            if self.render.schedule(job) {
                let mesh = mesh_block_section_with_materials(
                    &self.world,
                    &self.materials,
                    job.key.section,
                );
                let compiled = compile_block_mesh(&mesh, &|voxel: VoxelId, face: VoxelFace| {
                    resolved.faces[&(voxel.value(), face)]
                })?;
                self.render.finish(job, compiled)?;
            }
        }

        let mut commands = self.render.take_release_commands();
        for section in &self.resident {
            let key = SectionMeshKey::new(*section, MeshingProfile::Block);
            if self
                .render
                .bounds(key)
                .is_some_and(|bounds| section_in_view(bounds, local_view_projection))
            {
                commands.extend(
                    self.render
                        .draw_commands(key, &|look| resolved.surface(look, seconds)),
                );
            }
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

/// Where one voxel world's commands go, and what they are drawn against.
struct VoxelTarget<'a> {
    camera: ResolvedCamera,
    textures: &'a TextureBindings,
    tile_sets: Option<&'a TileSetBindings>,
    /// How long the scene has run, which is where animated faces have got to.
    seconds: f32,
    frame: &'a mut ExtractedFrame,
}

impl SceneExtractor {
    pub(super) fn push_voxel_worlds(
        &self,
        world: &World,
        cameras: &ResolvedCameras,
        textures: &TextureBindings,
        tile_sets: Option<&TileSetBindings>,
        seconds: f32,
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
                push_commands(runtime.release_all(), 0, camera, frame);
            }
        }

        for (entity, component) in components {
            let target = VoxelTarget {
                camera,
                textures,
                tile_sets,
                seconds,
                frame: &mut *frame,
            };
            let pushed = self.push_voxel_world(&mut runtimes, entity, &component, world, target);
            match pushed {
                Ok(()) => {}
                Err(error) if self.tolerant() => {
                    self.record(entity, VoxelWorldComponent::TYPE_NAME, &error);
                }
                Err(error) => return Err(error),
            }
        }
        Ok(())
    }

    /// One voxel world's commands for this frame.
    ///
    /// A definition that cannot be used -- an ID no material defines, two
    /// materials with one ID, a face texture that does not parse -- is checked
    /// before the resident world is replaced. Tolerantly, the world it would
    /// have replaced keeps drawing, and the error is still returned for the
    /// caller to record: the terrain stays on screen and responsive to the
    /// camera while the edit that broke it is finished or undone.
    fn push_voxel_world(
        &self,
        runtimes: &mut BTreeMap<EntityId, ResidentVoxelWorld>,
        entity: EntityId,
        component: &VoxelWorldComponent,
        world: &World,
        target: VoxelTarget<'_>,
    ) -> Result<(), SceneExtractError> {
        let VoxelTarget {
            camera,
            textures,
            tile_sets,
            seconds,
            frame,
        } = target;
        let refused = match definition(component, tile_sets).and_then(|definition| {
            let resolved = resolve_appearance(&definition.appearance, textures)?;
            Ok((definition, resolved))
        }) {
            Ok((definition, resolved)) => {
                let fresh = runtimes.get(&entity).is_some_and(|runtime| {
                    runtime.definition == definition && runtime.resolved == resolved
                });
                if !fresh {
                    if let Some(mut previous) = runtimes.remove(&entity) {
                        push_commands(previous.release_all(), component.layer, camera, frame);
                    }
                    runtimes.insert(entity, ResidentVoxelWorld::new(definition, resolved));
                }
                None
            }
            Err(error) if self.tolerant() && runtimes.contains_key(&entity) => Some(error),
            Err(error) => return Err(error),
        };
        let focus = SectionCoord::new(component.focus[0], component.focus[1], component.focus[2]);
        let transform = world.world_transform(entity).unwrap_or_default();
        let root = transform_matrix(transform);
        let mut commands = runtimes
            .get_mut(&entity)
            .expect("the voxel runtime was inserted or kept above")
            .commands(focus, camera.view_projection * root, seconds)?;
        for command in &mut commands {
            if let FrameCommand::CachedTexturedMesh { model, .. } = command {
                *model = root * *model;
            }
        }
        push_commands(commands, component.layer, camera, frame);
        refused.map_or(Ok(()), Err)
    }
}

#[allow(clippy::cast_precision_loss)]
fn section_in_view(bounds: sindri_voxel::SectionBounds, view_projection: glam::Mat4) -> bool {
    let min = bounds.min;
    let max = bounds.max_exclusive;
    aabb_in_view(
        glam::Vec3::new(min.x as f32, min.y as f32, min.z as f32),
        glam::Vec3::new(max.x as f32, max.y as f32, max.z as f32),
        view_projection,
    )
}

fn push_commands(
    commands: Vec<FrameCommand>,
    layer: i32,
    camera: super::camera::ResolvedCamera,
    frame: &mut ExtractedFrame,
) {
    for command in commands {
        frame.push(FramePass::new(
            RenderStage::Opaque3d,
            RenderLayer(layer),
            FrameCamera {
                view_projection: camera.view_projection,
                position: camera.view.inverse().transform_point3(glam::Vec3::ZERO),
            },
            command,
        ));
    }
}

fn definition(
    component: &VoxelWorldComponent,
    tile_sets: Option<&TileSetBindings>,
) -> Result<VoxelWorldDefinition, SceneExtractError> {
    if component.render_radius > MAX_RESIDENCY_RADIUS
        || component.vertical_radius > MAX_RESIDENCY_RADIUS
    {
        return Err(SceneExtractError::VoxelRadiusTooLarge {
            horizontal: component.render_radius,
            vertical: component.vertical_radius,
            maximum: MAX_RESIDENCY_RADIUS,
        });
    }
    let (palette, appearance) = match component.blocks.as_deref() {
        Some(set) if !set.trim().is_empty() => {
            let tile_set = tile_sets
                .and_then(|bindings| bindings.get(set))
                .ok_or_else(|| SceneExtractError::UnboundTileSet(set.to_owned()))?;
            block_palette(set, tile_set)?
        }
        _ => {
            let ids: BTreeSet<_> = component
                .materials
                .iter()
                .map(|material| material.voxel)
                .collect();
            if ids.len() != component.materials.len() || ids.contains(&VoxelId::AIR.value()) {
                return Err(SceneExtractError::InvalidVoxelMaterials);
            }
            (
                Palette::of_materials(ids),
                Appearance::Materials(component.materials.clone()),
            )
        }
    };
    let source = terrain_source(&component.generator, &palette)?;
    Ok(VoxelWorldDefinition {
        source,
        appearance,
        render_radius: component.render_radius,
        vertical_radius: component.vertical_radius,
    })
}
