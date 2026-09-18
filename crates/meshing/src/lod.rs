//! LOD: selección por distancia + downsampling 2³→1.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LodLevel {
    L0,
    L1,
    L2,
    L3,
    L4,
}

pub fn select_lod(dist_chunks: i32) -> LodLevel {
    match dist_chunks {
        0..=4 => LodLevel::L0,
        5..=12 => LodLevel::L1,
        13..=20 => LodLevel::L2,
        21..=30 => LodLevel::L3,
        _ => LodLevel::L4,
    }
}

/// Downsample 32³ → 16³ (2×2×2 → 1, vota sólido mayoritario).
pub fn downsample_2x(src: &[u16; 32768], dst: &mut [u16; 4096]) {
    for z in 0..16 {
        for y in 0..16 {
            for x in 0..16 {
                let mut votes = [0u8; 2];
                for dz in 0..2 {
                    for dy in 0..2 {
                        for dx in 0..2 {
                            let v = src[((y * 2 + dy) * 32 + (z * 2 + dz)) * 32 + (x * 2 + dx)];
                            votes[(v != 0) as usize] += 1;
                        }
                    }
                }
                // Representante: primer sólido encontrado (barato y estable).
                let mut rep = 0u16;
                if votes[1] > 0 {
                    'find: for dz in 0..2 {
                        for dy in 0..2 {
                            for dx in 0..2 {
                                let v = src[((y * 2 + dy) * 32 + (z * 2 + dz)) * 32 + (x * 2 + dx)];
                                if v != 0 {
                                    rep = v;
                                    break 'find;
                                }
                            }
                        }
                    }
                }
                dst[(y * 16 + z) * 16 + x] = rep;
            }
        }
    }
}
