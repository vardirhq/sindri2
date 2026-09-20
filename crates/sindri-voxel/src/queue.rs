use std::collections::BTreeSet;

use crate::SectionCoord;

/// Deterministic section work queue.
///
/// The queue is deliberately CPU/GPU agnostic. A later worker pool may drain
/// generation and meshing jobs asynchronously without changing world residency
/// semantics. `BTreeSet` also deduplicates repeated dirty notifications.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct VoxelWorkQueue {
    generation: BTreeSet<SectionCoord>,
    meshing: BTreeSet<SectionCoord>,
}

impl VoxelWorkQueue {
    pub fn queue_generation(&mut self, coord: SectionCoord) {
        self.generation.insert(coord);
    }

    pub fn queue_mesh(&mut self, coord: SectionCoord) {
        self.meshing.insert(coord);
    }

    #[must_use]
    pub fn generation_len(&self) -> usize {
        self.generation.len()
    }

    #[must_use]
    pub fn mesh_len(&self) -> usize {
        self.meshing.len()
    }

    pub fn take_generation(&mut self) -> Vec<SectionCoord> {
        std::mem::take(&mut self.generation).into_iter().collect()
    }

    pub fn take_meshes(&mut self) -> Vec<SectionCoord> {
        std::mem::take(&mut self.meshing).into_iter().collect()
    }

    pub fn forget(&mut self, coord: SectionCoord) {
        self.generation.remove(&coord);
        self.meshing.remove(&coord);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn work_is_deduplicated_and_deterministic() {
        let mut queue = VoxelWorkQueue::default();
        let a = SectionCoord::new(2, 0, 0);
        let b = SectionCoord::new(-1, 0, 3);
        queue.queue_mesh(a);
        queue.queue_mesh(b);
        queue.queue_mesh(a);
        assert_eq!(queue.mesh_len(), 2);
        assert_eq!(queue.take_meshes(), vec![b, a]);
        assert_eq!(queue.mesh_len(), 0);
    }

    #[test]
    fn unloading_can_forget_stale_work() {
        let mut queue = VoxelWorkQueue::default();
        let coord = SectionCoord::new(4, -2, 8);
        queue.queue_generation(coord);
        queue.queue_mesh(coord);
        queue.forget(coord);
        assert!(queue.take_generation().is_empty());
        assert!(queue.take_meshes().is_empty());
    }
}
