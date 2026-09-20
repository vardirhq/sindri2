use std::collections::{BTreeMap, BTreeSet};

use crate::{SectionCoord, VoxelCoord, VoxelId, VoxelSection, VoxelSource, VoxelWorkQueue};

/// Distances around a focus section that the engine keeps for rendering and
/// simulation. Simulation may never exceed render residency.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResidencyConfig {
    pub render_horizontal: i32,
    pub render_vertical: i32,
    pub simulation_horizontal: i32,
    pub simulation_vertical: i32,
}

impl ResidencyConfig {
    #[must_use]
    pub fn new(
        render_horizontal: i32,
        render_vertical: i32,
        simulation_horizontal: i32,
        simulation_vertical: i32,
    ) -> Self {
        assert!(render_horizontal >= 0 && render_vertical >= 0);
        assert!(simulation_horizontal >= 0 && simulation_vertical >= 0);
        assert!(simulation_horizontal <= render_horizontal);
        assert!(simulation_vertical <= render_vertical);
        Self {
            render_horizontal,
            render_vertical,
            simulation_horizontal,
            simulation_vertical,
        }
    }
}

/// Exact residency delta after moving the world's focus.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ResidencyDelta {
    pub entered: Vec<SectionCoord>,
    pub stayed: Vec<SectionCoord>,
    pub left: Vec<SectionCoord>,
}

/// Sparse engine-owned voxel world.
///
/// Generated sections are only a residency cache. Edits are retained as sparse
/// world-coordinate overrides, so unloading a section never loses player work.
pub struct VoxelWorld<S> {
    source: S,
    config: ResidencyConfig,
    focus: Option<SectionCoord>,
    resident: BTreeMap<SectionCoord, VoxelSection>,
    simulation: BTreeSet<SectionCoord>,
    edits: BTreeMap<VoxelCoord, VoxelId>,
    dirty: BTreeSet<SectionCoord>,
    work: VoxelWorkQueue,
}

impl<S: VoxelSource> VoxelWorld<S> {
    #[must_use]
    pub fn new(source: S, config: ResidencyConfig) -> Self {
        Self {
            source,
            config,
            focus: None,
            resident: BTreeMap::new(),
            simulation: BTreeSet::new(),
            edits: BTreeMap::new(),
            dirty: BTreeSet::new(),
            work: VoxelWorkQueue::default(),
        }
    }

    pub fn move_focus(&mut self, focus: SectionCoord) -> ResidencyDelta {
        let wanted = section_window(
            focus,
            self.config.render_horizontal,
            self.config.render_vertical,
        );
        let simulation = section_window(
            focus,
            self.config.simulation_horizontal,
            self.config.simulation_vertical,
        );
        let previous: BTreeSet<_> = self.resident.keys().copied().collect();

        let entered: Vec<_> = wanted.difference(&previous).copied().collect();
        let stayed: Vec<_> = wanted.intersection(&previous).copied().collect();
        let left: Vec<_> = previous.difference(&wanted).copied().collect();

        for coord in &left {
            self.resident.remove(coord);
            self.work.forget(*coord);
        }
        for coord in &entered {
            self.work.queue_generation(*coord);
        }
        self.drain_generation();

        self.focus = Some(focus);
        self.simulation = simulation;
        ResidencyDelta {
            entered,
            stayed,
            left,
        }
    }

    #[must_use]
    pub fn voxel(&self, coord: VoxelCoord) -> VoxelId {
        if let Some(voxel) = self.edits.get(&coord) {
            return *voxel;
        }
        if let Some(section) = self.resident.get(&coord.section()) {
            return section.get(coord.local());
        }
        self.source.voxel(coord)
    }

    pub fn set_voxel(&mut self, coord: VoxelCoord, voxel: VoxelId) -> bool {
        if self.voxel(coord) == voxel {
            return false;
        }
        self.edits.insert(coord, voxel);
        if let Some(section) = self.resident.get_mut(&coord.section()) {
            section.set(coord.local(), voxel);
        }
        self.mark_edit_dirty(coord);
        true
    }

    #[must_use]
    pub fn resident(&self, coord: SectionCoord) -> Option<&VoxelSection> {
        self.resident.get(&coord)
    }

    #[must_use]
    pub fn resident_len(&self) -> usize {
        self.resident.len()
    }

    #[must_use]
    pub fn is_simulating(&self, coord: SectionCoord) -> bool {
        self.simulation.contains(&coord)
    }

    pub fn take_dirty(&mut self) -> Vec<SectionCoord> {
        std::mem::take(&mut self.dirty).into_iter().collect()
    }

    pub fn take_mesh_work(&mut self) -> Vec<SectionCoord> {
        self.work.take_meshes()
    }

    fn drain_generation(&mut self) {
        for coord in self.work.take_generation() {
            let mut section = self.source.generate_section(coord);
            self.apply_edits(coord, &mut section);
            self.resident.insert(coord, section);
            self.work.queue_mesh(coord);
        }
    }

    fn apply_edits(&self, section_coord: SectionCoord, section: &mut VoxelSection) {
        let min = section_coord.min_voxel();
        let max = VoxelCoord::new(min.x + 15, min.y + 15, min.z + 15);
        for (coord, voxel) in self.edits.range(min..=max) {
            if coord.section() == section_coord {
                section.set(coord.local(), *voxel);
            }
        }
    }

    fn mark_edit_dirty(&mut self, coord: VoxelCoord) {
        let section = coord.section();
        let local = coord.local();
        self.dirty.insert(section);
        self.work.queue_mesh(section);
        for (axis, edge) in [
            ((-1, 0, 0), local.x() == 0),
            ((1, 0, 0), local.x() == 15),
            ((0, -1, 0), local.y() == 0),
            ((0, 1, 0), local.y() == 15),
            ((0, 0, -1), local.z() == 0),
            ((0, 0, 1), local.z() == 15),
        ] {
            if edge {
                let neighbour =
                    SectionCoord::new(section.x + axis.0, section.y + axis.1, section.z + axis.2);
                self.dirty.insert(neighbour);
                if self.resident.contains_key(&neighbour) {
                    self.work.queue_mesh(neighbour);
                }
            }
        }
    }
}

fn section_window(focus: SectionCoord, horizontal: i32, vertical: i32) -> BTreeSet<SectionCoord> {
    let mut sections = BTreeSet::new();
    for y in (focus.y - vertical)..=(focus.y + vertical) {
        for z in (focus.z - horizontal)..=(focus.z + horizontal) {
            for x in (focus.x - horizontal)..=(focus.x + horizontal) {
                sections.insert(SectionCoord::new(x, y, z));
            }
        }
    }
    sections
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Empty;

    impl VoxelSource for Empty {
        fn voxel(&self, _coord: VoxelCoord) -> VoxelId {
            VoxelId::AIR
        }
    }

    #[test]
    fn moving_focus_reports_only_the_residency_delta() {
        let config = ResidencyConfig::new(1, 0, 0, 0);
        let mut world = VoxelWorld::new(Empty, config);
        let first = world.move_focus(SectionCoord::new(0, 0, 0));
        assert_eq!(first.entered.len(), 9);
        assert_eq!(first.stayed.len(), 0);
        assert_eq!(first.left.len(), 0);

        let second = world.move_focus(SectionCoord::new(1, 0, 0));
        assert_eq!(second.entered.len(), 3);
        assert_eq!(second.stayed.len(), 6);
        assert_eq!(second.left.len(), 3);
        assert_eq!(world.resident_len(), 9);
    }

    #[test]
    fn simulation_residency_is_smaller_than_render_residency() {
        let config = ResidencyConfig::new(2, 1, 1, 0);
        let mut world = VoxelWorld::new(Empty, config);
        world.move_focus(SectionCoord::new(0, 0, 0));
        assert_eq!(world.resident_len(), 75);
        assert!(world.is_simulating(SectionCoord::new(1, 0, 1)));
        assert!(!world.is_simulating(SectionCoord::new(2, 0, 0)));
        assert!(!world.is_simulating(SectionCoord::new(0, 1, 0)));
    }

    #[test]
    fn edits_survive_unload_and_reload() {
        let config = ResidencyConfig::new(0, 0, 0, 0);
        let mut world = VoxelWorld::new(Empty, config);
        let origin = VoxelCoord::new(2, 3, 4);
        world.move_focus(origin.section());
        assert!(world.set_voxel(origin, VoxelId::new(9)));

        world.move_focus(SectionCoord::new(20, 0, 0));
        assert_eq!(world.voxel(origin), VoxelId::new(9));
        world.move_focus(origin.section());
        assert_eq!(
            world
                .resident(origin.section())
                .unwrap()
                .get(origin.local()),
            VoxelId::new(9)
        );
    }

    #[test]
    fn entering_sections_queue_mesh_work_once() {
        let config = ResidencyConfig::new(1, 0, 0, 0);
        let mut world = VoxelWorld::new(Empty, config);
        world.move_focus(SectionCoord::new(0, 0, 0));
        assert_eq!(world.take_mesh_work().len(), 9);

        world.move_focus(SectionCoord::new(0, 0, 0));
        assert!(world.take_mesh_work().is_empty());

        world.move_focus(SectionCoord::new(1, 0, 0));
        assert_eq!(world.take_mesh_work().len(), 3);
    }

    #[test]
    fn boundary_edit_queues_resident_neighbour_for_remesh() {
        let config = ResidencyConfig::new(1, 0, 0, 0);
        let mut world = VoxelWorld::new(Empty, config);
        world.move_focus(SectionCoord::new(0, 0, 0));
        world.take_mesh_work();

        world.set_voxel(VoxelCoord::new(15, 4, 5), VoxelId::new(1));
        assert_eq!(
            world.take_mesh_work(),
            vec![SectionCoord::new(0, 0, 0), SectionCoord::new(1, 0, 0)]
        );
    }

    #[test]
    fn boundary_edits_dirty_the_neighbouring_section() {
        let config = ResidencyConfig::new(0, 0, 0, 0);
        let mut world = VoxelWorld::new(Empty, config);
        world.move_focus(SectionCoord::new(0, 0, 0));
        world.set_voxel(VoxelCoord::new(15, 4, 5), VoxelId::new(1));
        assert_eq!(
            world.take_dirty(),
            vec![SectionCoord::new(0, 0, 0), SectionCoord::new(1, 0, 0)]
        );
    }
}
