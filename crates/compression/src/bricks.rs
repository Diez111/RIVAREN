//! BrickMaps: hojas densas 8³ en pool deduplicado.
//! En GPU: Texture3D R8Uint/R16Uint para acceso O(1) a los vóxeles calientes.
//! En CPU: pool con hash FNV + upload por regiones dirty.

use ahash::AHashMap;
use rivaren_core::BlockId;

pub const BRICK_SIZE: usize = 8;
pub const BRICK_VOLUME: usize = 512;
/// Chunk 32³ = 4×4×4 = 64 bricks de 8³.
pub const BRICKS_PER_CHUNK: usize = 64;

#[derive(Default)]
pub struct BrickPool {
    map: AHashMap<u64, u32>,
    pub data: Vec<BlockId>,
}

impl BrickPool {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn intern(&mut self, brick: &[BlockId; 512]) -> u32 {
        let mut h: u64 = 0xcbf29ce484222325;
        for v in brick.iter() {
            h ^= *v as u64;
            h = h.wrapping_mul(0x100000001b3);
        }
        if let Some(&id) = self.map.get(&h) {
            // Colisión hash posible pero rarísima; verifica contenido.
            let base = id as usize * BRICK_VOLUME;
            if self.data[base..base + BRICK_VOLUME] == *brick {
                return id;
            }
        }
        let id = (self.data.len() / BRICK_VOLUME) as u32;
        self.data.extend_from_slice(brick);
        self.map.insert(h, id);
        id
    }
    /// Parte un chunk 32³ en 64 bricks + los interna. Retorna los 64 ids.
    /// Orden: (by*4+bz)*4+bx, con by = y/8, etc.
    pub fn intern_chunk(&mut self, voxels: &[BlockId; 32768]) -> [u32; 64] {
        let mut out = [0u32; 64];
        let mut brick = [0u16; 512];
        for by in 0..4 {
            for bz in 0..4 {
                for bx in 0..4 {
                    for dy in 0..8 {
                        for dz in 0..8 {
                            for dx in 0..8 {
                                let x = bx * 8 + dx;
                                let y = by * 8 + dy;
                                let z = bz * 8 + dz;
                                brick[(dy * 8 + dz) * 8 + dx] =
                                    voxels[(y * 32 + z) * 32 + x];
                            }
                        }
                    }
                    out[(by * 4 + bz) * 4 + bx] = self.intern(&brick);
                }
            }
        }
        out
    }
    pub fn brick_count(&self) -> usize {
        self.data.len() / BRICK_VOLUME
    }
    /// Tamaño GPU necesario para la textura 3D de bricks (R16Uint).
    pub fn gpu_bytes(&self) -> usize {
        self.data.len() * 2
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dedup_air() {
        let mut p = BrickPool::new();
        let ids = p.intern_chunk(&[0u16; 32768]);
        assert!(ids.iter().all(|&i| i == 0));
        assert_eq!(p.brick_count(), 1);
    }
}
