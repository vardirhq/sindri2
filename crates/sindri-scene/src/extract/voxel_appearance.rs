//! What a voxel world's faces look like: which texture each face draws with,
//! which of them move or glow, and which of them let the faces behind show.
//!
//! Apart from the world's residency and meshing because it is a different
//! question. A world is rebuilt when its terrain changes; its appearance is
//! resolved again whenever a texture it names arrives, and its looks are asked
//! about every frame.

use std::collections::BTreeMap;

use sindri_core::{SpriteRef, TileDefinition, TileFace, TileSetDocument};
use sindri_render::MeshSurface;
use sindri_voxel::{VoxelFace, VoxelId, VoxelMaterial, VoxelMaterialSource};

use crate::{TextureBindings, VoxelMaterialDocument, VoxelTexture};

use super::SceneExtractError;
use super::voxel_source::Palette;

/// What each stored voxel ID looks like.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Appearance {
    /// The world's own materials, each a top, side and bottom texture.
    Materials(Vec<VoxelMaterialDocument>),
    /// Blocks from a block set, each with the ID the engine numbered it as.
    Blocks(Vec<(u16, String, TileDefinition)>),
}

/// Every face's texture, and the looks some of them are drawn with.
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct Resolved {
    pub(super) faces: BTreeMap<(u16, VoxelFace), VoxelTexture>,
    /// Indexed by a face's look, less one: look zero is plain and has no
    /// entry.
    pub(super) looks: Vec<Look>,
}

/// How a batch of faces is drawn beyond its texture.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Look {
    glow: f32,
    /// Where each frame sits relative to the one the faces were meshed with,
    /// and how many to show a second. The first frame is always no offset.
    frames: Vec<[f32; 2]>,
    fps: f32,
}

impl Look {
    /// What this look draws as, `seconds` into the scene.
    pub(super) fn surface(&self, seconds: f32) -> MeshSurface {
        let frame = if self.frames.len() > 1 && seconds.is_finite() && seconds > 0.0 {
            // Truncating is the frame's number; the modulo keeps it in range
            // however long the scene has been running.
            #[allow(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                clippy::cast_precision_loss
            )]
            let index = ((seconds * self.fps) as u64 % self.frames.len() as u64) as usize;
            self.frames[index]
        } else {
            [0.0, 0.0]
        };
        MeshSurface {
            uv_offset: frame,
            glow: self.glow,
            alpha_cutoff: 0.0,
        }
    }
}

impl Resolved {
    /// What a batch with this look draws as this frame.
    pub(super) fn surface(&self, look: u16, seconds: f32) -> MeshSurface {
        usize::from(look)
            .checked_sub(1)
            .and_then(|index| self.looks.get(index))
            .map_or_else(MeshSurface::default, |look| look.surface(seconds))
    }
}

/// Every block in a set, numbered from one in name order.
///
/// The numbers are the engine's: nothing authored names them, and a world is
/// rebuilt whenever its set changes, so a block added between two others
/// renumbering them changes nothing anyone can see.
pub(super) fn block_palette(
    set: &str,
    tile_set: &TileSetDocument,
) -> Result<(Palette, Appearance), SceneExtractError> {
    let mut palette = Palette::default();
    let mut blocks = Vec::with_capacity(tile_set.tiles.len());
    for (index, (name, block)) in tile_set.tiles.iter().enumerate() {
        let voxel = u16::try_from(index + 1)
            .ok()
            .filter(|voxel| *voxel != VoxelId::AIR.value())
            .ok_or_else(|| SceneExtractError::TooManyVoxelBlocks(set.to_owned()))?;
        palette.blocks.insert(name.clone(), voxel);
        blocks.push((voxel, name.clone(), block.clone()));
    }
    Ok((palette, Appearance::Blocks(blocks)))
}

const VOXEL_FACES: [VoxelFace; 6] = [
    VoxelFace::Left,
    VoxelFace::Right,
    VoxelFace::Back,
    VoxelFace::Front,
    VoxelFace::Top,
    VoxelFace::Bottom,
];

/// The face of a tile a voxel face shows.
///
/// A tile grid's third axis is up and its second runs south; a voxel world's
/// second axis is up and its third runs toward the front. So a tile's south
/// face is a voxel's front, and its north is the back.
const fn tile_face(face: VoxelFace) -> TileFace {
    match face {
        VoxelFace::Left => TileFace::West,
        VoxelFace::Right => TileFace::East,
        VoxelFace::Back => TileFace::North,
        VoxelFace::Front => TileFace::South,
        VoxelFace::Top => TileFace::Top,
        VoxelFace::Bottom => TileFace::Bottom,
    }
}

const fn face_name(face: TileFace) -> &'static str {
    match face {
        TileFace::Top => "top",
        TileFace::Bottom => "bottom",
        TileFace::North => "north",
        TileFace::South => "south",
        TileFace::East => "east",
        TileFace::West => "west",
    }
}

pub(super) fn resolve_appearance(
    appearance: &Appearance,
    textures: &TextureBindings,
) -> Result<Resolved, SceneExtractError> {
    let mut resolved = Resolved::default();
    let lookup = |sprite: &str| -> Result<VoxelTexture, SceneExtractError> {
        let reference = SpriteRef::parse(sprite)?;
        let (texture, uv) = textures.resolve_sprite(&reference);
        Ok(VoxelTexture::new(texture, uv))
    };
    match appearance {
        Appearance::Materials(materials) => {
            for material in materials {
                for face in VOXEL_FACES {
                    let texture = match face {
                        VoxelFace::Top => &material.top,
                        VoxelFace::Bottom => &material.bottom,
                        VoxelFace::Left | VoxelFace::Right | VoxelFace::Back | VoxelFace::Front => {
                            &material.side
                        }
                    };
                    resolved
                        .faces
                        .insert((material.voxel, face), lookup(texture)?);
                }
            }
        }
        Appearance::Blocks(blocks) => {
            for (voxel, name, block) in blocks {
                for face in VOXEL_FACES {
                    let tile = tile_face(face);
                    let (_, visual) = block.faces.resolved(tile).ok_or_else(|| {
                        SceneExtractError::VoxelBlockWithoutFace {
                            block: name.clone(),
                            face: face_name(tile),
                        }
                    })?;
                    let first = lookup(&visual.sprite)?;
                    let frames = match &visual.animation {
                        Some(animation) => frame_offsets(name, first, &animation.frames, lookup)?,
                        None => vec![[0.0, 0.0]],
                    };
                    let fps = visual
                        .animation
                        .as_ref()
                        .map_or(0.0, |animation| animation.fps);
                    let mut texture = first;
                    if block.glow > 0.0 || frames.len() > 1 {
                        let look = Look {
                            glow: block.glow,
                            frames,
                            fps,
                        };
                        texture = texture.with_look(resolved.look_for(look));
                    }
                    resolved.faces.insert((*voxel, face), texture);
                }
            }
        }
    }
    Ok(resolved)
}

impl Resolved {
    /// The number of this look, adding it if no face has used it yet, so a
    /// lake's six faces share one batch per texture rather than six.
    fn look_for(&mut self, look: Look) -> u16 {
        let index = self
            .looks
            .iter()
            .position(|known| *known == look)
            .unwrap_or_else(|| {
                self.looks.push(look);
                self.looks.len() - 1
            });
        u16::try_from(index + 1).unwrap_or(u16::MAX)
    }
}

/// Where each frame of an animation sits relative to its first.
///
/// Every frame must be the same size on the same texture as the first: the
/// faces are meshed once, with the first frame's coordinates, and a frame is
/// shown by shifting where they read.
fn frame_offsets(
    block: &str,
    first: VoxelTexture,
    frames: &[String],
    lookup: impl Fn(&str) -> Result<VoxelTexture, SceneExtractError>,
) -> Result<Vec<[f32; 2]>, SceneExtractError> {
    let mut offsets = vec![[0.0, 0.0]];
    for frame in frames {
        let texture = lookup(frame)?;
        let same_size = (texture.uv.width() - first.uv.width()).abs() < 1.0e-6
            && (texture.uv.height() - first.uv.height()).abs() < 1.0e-6;
        if texture.texture != first.texture || !same_size {
            return Err(SceneExtractError::VoxelAnimationFrame {
                block: block.to_owned(),
                frame: frame.clone(),
            });
        }
        offsets.push([texture.uv.x() - first.uv.x(), texture.uv.y() - first.uv.y()]);
    }
    Ok(offsets)
}

/// Which faces each voxel hides, and which pass it draws in.
///
/// Every block that hides its neighbours is opaque, as before. One that does
/// not is cut out (leaves, glass) and hides nothing, unless it is tagged
/// `liquid`: a lake hides the faces between its own blocks, so it has no
/// walls inside it, and still shows the ground it touches.
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct BlockMaterials {
    by_voxel: BTreeMap<u16, VoxelMaterial>,
}

impl BlockMaterials {
    pub(super) fn of(appearance: &Appearance) -> Self {
        let Appearance::Blocks(blocks) = appearance else {
            return Self::default();
        };
        let by_voxel = blocks
            .iter()
            .filter(|(_, _, block)| !block.occludes)
            .map(|(voxel, _, block)| {
                let liquid = block.tags.iter().any(|tag| tag == "liquid");
                let material = if liquid {
                    VoxelMaterial::new(
                        sindri_voxel::RenderClass::Opaque,
                        sindri_voxel::FaceOcclusion::MatchingVoxel,
                    )
                } else {
                    VoxelMaterial::cutout()
                };
                (*voxel, material)
            })
            .collect();
        Self { by_voxel }
    }
}

impl VoxelMaterialSource for BlockMaterials {
    fn material(&self, voxel: VoxelId) -> VoxelMaterial {
        self.by_voxel
            .get(&voxel.value())
            .copied()
            .unwrap_or_else(VoxelMaterial::opaque)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn look(frames: usize, fps: f32) -> Look {
        Look {
            glow: 0.0,
            frames: (0..frames)
                .map(|frame| {
                    #[allow(clippy::cast_precision_loss)]
                    let x = frame as f32 * 0.25;
                    [x, 0.0]
                })
                .collect(),
            fps,
        }
    }

    fn frame_at(look: &Look, seconds: f32) -> [f32; 2] {
        look.surface(seconds).uv_offset
    }

    fn near(a: [f32; 2], b: [f32; 2]) -> bool {
        (a[0] - b[0]).abs() < 1.0e-6 && (a[1] - b[1]).abs() < 1.0e-6
    }

    #[test]
    fn an_animation_steps_through_its_frames_and_wraps() {
        let water = look(4, 2.0);
        assert!(near(frame_at(&water, 0.0), [0.0, 0.0]));
        assert!(near(frame_at(&water, 0.6), [0.25, 0.0]));
        assert!(near(frame_at(&water, 1.6), [0.75, 0.0]));
        assert!(near(frame_at(&water, 2.1), [0.0, 0.0]), "it wraps");
    }

    #[test]
    fn look_zero_is_plain() {
        let resolved = Resolved::default();
        assert!(resolved.surface(0, 5.0) == MeshSurface::default());
    }
}
