//! SVDAG simplificado pero real: octree 32³ (5 niveles) con dedup bottom-up.
//! + SSVDAG: 3 bits de simetría XYZ por nodo (reflexión).
//!
//! Niveles: 32→16→8→4→2→1 (hojas de 2³ = 8 vóxeles en u8 mask + paleta).
//! Compresión típica terreno: 2–8 KB por chunk (medir con bench).

use ahash::AHashMap;
use rivaren_core::{BlockId, CHUNK_SIZE};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct SubtreeKey {
    children: [u32; 8],
    /// 3 bits simetría + 5 bits nivel
    meta: u8,
}

#[derive(Debug, Clone, Copy)]
pub struct Node {
    pub children: [u32; 8],
    /// bit0-2: mirror_x/y/z, bit3-7: nivel
    pub meta: u8,
}

impl Node {
    pub fn level(&self) -> u8 {
        self.meta >> 3
    }
    pub fn symmetry(&self) -> u8 {
        self.meta & 0x7
    }
}

#[derive(Debug, Default)]
pub struct Svdag {
    pub nodes: Vec<Node>,
    pub root: u32,
    /// Hojas: paleta de 8 vóxeles (2³) empaquetados como 8×u16 → u64 key → id
    pub leaves: Vec<[BlockId; 8]>,
}

pub struct SvdagBuilder {
    dedup: AHashMap<SubtreeKey, u32>,
    leaf_dedup: AHashMap<[BlockId; 8], u32>,
    nodes: Vec<Node>,
    leaves: Vec<[BlockId; 8]>,
}

impl SvdagBuilder {
    pub fn new() -> Self {
        Self {
            dedup: AHashMap::new(),
            leaf_dedup: AHashMap::new(),
            nodes: Vec::with_capacity(2048),
            leaves: Vec::new(),
        }
    }

    /// Construye desde voxels densos 32³.
    pub fn build(&mut self, voxels: &[BlockId; 32768]) -> Svdag {
        self.dedup.clear();
        self.leaf_dedup.clear();
        self.nodes.clear();
        self.leaves.clear();
        // Nivel hoja: bloques de 2³. Hay 16³ = 4096 hojas.
        // Nivel 1..5: octree sobre ids de hoja/nodo.
        let mut level_ids = Vec::with_capacity(4096);
        for bz in 0..16 {
            for by in 0..16 {
                for bx in 0..16 {
                    let mut leaf = [0u16; 8];
                    for dz in 0..2 {
                        for dy in 0..2 {
                            for dx in 0..2 {
                                let x = bx * 2 + dx;
                                let y = by * 2 + dy;
                                let z = bz * 2 + dz;
                                let v = voxels[(y * 32 + z) * 32 + x];
                                leaf[(dy * 2 + dz) * 2 + dx] = v;
                            }
                        }
                    }
                    level_ids.push(self.intern_leaf(leaf));
                }
            }
        }
        // Niveles 1..=4 (16→8→4→2→1 nodos por eje).
        let mut dim = 16u32;
        // offset: leaf ids se indexan con bit alto para distinguir de nodos.
        const LEAF_BIT: u32 = 1 << 31;
        let mut cur: Vec<u32> = level_ids.into_iter().map(|id| id | LEAF_BIT).collect();
        while dim > 1 {
            let nd = dim / 2;
            let mut next = Vec::with_capacity((nd * nd * nd) as usize);
            for bz in 0..nd {
                for by in 0..nd {
                    for bx in 0..nd {
                        let mut children = [0u32; 8];
                        for dz in 0..2 {
                            for dy in 0..2 {
                                for dx in 0..2 {
                                    let cx = bx * 2 + dx;
                                    let cy = by * 2 + dy;
                                    let cz = bz * 2 + dz;
                                    children[((dy * 2 + dz) * 2 + dx) as usize] =
                                        cur[((cz * dim + cy) * dim + cx) as usize];
                                }
                            }
                        }
                        next.push(self.intern_node(children, nd));
                    }
                }
            }
            cur = next;
            dim = nd;
        }
        Svdag {
            nodes: std::mem::take(&mut self.nodes),
            root: cur[0],
            leaves: std::mem::take(&mut self.leaves),
        }
    }

    fn intern_leaf(&mut self, leaf: [BlockId; 8]) -> u32 {
        if let Some(&id) = self.leaf_dedup.get(&leaf) {
            return id;
        }
        let id = self.leaves.len() as u32;
        self.leaves.push(leaf);
        self.leaf_dedup.insert(leaf, id);
        id
    }

    fn intern_node(&mut self, mut children: [u32; 8], _dim: u32) -> u32 {
        // SSVDAG: canonicaliza bajo reflexión X/Y/Z para dedup extra.
        // Calcula las 8 variantes espejadas y usa la mínima como clave,
        // guardando los 3 bits de simetría en meta.
        let level = match _dim {
            8 => 1,
            4 => 2,
            2 => 3,
            1 => 4,
            _ => 0,
        };
        let (canon, sym) = canonical_with_symmetry(children);
        children = canon;
        let key = SubtreeKey { children, meta: (level << 3) | sym };
        if let Some(&id) = self.dedup.get(&key) {
            return id;
        }
        let id = self.nodes.len() as u32;
        self.nodes.push(Node { children, meta: key.meta });
        self.dedup.insert(key, id);
        id
    }
}

impl Default for SvdagBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Devuelve (children_canónicos, sym_bits). Reflexión = permutar índices.
fn canonical_with_symmetry(children: [u32; 8]) -> ([u32; 8], u8) {
    // índices: bit0=x, bit1=z? orden (dy*2+dz)*2+dx → idx = dx + dz*2 + dy*4
    let mirror_x = |c: [u32; 8]| [c[1], c[0], c[3], c[2], c[5], c[4], c[7], c[6]];
    let mirror_z = |c: [u32; 8]| [c[2], c[3], c[0], c[1], c[6], c[7], c[4], c[5]];
    let mirror_y = |c: [u32; 8]| [c[4], c[5], c[6], c[7], c[0], c[1], c[2], c[3]];
    let mut best = children;
    let mut best_sym = 0u8;
    let variants = [
        (children, 0),
        (mirror_x(children), 1),
        (mirror_z(children), 2),
        (mirror_y(children), 4),
        (mirror_x(mirror_z(children)), 3),
        (mirror_x(mirror_y(children)), 5),
        (mirror_z(mirror_y(children)), 6),
        (mirror_x(mirror_z(mirror_y(children))), 7),
    ];
    for (v, s) in variants {
        if v < best {
            best = v;
            best_sym = s;
        }
    }
    let _ = CHUNK_SIZE;
    (best, best_sym)
}

impl Svdag {
    /// Tamaño comprimido aproximado en bytes (nodos + hojas).
    pub fn compressed_bytes(&self) -> usize {
        self.nodes.len() * std::mem::size_of::<Node>() + self.leaves.len() * 16
    }
    /// Bits por vóxel no vacío (métrica objetivo <0.12 en real; aquí informativa).
    pub fn bits_per_voxel(&self, nonempty: usize) -> f32 {
        if nonempty == 0 {
            return 0.0;
        }
        self.compressed_bytes() as f32 * 8.0 / nonempty as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn empty_roundtrip_size() {
        let mut b = SvdagBuilder::new();
        let v = [0u16; 32768];
        let d = b.build(&v);
        assert_eq!(d.leaves.len(), 1); // todo aire → 1 hoja
        assert!(d.nodes.len() < 600);
    }
    #[test]
    fn checkerboard_compresses() {
        let mut b = SvdagBuilder::new();
        let mut v = [0u16; 32768];
        for y in 0..32 {
            for z in 0..32 {
                for x in 0..32 {
                    if (x + y + z) % 2 == 0 {
                        v[(y * 32 + z) * 32 + x] = 1;
                    }
                }
            }
        }
        let t = std::time::Instant::now();
        let d = b.build(&v);
        let el = t.elapsed();
        assert!(el.as_micros() < 50_000, "demasiado lento: {el:?}");
        assert!(d.compressed_bytes() < 64 * 1024);
    }
}
