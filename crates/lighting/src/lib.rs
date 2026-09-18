//! `rivaren-lighting`: sombras híbridas + GI temporal + propagación + cielo.
//!
//! Tiers:
//! - Potato: sin GI, AO horneado + 1 cascada 1024.
//! - Mobile: 2 rayos/px a 1/4 res, SH difuso, acum 8f, denoise edge-aware.
//! - Balanced/Ultra: 4-8 rayos, ReSTIR 8×8, TAA 32f (NAADF 2026).

use glam::Vec3;

/// Luz puntual de bloque (antorcha, cristal...). Propagación BFS radio 16.
#[derive(Debug, Clone, Copy)]
pub struct BlockLight {
    pub pos: [i32; 3],
    pub color: [u8; 3],
    pub level: u8, // 0..15
}

/// Sol direccional: hora del día 0..1 (0=amanecer, 0.5=mediodía).
pub fn sun_direction(time_of_day: f32) -> Vec3 {
    let a = time_of_day * std::f32::consts::TAU;
    Vec3::new(a.cos(), a.sin().max(-0.2), 0.35).normalize()
}

/// Atenuación cielo por profundidad (horneada en meshing, interpolada en shader).
pub fn sky_light_at_depth(depth: f32) -> u8 {
    ((15.0 * (-depth / 64.0).exp()).clamp(0.0, 15.0)) as u8
}

/// Sonda de GI (screen-probe 16×16, 64 direcciones como SmartGI/Lumen-lite).
#[derive(Debug, Clone, Copy, Default)]
pub struct ScreenProbe {
    pub radiance: [f32; 3],
    pub normal: [f32; 3],
    pub depth: f32,
}

/// Propagación de luz de bloque: BFS limitada a radio 16, amortizable.
/// `get_opacity` retorna 0 (aire) .. 15 (opaco). `set_light` escribe nivel.
/// Retorna nº de celdas actualizadas (para presupuestar 0.1-0.5ms/cambio).
pub fn propagate_block_light(
    origin: [i32; 3],
    level: u8,
    radius: i32,
    mut get_opacity: impl FnMut([i32; 3]) -> u8,
    mut set_light: impl FnMut([i32; 3], u8),
) -> usize {
    use std::collections::{HashMap, VecDeque};
    let mut queue = VecDeque::new();
    let mut best: HashMap<[i32; 3], u8> = HashMap::new();
    let mut updated = 0;
    queue.push_back((origin, level));
    best.insert(origin, level);
    set_light(origin, level);
    updated += 1;
    while let Some((pos, lv)) = queue.pop_front() {
        if lv <= 1 {
            continue;
        }
        for (dx, dy, dz) in [(1, 0, 0), (-1, 0, 0), (0, 1, 0), (0, -1, 0), (0, 0, 1), (0, 0, -1)] {
            let np = [pos[0] + dx, pos[1] + dy, pos[2] + dz];
            if (np[0] - origin[0]).abs() > radius
                || (np[1] - origin[1]).abs() > radius
                || (np[2] - origin[2]).abs() > radius
            {
                continue;
            }
            let opacity = get_opacity(np);
            if opacity >= 15 {
                continue;
            }
            let nl = lv.saturating_sub(1 + opacity);
            if nl == 0 {
                continue;
            }
            // Solo propaga si mejora lo ya conocido (evita re-visitas que
            // degradarían el origen a través de caminos largos).
            if best.get(&np).map(|&b| b >= nl).unwrap_or(false) {
                continue;
            }
            best.insert(np, nl);
            // Solo propaga si mejora (el llamador puede pre-chequear con get).
            set_light(np, nl);
            updated += 1;
            // Cap de seguridad: 16³ = 4096 celdas máx por cambio.
            if updated >= 4096 {
                return updated;
            }
            queue.push_back((np, nl));
        }
    }
    updated
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sun_moves() {
        assert_ne!(sun_direction(0.0), sun_direction(0.25));
    }
    #[test]
    fn propagation_in_air() {
        let mut grid = std::collections::HashMap::new();
        let n = propagate_block_light(
            [0, 0, 0],
            15,
            2,
            |_| 0,
            |p, l| {
                grid.insert(p, l);
            },
        );
        assert!(n > 10);
        assert_eq!(grid[&[0, 0, 0]], 15);
        assert_eq!(grid[&[1, 0, 0]], 14);
    }
}
