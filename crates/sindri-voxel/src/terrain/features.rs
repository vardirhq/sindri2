//! What turns a heightmap into a world: water, overhangs, caves and trees.

use crate::{VoxelCoord, VoxelId};

use super::NaturalTerrain;
use super::column::{Column, OVERHANG};
use super::noise::{hash2, noise3};

const OVERHANG_SALT: u64 = 0xD0;
const TUNNEL_A: u64 = 0xCA;
const TUNNEL_B: u64 = 0xCB;
const CAVERN: u64 = 0xCC;
const TREE_X: u64 = 0xF1;
const TREE_Z: u64 = 0xF2;
const TREE_CHANCE: u64 = 0xF3;
const TREE_HEIGHT: u64 = 0xF4;

/// Trees stand one to a square of this many voxels, at most, which keeps a
/// forest walkable and lets a voxel find the trees that could reach it by
/// looking at a handful of squares.
const TREE_SPACING: i32 = 6;

/// How far a crown reaches from its trunk.
const CROWN: i32 = 2;

/// How far above a column's ground a tree could put a voxel: the tallest
/// trunk and crown, standing on ground a few voxels higher next door.
pub(super) const TREE_REACH: i32 = 14;

/// A ridge only undercuts where it is this much of a range.
const OVERHANG_FROM: f32 = 0.2;

/// Below this warmth a biome's trees are conifers.
const CONIFER_BELOW: f32 = 0.35;

#[derive(Clone, Copy, Debug)]
pub(super) struct Tree {
    x: i32,
    z: i32,
    /// The first trunk voxel, on top of the ground.
    base: i32,
    /// The last trunk voxel.
    top: i32,
    conifer: bool,
}

impl NaturalTerrain {
    pub(super) fn voxel_in(&self, column: &Column, coord: VoxelCoord, trees: &[Tree]) -> VoxelId {
        let settings = &self.settings;
        let ground = column.shape.ground;
        let solid = self.solid(column, coord);
        if !solid {
            return self.open_voxel(column, coord, trees);
        }
        if self.cave(column, coord) {
            return VoxelId::AIR;
        }
        let stone = settings.palette.stone;
        if coord.y > ground {
            // The underside of an overhang: rock, whatever the surface is.
            return settings.palette.cliff.unwrap_or(stone);
        }
        match ground - coord.y {
            0 => column.top,
            depth if depth <= column.under_depth => column.under,
            _ => stone,
        }
    }

    /// Whether the ground occupies a voxel, before caves are cut from it.
    ///
    /// Away from the ranges this is the heightmap. In them, a band around the
    /// surface is decided by a three-dimensional field instead, squashed
    /// vertically so it makes ledges and undercuts rather than blobs.
    #[allow(clippy::cast_precision_loss)]
    fn solid(&self, column: &Column, coord: VoxelCoord) -> bool {
        let ground = column.shape.ground;
        let reach = if column.shape.mountain > OVERHANG_FROM {
            column.shape.mountain * OVERHANG
        } else {
            0.0
        };
        let height = (coord.y - ground) as f32;
        if height.abs() >= reach {
            return coord.y <= ground;
        }
        let noise = noise3(
            self.settings.seed ^ OVERHANG_SALT,
            coord.x as f32 / 14.0,
            coord.y as f32 / 7.0,
            coord.z as f32 / 14.0,
        );
        -height / reach + (noise - 0.5) * 1.6 > 0.0
    }

    /// Air, water or a tree: whatever is in a voxel the ground does not fill.
    fn open_voxel(&self, column: &Column, coord: VoxelCoord, trees: &[Tree]) -> VoxelId {
        let settings = &self.settings;
        if coord.y <= settings.sea_level
            && let Some(water) = settings.palette.water
        {
            if coord.y == settings.sea_level && column.frozen {
                return settings.palette.ice.unwrap_or(water);
            }
            return water;
        }
        if self.may_hold_a_tree(column, coord.y) {
            for tree in trees {
                if let Some(voxel) = self.tree_voxel(tree, coord) {
                    return voxel;
                }
            }
        }
        VoxelId::AIR
    }

    /// Tunnels where two slow fields both cross their middle — a worm through
    /// the rock rather than a bubble — and caverns deep down where a third is
    /// high. Never within a few voxels of the surface except in the ranges,
    /// where a tunnel breaking out is a cave mouth; never under shallow water,
    /// where it would drain nothing and show as a hole in the sea bed.
    #[allow(clippy::cast_precision_loss)]
    fn cave(&self, column: &Column, coord: VoxelCoord) -> bool {
        let settings = &self.settings;
        if !settings.caves {
            return false;
        }
        let ground = column.shape.ground;
        let cover = if ground < settings.sea_level + 2 {
            8
        } else if column.shape.mountain > 0.3 {
            1
        } else {
            4
        };
        if ground - coord.y < cover {
            return false;
        }
        let at = [coord.x as f32, coord.y as f32, coord.z as f32];
        let field = |salt: u64, across: f32, up: f32| {
            noise3(
                settings.seed ^ salt,
                at[0] / across,
                at[1] / up,
                at[2] / across,
            )
        };
        // The second field is only asked when the first is near its middle,
        // which it seldom is: most rock costs one sample, not two.
        if (field(TUNNEL_A, 22.0, 14.0) - 0.5).abs() < 0.045
            && (field(TUNNEL_B, 22.0, 14.0) - 0.5).abs() < 0.07
        {
            return true;
        }
        coord.y < settings.sea_level - 12 && field(CAVERN, 40.0, 22.0) > 0.74
    }

    pub(super) fn may_hold_a_tree(&self, column: &Column, y: i32) -> bool {
        let palette = self.settings.palette;
        palette.trunk.is_some()
            && palette.leaves.is_some()
            && y > column.shape.ground.max(self.settings.sea_level)
            && y <= column.shape.ground + TREE_REACH
    }

    /// Every tree whose crown could reach a column in this rectangle.
    pub(super) fn trees_near(&self, min_x: i32, min_z: i32, max_x: i32, max_z: i32) -> Vec<Tree> {
        let palette = self.settings.palette;
        if palette.trunk.is_none() || palette.leaves.is_none() {
            return Vec::new();
        }
        let cells = |min: i32, max: i32| {
            (min - CROWN).div_euclid(TREE_SPACING)..=(max + CROWN).div_euclid(TREE_SPACING)
        };
        let mut trees = Vec::new();
        for cell_z in cells(min_z, max_z) {
            for cell_x in cells(min_x, max_x) {
                if let Some(tree) = self.tree_in(cell_x, cell_z) {
                    trees.push(tree);
                }
            }
        }
        trees
    }

    /// The tree standing in one square, if one does.
    ///
    /// Kept a voxel in from the square's edges, so neighbouring trunks are
    /// never side by side, and decided by the biome of the ground it stands
    /// on.
    #[allow(clippy::cast_possible_truncation)]
    fn tree_in(&self, cell_x: i32, cell_z: i32) -> Option<Tree> {
        let seed = self.settings.seed;
        let inset = |salt| 1 + (hash2(seed ^ salt, cell_x, cell_z) * 4.0) as i32;
        let x = cell_x * TREE_SPACING + inset(TREE_X);
        let z = cell_z * TREE_SPACING + inset(TREE_Z);
        let column = self.column(x, z);
        let biome = &self.settings.biomes[column.shape.biome];
        if !column.open
            || column.shape.ground < self.settings.sea_level
            || hash2(seed ^ TREE_CHANCE, cell_x, cell_z) >= biome.trees
        {
            return None;
        }
        let conifer = biome.temperature < CONIFER_BELOW;
        let tall = (hash2(seed ^ TREE_HEIGHT, cell_x, cell_z) * 3.0) as i32;
        let height = if conifer { 5 + tall } else { 4 + tall };
        let base = column.shape.ground + 1;
        Some(Tree {
            x,
            z,
            base,
            top: base + height - 1,
            conifer,
        })
    }

    fn tree_voxel(&self, tree: &Tree, coord: VoxelCoord) -> Option<VoxelId> {
        let palette = self.settings.palette;
        let (dx, dz) = ((coord.x - tree.x).abs(), (coord.z - tree.z).abs());
        if dx == 0 && dz == 0 && (tree.base..=tree.top).contains(&coord.y) {
            return palette.trunk;
        }
        let above = coord.y - tree.top;
        let leafy = if tree.conifer {
            // A cone: wide near the bottom of the crown, a point on top.
            match above {
                -3 => dx.max(dz) <= 2 && dx + dz <= 3,
                -2 | 0 => dx + dz <= 1,
                -1 => dx.max(dz) <= 1,
                1 => dx + dz == 0,
                _ => false,
            }
        } else {
            // A chunky broadleaf crown, its corners bitten off so it is not
            // a green cube from any of the camera's views.
            match above {
                -2 | -1 => dx.max(dz) <= 2 && !(dx == 2 && dz == 2),
                0 => dx.max(dz) <= 1,
                1 => dx + dz <= 1,
                _ => false,
            }
        };
        if leafy { palette.leaves } else { None }
    }
}
