//! Re-meshing incremental: solo el chunk dirty (+ vecinos si toca el borde).
//! Deadline 2 frames. El set vive en `app` y lo consume el JobSystem.

use glam::IVec3;
use std::collections::HashSet;

#[derive(Default)]
pub struct DirtySet {
    set: HashSet<IVec3>,
}

impl DirtySet {
    pub fn mark(&mut self, chunk: IVec3, local: (u8, u8, u8)) {
        self.set.insert(chunk);
        // Si el bloque toca el borde, marca vecinos.
        let (x, y, z) = local;
        if x == 0 {
            self.set.insert(chunk + IVec3::new(-1, 0, 0));
        }
        if x == 31 {
            self.set.insert(chunk + IVec3::new(1, 0, 0));
        }
        if y == 0 {
            self.set.insert(chunk + IVec3::new(0, -1, 0));
        }
        if y == 31 {
            self.set.insert(chunk + IVec3::new(0, 1, 0));
        }
        if z == 0 {
            self.set.insert(chunk + IVec3::new(0, 0, -1));
        }
        if z == 31 {
            self.set.insert(chunk + IVec3::new(0, 0, 1));
        }
    }
    pub fn drain(&mut self) -> Vec<IVec3> {
        self.set.drain().collect()
    }
    pub fn len(&self) -> usize {
        self.set.len()
    }
    pub fn is_empty(&self) -> bool {
        self.set.is_empty()
    }
}
