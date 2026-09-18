//! SDF de cuevas: cheese + spaghetti + noodle + acuíferos.

use super::noise::{fbm_3d, value_noise_3d};
use rivaren_core::Seed;

/// SDF de cuevas. Positivo = aire de cueva (come terreno en unión max()).
pub fn caves_sdf(seed: Seed, x: f32, y: f32, z: f32) -> f32 {
    // Sin cuevas sobre el cielo ni bajo el manto profundo.
    if y > 200.0 || y < -280.0 {
        return -1000.0; // no cueva
    }
    // Cheese: cámaras grandes, baja frecuencia.
    let cheese = fbm_3d(seed ^ 0xCE35E, x / 128.0, y / 96.0, z / 128.0, 3);
    // Spaghetti: intersección de dos ruidos ortogonales → túneles.
    let s1 = value_noise_3d(seed ^ 0x5FA6, x / 96.0, y / 96.0, z / 96.0);
    let s2 = value_noise_3d(seed ^ 0x6E77, z / 96.0, y / 96.0, x / 96.0);
    // Noodle: alta frecuencia, túneles finos.
    let noodle = value_noise_3d(seed ^ 0xB00D, x / 32.0, y / 32.0, z / 32.0).abs();

    // Umbrales → SDF aproximado (positivo = aire).
    let mut cave: f32 = -1000.0;
    // Cheese: cheese > 0.55 → cueva.
    cave = cave.max((cheese - 0.55) * 80.0);
    // Spaghetti: ambos cerca de 0 → túnel.
    let spaghetti = (s1.abs() + s2.abs()) * 0.5;
    cave = cave.max((0.08 - spaghetti) * 120.0);
    // Noodle solo en profundidad (y<0): evita picar la superficie.
    if y < 0.0 {
        cave = cave.max((0.05 - noodle) * 90.0);
    }
    // Acuíferos: bajo y<8, ruido de agua llena la cueva (se maneja como fluido,
    // aquí solo evitamos cuevas de aire bajo el nivel freático salvo cheese grande).
    let aquifer = fbm_3d(seed ^ 0xA901, x / 256.0, 0.0, z / 256.0, 2);
    if y < 8.0 && aquifer < 0.1 && cheese < 0.7 {
        cave = cave.min(-10.0); // inundada → no aire (el fluido la llena)
    }
    cave
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn deterministic() {
        assert_eq!(caves_sdf(3, 1.0, 2.0, 3.0), caves_sdf(3, 1.0, 2.0, 3.0));
    }
    #[test]
    fn no_caves_in_sky() {
        assert!(caves_sdf(3, 0.0, 500.0, 0.0) < 0.0);
    }
}
