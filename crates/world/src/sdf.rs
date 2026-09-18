//! SDF del terreno: distancia con signo a la superficie.
//! Negativo = dentro del terreno, positivo = aire.
//! Capas: continentalness (1/2048) + erosión (1/512) + picos/valles (1/256)
//!        + densidad (1/64, cuevas y overhangs). Spline entre capas.

use super::noise::{fbm_3d, value_noise_3d};
use rivaren_core::Seed;

/// Altura base del terreno en (x,z). Rango aprox [-64, 320].
pub fn surface_height(seed: Seed, x: f32, z: f32) -> f32 {
    // Continentalness: masas tierra/océano, escala enorme.
    let cont = fbm_3d(seed ^ 0xC071, x / 2048.0, 0.0, z / 2048.0, 3);
    // Erosión: suaviza o recorta.
    let eros = fbm_3d(seed ^ 0xE20510, x / 512.0, 7.3, z / 512.0, 3);
    // Picos y valles centrados en 0 (la base domina la altura media).
    let pv = fbm_3d(seed ^ 0x9EA5, x / 256.0, 13.7, z / 256.0, 4).abs() * 2.0 - 1.0;
    // Spline continental: océano profundo → costa → llanura → meseta → montaña.
    let base = spline_continental(cont);
    // Erosión modula amplitud de picos: erosión alta = terreno suave.
    let amp = 52.0 * (1.0 - eros * 0.45) + 8.0;
    base + 86.0 + pv * amp + eros * 18.0
}

#[inline(always)]
fn spline_continental(c: f32) -> f32 {
    // c en [-1,1] → altura base. El nivel del mar es 62.
    if c < -0.55 {
        -52.0 + (c + 1.0) * 44.0 // fosa abisal: -52..-32
    } else if c < -0.25 {
        -32.0 + (c + 0.55) * 90.0 // océano: -32..-5
    } else if c < 0.2 {
        -5.0 + (c + 0.25) * 190.0 // costa → llanura: -5..80
    } else if c < 0.6 {
        80.0 + (c - 0.2) * 150.0 // colinas → meseta: 80..140
    } else {
        140.0 + (c - 0.6) * 260.0 // montañas: 140..244
    }
}

/// SDF puro del terreno (sin cuevas). Determinista.
#[inline(always)]
pub fn terrain_sdf(seed: Seed, x: f32, y: f32, z: f32) -> f32 {
    let h = surface_height(seed, x, z);
    // Densidad 3D de detalle: overhangs y variación vertical.
    let detail = value_noise_3d(seed ^ 0xDE7A11, x / 64.0, y / 64.0, z / 64.0) * 6.0;
    // SDF = -(profundidad bajo superficie). y<h → negativo (sólido).
    (y - h) + detail * density_falloff(y, h)
}

#[inline(always)]
fn density_falloff(y: f32, h: f32) -> f32 {
    // El detalle 3D pesa más cerca de la superficie, menos en profundidad
    // (evita islas flotantes profundas).
    let d = (y - h).abs();
    (1.0 - (d / 48.0).clamp(0.0, 1.0)).max(0.15)
}

/// SDF combinado terreno + cuevas. Aire si > 0.
#[inline(always)]
pub fn world_sdf(seed: Seed, x: f32, y: f32, z: f32) -> f32 {
    let t = terrain_sdf(seed, x, y, z);
    // Solo evalúa cuevas bajo superficie (ahorra ~40% CPU).
    if t > 24.0 {
        return t;
    }
    let c = super::caves::caves_sdf(seed, x, y, z);
    // Unión SDF: max(t, c) — la cueva "come" terreno donde c>0... convención:
    // terreno sólido (t<0) + cueva aire (c>0) → aire. max() lo expresa.
    t.max(c)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn solid_deep_underground() {
        assert!(terrain_sdf(1, 0.0, -200.0, 0.0) < 0.0);
    }
    #[test]
    fn air_high_up() {
        assert!(terrain_sdf(1, 0.0, 600.0, 0.0) > 0.0);
    }
    #[test]
    fn deterministic() {
        assert_eq!(world_sdf(5, 10.0, 20.0, 30.0), world_sdf(5, 10.0, 20.0, 30.0));
    }
}
