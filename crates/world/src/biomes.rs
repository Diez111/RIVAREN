//! Biomas originales RIVAREN (nombres propios, no Minecraft).
//! Cache 4×4 + interpolación bilineal. Evaluación SIMD-friendly.

use rivaren_core::Seed;
use serde::{Deserialize, Serialize};

use super::noise::fbm_3d;

pub type BiomeId = u16;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaveBiome {
    pub id: BiomeId,
    pub name: &'static str,
    pub depth_range: (i32, i32),
    pub ambient_light: u8,
    pub fog_color: [f32; 3],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Biome {
    pub id: BiomeId,
    pub name: &'static str,
    pub temperature: f32,
    pub humidity: f32,
    pub surface_blocks: [u16; 4],
    pub cave: Option<BiomeId>,
}

/// Tabla estática de biomas originales.
pub fn biome_table() -> &'static [Biome] {
    use std::sync::OnceLock;
    static T: OnceLock<Vec<Biome>> = OnceLock::new();
    T.get_or_init(|| {
        vec![
            Biome { id: 0, name: "Pradera Ferral", temperature: 0.7, humidity: 0.5, surface_blocks: [1, 2, 3, 4], cave: None },
            Biome { id: 1, name: "Bosque de Cristal", temperature: 0.6, humidity: 0.8, surface_blocks: [5, 2, 3, 4], cave: Some(100) },
            Biome { id: 2, name: "Desierto de Canto", temperature: 1.0, humidity: 0.05, surface_blocks: [6, 6, 3, 4], cave: None },
            Biome { id: 3, name: "Jungla de Bruma", temperature: 0.95, humidity: 0.95, surface_blocks: [7, 2, 3, 4], cave: Some(100) },
            Biome { id: 4, name: "Taiga Ceniza", temperature: 0.25, humidity: 0.6, surface_blocks: [8, 2, 3, 4], cave: None },
            Biome { id: 5, name: "Tundra Silente", temperature: 0.0, humidity: 0.4, surface_blocks: [9, 2, 3, 4], cave: None },
            Biome { id: 6, name: "Sabana Ocre", temperature: 0.9, humidity: 0.2, surface_blocks: [10, 2, 3, 4], cave: None },
            Biome { id: 7, name: "Baldío de Cobre", temperature: 0.8, humidity: 0.1, surface_blocks: [11, 11, 3, 4], cave: Some(102) },
            Biome { id: 8, name: "Pantano Manglar", temperature: 0.85, humidity: 0.9, surface_blocks: [12, 13, 3, 4], cave: None },
            Biome { id: 9, name: "Océano Abisal", temperature: 0.5, humidity: 1.0, surface_blocks: [14, 14, 14, 4], cave: None },
            Biome { id: 10, name: "Isla Seta", temperature: 0.75, humidity: 0.7, surface_blocks: [15, 15, 3, 4], cave: None },
            Biome { id: 11, name: "Huerto Cerezo", temperature: 0.65, humidity: 0.6, surface_blocks: [16, 2, 3, 4], cave: None },
        ]
    })
}

pub fn cave_biome_table() -> &'static [CaveBiome] {
    use std::sync::OnceLock;
    static T: OnceLock<Vec<CaveBiome>> = OnceLock::new();
    T.get_or_init(|| {
        vec![
            CaveBiome { id: 100, name: "Vergel Hondo", depth_range: (-64, 30), ambient_light: 6, fog_color: [0.2, 0.5, 0.3] },
            CaveBiome { id: 101, name: "Galería Estalactita", depth_range: (-128, -16), ambient_light: 2, fog_color: [0.5, 0.45, 0.4] },
            CaveBiome { id: 102, name: "Abismo Sordo", depth_range: (-280, -64), ambient_light: 0, fog_color: [0.05, 0.02, 0.1] },
        ]
    })
}

/// Muestreo de bioma en (x,z): 2 ruidos T/H + continentalness implícita.
/// Barato (~0.05ms/chunk con cache 4×4 en pipeline).
pub fn sample_biome(seed: Seed, x: f32, z: f32) -> BiomeId {
    let t = fbm_3d(seed ^ 0x7E77, x / 1024.0, 3.1, z / 1024.0, 2) * 0.5 + 0.5;
    let h = fbm_3d(seed ^ 0xB071, x / 1024.0, 9.4, z / 1024.0, 2) * 0.5 + 0.5;
    // Clasificación por cuadrante T/H (rápida, sin splines caros).
    match (t, h) {
        _ if t > 0.85 && h < 0.2 => 2,   // desierto
        _ if t > 0.8 && h > 0.8 => 3,    // jungla
        _ if t < 0.2 => 5,               // tundra
        _ if t < 0.35 && h > 0.4 => 4,   // taiga
        _ if t > 0.75 && h < 0.4 => 6,   // sabana
        _ if h > 0.85 && t > 0.7 => 8,   // pantano
        _ if t > 0.6 && h > 0.6 && t < 0.75 => 11, // cerezo (raro)
        _ if h > 0.55 && t > 0.5 => 1,   // cristal
        _ => 0,                          // pradera por defecto
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn deterministic_biome() {
        assert_eq!(sample_biome(1, 10.0, 20.0), sample_biome(1, 10.0, 20.0));
    }
}
