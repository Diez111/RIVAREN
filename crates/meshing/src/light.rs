//! Horneado de luz por chunk: sky light + block light (BFS).
//! Resultado: `[u8; 32768]` con sky:4 | block:4 por vóxel.
//! Coste típico: < 0.2 ms (columnas + BFS corto).

use rivaren_core::{AIR, BlockId, CHUNK_SIZE};
use std::collections::VecDeque;

const VOL: usize = 32768;

#[inline(always)]
fn idx(x: usize, y: usize, z: usize) -> usize {
    (y * CHUNK_SIZE + z) * CHUNK_SIZE + x
}

#[inline(always)]
fn is_transparent(v: BlockId) -> bool {
    v == AIR || v == 20 // aire + agua
}

/// Emisivos: id → nivel (0..15).
#[inline(always)]
fn emissive(v: BlockId) -> u8 {
    match v {
        30 => 12, // marco de portal
        21 => 5,  // losa de plaza (brillo tenue)
        _ => 0,
    }
}

pub fn bake_lighting(voxels: &[BlockId; VOL]) -> [u8; VOL] {
    let mut light = [0u8; VOL];
    // 1. Sky light por columnas: 15 en aire hasta el primer opaco;
    //    bajo el opaco decae 4 por bloque (cuevas oscuras).
    for z in 0..CHUNK_SIZE {
        for x in 0..CHUNK_SIZE {
            let mut blocked_depth: Option<i32> = None;
            for y in (0..CHUNK_SIZE).rev() {
                let v = voxels[idx(x, y, z)];
                if is_transparent(v) {
                    let sky = match blocked_depth {
                        None => 15u8,
                        Some(d) => (15i32 - d * 4).max(0) as u8,
                    };
                    light[idx(x, y, z)] = sky;
                } else if blocked_depth.is_none() {
                    blocked_depth = Some(1);
                } else if let Some(d) = &mut blocked_depth {
                    *d += 1;
                }
            }
        }
    }
    // 2. Block light: BFS multi-fuente.
    let mut queue: VecDeque<(u8, u8, u8, u8)> = VecDeque::new();
    for y in 0..CHUNK_SIZE {
        for z in 0..CHUNK_SIZE {
            for x in 0..CHUNK_SIZE {
                let e = emissive(voxels[idx(x, y, z)]);
                if e > 0 {
                    light[idx(x, y, z)] = (light[idx(x, y, z)] & 0xF) | (e << 4);
                    queue.push_back((x as u8, y as u8, z as u8, e));
                }
            }
        }
    }
    const DIRS: [(i32, i32, i32); 6] = [
        (1, 0, 0),
        (-1, 0, 0),
        (0, 1, 0),
        (0, -1, 0),
        (0, 0, 1),
        (0, 0, -1),
    ];
    while let Some((x, y, z, lv)) = queue.pop_front() {
        if lv <= 1 {
            continue;
        }
        for (dx, dy, dz) in DIRS {
            let nx = x as i32 + dx;
            let ny = y as i32 + dy;
            let nz = z as i32 + dz;
            if nx < 0 || ny < 0 || nz < 0 || nx > 31 || ny > 31 || nz > 31 {
                continue;
            }
            let (nx, ny, nz) = (nx as usize, ny as usize, nz as usize);
            if !is_transparent(voxels[idx(nx, ny, nz)]) {
                continue;
            }
            let cur = (light[idx(nx, ny, nz)] >> 4) & 0xF;
            let nl = lv - 1;
            if cur >= nl {
                continue;
            }
            light[idx(nx, ny, nz)] = (light[idx(nx, ny, nz)] & 0xF) | (nl << 4);
            queue.push_back((nx as u8, ny as u8, nz as u8, nl));
        }
    }
    light
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn open_air_is_full_sky() {
        let v = [AIR; VOL];
        let l = bake_lighting(&v);
        assert_eq!(l[0] & 0xF, 15);
        assert_eq!((l[VOL - 1] >> 4) & 0xF, 0);
    }
    #[test]
    fn deep_solid_is_dark() {
        let v = [4u16; VOL];
        let l = bake_lighting(&v);
        assert_eq!(l[0] & 0xF, 0);
    }
    #[test]
    fn emissive_spreads() {
        let mut v = [AIR; VOL];
        v[idx(16, 16, 16)] = 30;
        let l = bake_lighting(&v);
        assert_eq!((l[idx(16, 16, 16)] >> 4) & 0xF, 12);
        assert_eq!((l[idx(17, 16, 16)] >> 4) & 0xF, 11);
    }
}
