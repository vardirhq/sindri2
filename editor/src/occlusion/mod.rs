//! Showing an author where the ground draws over what stands on it.
//!
//! `sindri_scene::sweep_occlusion` answers this for a whole volume, and the
//! answer is a list of coordinates. Twenty-two of those tell you nothing; seeing
//! that all twenty-two lie along one edge of one plot tells you there is one
//! cause rather than twenty-two bugs. So the report is drawn on the grid it is
//! about.
//!
//! On the grid rather than on the view, because that is what a fault belongs to:
//! it is a fact about two cells and the space between them, and it projects
//! through the volume's own transform cell by cell, exactly as the block preview
//! beside it does. Move the grid and the marks move with it.
//!
//! The sweep costs far too much to run per frame, so it is cached against a
//! fingerprint of the volumes it read. Undo restores a payload, the fingerprint
//! goes back with it, and the marks follow — without this module knowing that
//! undo exists.

use std::collections::BTreeMap;
use std::collections::btree_map::Entry;
use std::hash::{Hash, Hasher};

use glam::Mat4;
use sindri_core::{ComponentSchemaRegistry, EntityId, Transform3D, World};
use sindri_grid::GridCoord3;
use sindri_scene::{
    OcclusionProbe, OcclusionReport, TileGridComponent, TileSetBindings, TileVolumeComponent,
    sweep_occlusion,
};

use crate::tile_volume::cell_outline;

/// One fault, ready to paint.
pub struct FaultMark {
    /// The ground the probe was standing on.
    pub standing: [[f32; 2]; 4],
    /// The cell drawn over it.
    pub covering: [[f32; 2]; 4],
    /// The raised step that took the probe's clearance away, when one did.
    ///
    /// `None` means nothing accounts for this fault, which is worth drawing
    /// differently: the others are a case the rule declines, this is the rule
    /// being wrong.
    pub blocked_by: Option<[[f32; 2]; 4]>,
}

/// A cached sweep of the open scene.
#[derive(Default)]
pub struct OcclusionOverlay {
    pub enabled: bool,
    fingerprint: Option<u64>,
    report: OcclusionReport,
    problem: Option<String>,
}

impl OcclusionOverlay {
    /// Re-sweeps when the volumes have changed since the last one.
    pub fn ensure(
        &mut self,
        world: &World,
        components: &ComponentSchemaRegistry,
        tile_sets: &TileSetBindings,
        probe: OcclusionProbe,
    ) {
        if !self.enabled {
            self.fingerprint = None;
            self.report = OcclusionReport::default();
            self.problem = None;
            return;
        }
        let fingerprint = fingerprint(world, components);
        if self.fingerprint == Some(fingerprint) {
            return;
        }
        self.fingerprint = Some(fingerprint);
        match sweep_occlusion(world, components, tile_sets, probe) {
            Ok(report) => {
                self.report = report;
                self.problem = None;
            }
            Err(error) => {
                self.report = OcclusionReport::default();
                self.problem = Some(error.to_string());
            }
        }
    }

    #[must_use]
    pub const fn report(&self) -> &OcclusionReport {
        &self.report
    }

    /// Why the last sweep produced nothing, when that was a failure.
    #[must_use]
    pub fn problem(&self) -> Option<&str> {
        self.problem.as_deref()
    }

    /// What to say about the sweep in one line.
    #[must_use]
    pub fn summary(&self) -> String {
        if let Some(problem) = self.problem() {
            return format!("Ordering: {problem}");
        }
        let unexplained = self.report.unexplained().count();
        match (self.report.findings.len(), unexplained) {
            (0, _) => format!("Ordering: clear over {} surfaces", self.report.probes),
            (total, 0) => format!(
                "Ordering: {total} of {} surfaces hit the wall corner",
                self.report.probes
            ),
            (total, unexplained) => format!(
                "Ordering: {total} faults over {} surfaces, {unexplained} unexplained",
                self.report.probes
            ),
        }
    }

    /// Every fault in the world, projected into the viewport.
    ///
    /// Grouped by volume rather than taken one at a time, because each one
    /// projects through its own grid and transform: two grids in a scene are
    /// two different spaces, and a mark drawn through the wrong one lands
    /// somewhere convincing and false.
    pub fn marks(
        &self,
        world: &World,
        components: &ComponentSchemaRegistry,
        view_projection: Mat4,
    ) -> Vec<FaultMark> {
        let mut marks = Vec::new();
        let mut grids: BTreeMap<EntityId, (TileGridComponent, Transform3D)> = BTreeMap::new();
        for finding in &self.report.findings {
            let entry = match grids.entry(finding.volume) {
                Entry::Occupied(entry) => entry.into_mut(),
                Entry::Vacant(slot) => {
                    let Ok(Some(grid)) = components.get::<TileGridComponent>(world, finding.volume)
                    else {
                        continue;
                    };
                    let transform = world
                        .get(finding.volume)
                        .and_then(|data| data.transform_3d)
                        .unwrap_or_default();
                    slot.insert((grid, transform))
                }
            };
            let (grid, transform) = (&entry.0, entry.1);
            let outline = |coord: GridCoord3| cell_outline(grid, transform, view_projection, coord);
            let Some(standing) = outline(finding.standing_cell) else {
                continue;
            };
            let Some(covering) = outline(finding.cell) else {
                continue;
            };
            marks.push(FaultMark {
                standing,
                covering,
                blocked_by: finding
                    .blocked_by
                    .and_then(|column| self.standing_cell_of(finding.volume, column))
                    .and_then(outline),
            });
        }
        marks
    }

    /// The cell another probe stood on in that column.
    ///
    /// The sweep reports the blocking *column*, because which of its cells is
    /// the wall is not what the ordering rule asks. For drawing, a probe that
    /// stood there supplies one; where none did, the mark is left off rather
    /// than guessed at.
    fn standing_cell_of(
        &self,
        volume: EntityId,
        column: sindri_grid::GridCoord,
    ) -> Option<GridCoord3> {
        self.report
            .findings
            .iter()
            .find(|finding| finding.volume == volume && finding.standing == column)
            .map(|finding| finding.standing_cell)
    }
}

/// A cheap stand-in for "the volumes have not changed".
///
/// Every cell of every volume, which is a few hundred integers rather than the
/// several hundred thousand overlap tests a sweep is. Recomputed per frame so
/// that any edit, undo or scene change is picked up without this module being
/// told about it.
fn fingerprint(world: &World, components: &ComponentSchemaRegistry) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    let Ok(volumes) = components.query::<TileVolumeComponent>(world) else {
        return 0;
    };
    for (entity, volume) in volumes {
        entity.index().hash(&mut hasher);
        volume.tileset.hash(&mut hasher);
        volume.layer.hash(&mut hasher);
        for cell in &volume.cells {
            cell.position.hash(&mut hasher);
            cell.tile.hash(&mut hasher);
        }
        if let Ok(Some(grid)) = components.get::<TileGridComponent>(world, entity) {
            grid.columns.hash(&mut hasher);
            grid.rows.hash(&mut hasher);
            grid.cell_size.map(f32::to_bits).hash(&mut hasher);
            grid.level_step.map(f32::to_bits).hash(&mut hasher);
            grid.depth_step.to_bits().hash(&mut hasher);
        }
    }
    hasher.finish()
}

#[cfg(test)]
mod tests;
