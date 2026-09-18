//! Ventana deslizante infinita 512³: origin + shift sin re-subir todo.
//! En CPU se calcula el delta + regiones dirty; en GPU un compute shader
//! mueve los datos (origin-shift) y solo se suben los chunks del borde.
//!
//! 512³ / 32³ = 16×16×16 = 4096 chunks direccionables en GPU.

use glam::IVec3;

pub const CHUNKS_PER_AXIS: i32 = 16;

#[derive(Debug)]
pub struct SlidingWindow {
    pub origin: IVec3, // origen en vóxeles, alineado a 32
    pub size: i32,
}

#[derive(Debug, Default)]
pub struct WindowDelta {
    /// Chunks a generar (borde entrante) en coords de chunk.
    pub to_generate: Vec<IVec3>,
    /// Chunks a descartar (borde saliente).
    pub to_evict: Vec<IVec3>,
    /// Shift en vóxeles (para el compute shader de la GPU).
    pub shift_voxels: IVec3,
}

impl SlidingWindow {
    pub fn new(origin: IVec3) -> Self {
        Self { origin: align_chunk(origin), size: rivaren_core::WINDOW_SIZE }
    }
    /// Mueve la ventana. Retorna el delta con listas de chunks.
    /// `player_chunk` = chunk donde está el jugador.
    pub fn shift(&mut self, player_chunk: IVec3) -> WindowDelta {
        // La ventana se centra en el jugador: origen = player*32 - 256.
        let want = IVec3::new(
            player_chunk.x * 32 - 256,
            0, // Y fijo en superficie por ahora (Fase 6: sigue al jugador en Y)
            player_chunk.z * 32 - 256,
        );
        let want = align_chunk(want);
        let shift = want - self.origin;
        if shift == IVec3::ZERO {
            return WindowDelta::default();
        }
        // Calcula borde: chunks nuevos = los que están en la ventana NUEVA
        // pero no estaban en la VIEJA.
        let mut delta = WindowDelta { shift_voxels: shift, ..Default::default() };
        let old_cx = self.origin.x / 32;
        let old_cz = self.origin.z / 32;
        let new_cx = want.x / 32;
        let new_cz = want.z / 32;
        for cz in 0..CHUNKS_PER_AXIS {
            for cx in 0..CHUNKS_PER_AXIS {
                let nc = IVec3::new(new_cx + cx, 0, new_cz + cz);
                let was_inside = nc.x >= old_cx
                    && nc.x < old_cx + CHUNKS_PER_AXIS
                    && nc.z >= old_cz
                    && nc.z < old_cz + CHUNKS_PER_AXIS;
                if !was_inside {
                    delta.to_generate.push(nc);
                }
            }
        }
        // Evict: estaban en la vieja pero no están en la nueva.
        for cz in 0..CHUNKS_PER_AXIS {
            for cx in 0..CHUNKS_PER_AXIS {
                let oc = IVec3::new(old_cx + cx, 0, old_cz + cz);
                let still_inside = oc.x >= new_cx
                    && oc.x < new_cx + CHUNKS_PER_AXIS
                    && oc.z >= new_cz
                    && oc.z < new_cz + CHUNKS_PER_AXIS;
                if !still_inside {
                    delta.to_evict.push(oc);
                }
            }
        }
        self.origin = want;
        delta
    }
    pub fn contains(&self, p: IVec3) -> bool {
        let d = p - self.origin;
        d.x >= 0 && d.y >= 0 && d.z >= 0 && d.x < self.size && d.y < self.size && d.z < self.size
    }
}

fn align_chunk(v: IVec3) -> IVec3 {
    IVec3::new(v.x.div_euclid(32) * 32, v.y.div_euclid(32) * 32, v.z.div_euclid(32) * 32)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn no_shift_no_work() {
        let mut w = SlidingWindow::new(IVec3::new(0, 0, 0));
        let d = w.shift(IVec3::new(8, 0, 8)); // centro de la ventana inicial
        assert!(d.to_generate.is_empty());
    }
    #[test]
    fn shift_one_chunk() {
        let mut w = SlidingWindow::new(IVec3::new(0, 0, 0));
        // jugador en chunk (9,0,8) → ventana quiere origen x=32 → shift +32
        let d = w.shift(IVec3::new(9, 0, 8));
        assert_eq!(d.shift_voxels, IVec3::new(32, 0, 0));
        assert_eq!(d.to_generate.len(), 16); // una fila de 16 chunks
        assert_eq!(d.to_evict.len(), 16);
    }
}
