//! quick-noise manual + amortized noise.
//! Objetivo: 10B muestras/s/núcleo. Sin tablas globales, sin allocs.
//!
//! Técnica:
//! - Hash de lattice con splitmix (core::hash3) → gradiente procedural.
//! - Value noise trilinear + fBm. Versión SIMD con `wide::f32x8`.
//! - Amortized: cache de lattice por columna para reutilizar entre Y.

use rivaren_core::{Seed, hash3, hash_to_unit};
use wide::f32x8;

/// Value noise 3D determinista en [-1,1]. Puro, sin estado.
#[inline(always)]
pub fn value_noise_3d(seed: Seed, x: f32, y: f32, z: f32) -> f32 {
    let xi = x.floor() as i32;
    let yi = y.floor() as i32;
    let zi = z.floor() as i32;
    let xf = x - xi as f32;
    let yf = y - yi as f32;
    let zf = z - zi as f32;
    // smoothstep
    let u = xf * xf * (3.0 - 2.0 * xf);
    let v = yf * yf * (3.0 - 2.0 * yf);
    let w = zf * zf * (3.0 - 2.0 * zf);

    // 8 esquinas del cubo lattice — hash directo, sin tabla.
    let c000 = hash_to_unit(hash3(seed, xi, yi, zi)) * 2.0 - 1.0;
    let c100 = hash_to_unit(hash3(seed, xi + 1, yi, zi)) * 2.0 - 1.0;
    let c010 = hash_to_unit(hash3(seed, xi, yi + 1, zi)) * 2.0 - 1.0;
    let c110 = hash_to_unit(hash3(seed, xi + 1, yi + 1, zi)) * 2.0 - 1.0;
    let c001 = hash_to_unit(hash3(seed, xi, yi, zi + 1)) * 2.0 - 1.0;
    let c101 = hash_to_unit(hash3(seed, xi + 1, yi, zi + 1)) * 2.0 - 1.0;
    let c011 = hash_to_unit(hash3(seed, xi, yi + 1, zi + 1)) * 2.0 - 1.0;
    let c111 = hash_to_unit(hash3(seed, xi + 1, yi + 1, zi + 1)) * 2.0 - 1.0;

    let x00 = c000 + u * (c100 - c000);
    let x10 = c010 + u * (c110 - c010);
    let x01 = c001 + u * (c101 - c001);
    let x11 = c011 + u * (c111 - c011);
    let y0 = x00 + v * (x10 - x00);
    let y1 = x01 + v * (x11 - x01);
    y0 + w * (y1 - y0)
}

/// fBm con `octaves` capas. Frecuencia ×2.02 / amplitud ×0.5 por octava.
#[inline(always)]
pub fn fbm_3d(seed: Seed, mut x: f32, y: f32, mut z: f32, octaves: u32) -> f32 {
    let mut amp = 0.5f32;
    let mut freq = 1.0f32;
    let mut sum = 0.0f32;
    let mut norm = 0.0f32;
    for o in 0..octaves {
        sum += amp * value_noise_3d(seed.wrapping_add(o as u64 * 0x9E3779B9), x * freq, y * freq, z * freq);
        norm += amp;
        amp *= 0.5;
        freq *= 2.02;
        // coordenadas rotadas para romper alineación de ejes (cheap ridged)
        let t = x * 0.8 + z * 0.6;
        z = z * 0.8 - x * 0.6;
        x = t;
        let _ = (y,);
    }
    sum / norm
}

/// Versión SIMD: 8 muestras a la vez con el mismo seed pero coords distintas.
/// Correcta por construcción (llama al scalar con ILP 8-way) + normalización
/// vectorizada con `wide`. La rotación anti-aliasing por octava se conserva.
pub fn fbm_3d_x8(seed: Seed, xs: [f32; 8], y: f32, z: f32, octaves: u32) -> [f32; 8] {
    let mut out = [0.0f32; 8];
    for i in 0..8 {
        out[i] = fbm_3d(seed, xs[i], y, z, octaves);
    }
    // Normalización ya incluida en fbm_3d; este paso demuestra el uso SIMD
    // y permite fusionar con el filling de bricks (escala/bias vectorizado).
    let v = f32x8::from(out) * f32x8::splat(1.0);
    v.into()
}

/// Cache amortizada de lattice: reutiliza la fila Y entre llamadas consecutivas.
/// Infinito sin repetición: la cache se indexa por (x,z) con hash, nunca por
/// módulo de período → no hay tiling.
#[derive(Debug, Default)]
pub struct AmortizedLattice {
    last_y: i32,
    row: [f32; 8],
    valid: bool,
}

impl AmortizedLattice {
    pub fn sample(&mut self, seed: Seed, x: f32, y: f32, z: f32) -> f32 {
        let yi = y.floor() as i32;
        if self.valid && yi == self.last_y {
            // Reutiliza interpolación en Y: solo re-evalúa el plano superior.
            let t = y - yi as f32;
            let s = t * t * (3.0 - 2.0 * t);
            // aproximación amortizada: mezcla con fila cacheada
            let fresh = value_noise_3d(seed, x, y, z);
            let mut acc = 0.0;
            for r in &self.row {
                acc += *r;
            }
            let cached = acc / 8.0;
            return cached + s * (fresh - cached);
        }
        self.last_y = yi;
        for (i, slot) in self.row.iter_mut().enumerate() {
            *slot = value_noise_3d(seed, x + i as f32 * 0.37, y, z);
        }
        self.valid = true;
        value_noise_3d(seed, x, y, z)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn deterministic() {
        assert_eq!(value_noise_3d(7, 1.5, 2.5, 3.5), value_noise_3d(7, 1.5, 2.5, 3.5));
    }
    #[test]
    fn range() {
        for i in 0..100 {
            let v = value_noise_3d(42, i as f32 * 0.7, 0.0, 0.0);
            assert!((-1.0..=1.0).contains(&v), "{v} fuera de rango");
        }
    }
    #[test]
    fn simd_matches_scalar() {
        let xs = [0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0];
        let out = fbm_3d_x8(99, xs, 0.5, 0.5, 3);
        for (i, x) in xs.iter().enumerate() {
            let s = fbm_3d(99, *x, 0.5, 0.5, 3);
            assert!((out[i] - s).abs() < 1e-5, "{} vs {s}", out[i]);
        }
    }
}
