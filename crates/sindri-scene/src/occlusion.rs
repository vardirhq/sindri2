//! Walking a virtual actor over every standable surface to see what covers it.
//!
//! Isometric ordering fails in places rather than in general. A rule can be
//! right everywhere a test happens to look and wrong in the one corner where a
//! raised plot meets open ground, and the way that reaches a player is that
//! nobody stood there. So something stands everywhere.
//!
//! The sweep puts a probe on every column that has ground in it, asks the two
//! questions the renderer asks -- how deep is this cell's face drawn, how deep
//! is something standing here -- and reports every pair where a cell that
//! cannot physically cover the probe is drawn after it anyway. It asks them by
//! calling `TileGridComponent::face_depth` and `standing_depth` rather than
//! restating them, so a sweep that passes is evidence about the renderer and
//! not about a second copy of its arithmetic.
//!
//! What it does not do is decide the draw order. That is still one number per
//! entity, and `docs/2d-depth-and-placement.md` records why one number cannot
//! always be right. The sweep is how the places it is wrong stop being found by
//! looking at a phone.

use sindri_core::{ComponentSchemaRegistry, EntityId, World};
use sindri_grid::{GridCoord, GridCoord3};
use thiserror::Error;

use crate::extract::face_is_occluded;
use crate::{
    SceneExtractError, TileGridComponent, TileSetBindings, TileSurfaceError, TileSurfaces,
    TileVolumeComponent, TileVolumeError, blocking_step_ahead, standing_depth,
};

/// The virtual actor a sweep walks over the world.
///
/// A size rather than a sprite, because what matters is the screen area it
/// covers: a tall actor reaches cells a short one never touches, and the two do
/// not have the same answer. Given in the grid's own plane units, as a tile
/// face's `size` is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OcclusionProbe {
    /// Width and height of the area the actor covers.
    pub size: [f32; 2],
    /// Where that area sits relative to the point the actor stands on.
    pub offset: [f32; 2],
}

impl Default for OcclusionProbe {
    /// One cell wide and two tall, standing on its bottom edge.
    ///
    /// A person-shaped thing. A sweep for a specific actor should pass that
    /// actor's own bounds instead.
    fn default() -> Self {
        Self {
            size: [1.0, 2.0],
            offset: [0.0, 1.0],
        }
    }
}

/// One place where something drew over an actor that it could not cover.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OcclusionFinding {
    /// The volume this happened in, so a caller with several can tell them
    /// apart and project each through its own grid.
    pub volume: EntityId,
    /// The column the probe was standing in.
    pub standing: GridCoord,
    /// The cell whose top the probe was standing on.
    pub standing_cell: GridCoord3,
    /// How high the ground under the probe was.
    pub feet: f32,
    /// The cell whose face was drawn over it.
    pub cell: GridCoord3,
    /// How high that cell's column stands. Never above `feet`, or it would be
    /// a wall and covering the probe would be its job.
    pub surface: f32,
    /// The raised step ahead that took the probe's clearance away, if one did.
    ///
    /// `Some` is the known corner: the probe is standing beside a wall, one
    /// depth cannot answer for the wall and the open ground at once, and it
    /// answered for the wall. `None` is a finding nothing explains, which is a
    /// bug in the ordering rule rather than a case it declines.
    pub blocked_by: Option<GridCoord>,
}

impl OcclusionFinding {
    /// Whether the documented corner explains this finding.
    #[must_use]
    pub const fn is_explained(&self) -> bool {
        self.blocked_by.is_some()
    }
}

/// What a sweep found, and how much it looked at.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct OcclusionReport {
    pub findings: Vec<OcclusionFinding>,
    /// How many places the probe was put.
    pub probes: usize,
    /// How many faces were tested against it in total.
    pub faces: usize,
}

impl OcclusionReport {
    #[must_use]
    pub fn is_clean(&self) -> bool {
        self.findings.is_empty()
    }

    /// Findings the documented corner does not account for.
    ///
    /// What a sweep is actually asked. The corner is known, recorded and
    /// bounded; anything outside it is the ordering rule being wrong somewhere
    /// nobody has looked yet.
    pub fn unexplained(&self) -> impl Iterator<Item = &OcclusionFinding> {
        self.findings
            .iter()
            .filter(|finding| !finding.is_explained())
    }
}

#[derive(Debug, Error)]
pub enum OcclusionError {
    #[error("a grid or volume payload is invalid: {source}")]
    InvalidPayload {
        #[source]
        source: sindri_core::ComponentRegistryError,
    },
    #[error("a swept volume is invalid: {source}")]
    InvalidVolume {
        #[source]
        source: TileVolumeError,
    },
    #[error("a swept volume's surfaces cannot be derived: {source}")]
    InvalidSurface {
        #[source]
        source: TileSurfaceError,
    },
    #[error("a swept volume names tile set `{0}`, which is not bound")]
    UnboundTileSet(String),
    #[error("a swept volume could not be resolved: {source}")]
    Unresolvable {
        #[source]
        source: Box<SceneExtractError>,
    },
}

/// Walks `probe` over every standable surface of every volume in `world`.
pub fn sweep_occlusion(
    world: &World,
    components: &ComponentSchemaRegistry,
    tile_sets: &TileSetBindings,
    probe: OcclusionProbe,
) -> Result<OcclusionReport, OcclusionError> {
    let mut report = OcclusionReport::default();
    let volumes = components
        .query::<TileVolumeComponent>(world)
        .map_err(|source| OcclusionError::InvalidPayload { source })?;
    for (entity, volume) in volumes {
        let Some(grid) = components
            .get::<TileGridComponent>(world, entity)
            .map_err(|source| OcclusionError::InvalidPayload { source })?
        else {
            continue;
        };
        sweep_volume(entity, &grid, &volume, tile_sets, probe, &mut report)?;
    }
    report.findings.sort_by_key(|finding| {
        (
            finding.standing.x,
            finding.standing.y,
            finding.cell.x,
            finding.cell.y,
            finding.cell.z,
        )
    });
    Ok(report)
}

fn sweep_volume(
    entity: EntityId,
    grid: &TileGridComponent,
    volume: &TileVolumeComponent,
    tile_sets: &TileSetBindings,
    probe: OcclusionProbe,
    report: &mut OcclusionReport,
) -> Result<(), OcclusionError> {
    if volume.cells.is_empty() {
        return Ok(());
    }
    let tile_set = tile_sets
        .get(&volume.tileset)
        .ok_or_else(|| OcclusionError::UnboundTileSet(volume.tileset.clone()))?;
    let occupied = volume
        .index(grid)
        .map_err(|source| OcclusionError::InvalidVolume { source })?;
    let surfaces = TileSurfaces::derive(volume, tile_set)
        .map_err(|source| OcclusionError::InvalidSurface { source })?;
    let visible = grid.projection.visible_faces();

    // Every face that is actually drawn, with where it is drawn and how deep.
    let mut drawn = Vec::new();
    for (coord, tile) in volume.occupied() {
        let Some(definition) = tile_set.tile(tile) else {
            continue;
        };
        let Some(origin) = grid.cell_to_local(coord) else {
            continue;
        };
        let depth = grid.face_depth(coord);
        for (face, visual) in definition.faces.iter() {
            if !visible.contains(&face) {
                continue;
            }
            let occluded = face_is_occluded(
                &occupied,
                &volume.tileset,
                tile_set,
                coord,
                face,
                definition,
            )
            .map_err(|source| OcclusionError::Unresolvable {
                source: Box::new(source),
            })?;
            if occluded {
                continue;
            }
            drawn.push((coord, quad(origin, visual.size, visual.offset), depth));
        }
    }

    for (standing, feet) in surfaces.columns() {
        let Some(standing_cell) = surfaces.top(standing) else {
            continue;
        };
        let at = (f64::from(standing.x), f64::from(standing.y));
        let Some(origin) = grid.point_to_local_at_height(at.0, at.1, feet) else {
            continue;
        };
        let actor = quad(origin, probe.size, probe.offset);
        let actor_depth = standing_depth(grid, Some(&surfaces), at, feet);
        let blocked_by = blocking_step_ahead(grid, Some(&surfaces), at, feet);
        report.probes += 1;
        for (cell, face, depth) in &drawn {
            // A cell whose ground is higher than the probe's feet is a wall,
            // and covering the probe is exactly what it is for.
            let column = GridCoord::new(cell.x, cell.y);
            if surfaces.height(column).is_none_or(|surface| surface > feet) {
                continue;
            }
            if !overlaps(&actor, face) {
                continue;
            }
            report.faces += 1;
            if *depth > actor_depth {
                report.findings.push(OcclusionFinding {
                    volume: entity,
                    standing,
                    standing_cell,
                    feet,
                    cell: *cell,
                    surface: surfaces.height(column).unwrap_or_default(),
                    blocked_by,
                });
            }
        }
    }
    Ok(())
}

/// Minimum and maximum corners of a quad drawn at `origin`.
fn quad(origin: [f32; 2], size: [f32; 2], offset: [f32; 2]) -> [f32; 4] {
    let (x, y) = (origin[0] + offset[0], origin[1] + offset[1]);
    let (half_w, half_h) = (size[0] / 2.0, size[1] / 2.0);
    [x - half_w, y - half_h, x + half_w, y + half_h]
}

/// Whether two quads share any area.
///
/// Touching is not overlapping: a cell's face meeting an actor exactly on an
/// edge shares a line and no pixels, and reporting it would bury the real
/// findings under every tile the actor stands beside.
fn overlaps(a: &[f32; 4], b: &[f32; 4]) -> bool {
    a[0] < b[2] && b[0] < a[2] && a[1] < b[3] && b[1] < a[3]
}
