//! `rivaren-gameplay`: corazón de RIVAREN — karma 3 ejes, 3 dimensiones,
//! muerte/respawn, aldeas con historia, crafteo, clima, portales.
//! Todo original, sin nombres/assets de Minecraft.

use serde::{Deserialize, Serialize};

/// Los tres ejes del karma. Rango -100..100 cada uno.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct Karma {
    pub compasion: f32,
    pub justicia: f32,
    pub sabiduria: f32,
}

impl Karma {
    pub fn add(&mut self, axis: KarmaAxis, v: f32) {
        let slot = match axis {
            KarmaAxis::Compasion => &mut self.compasion,
            KarmaAxis::Justicia => &mut self.justicia,
            KarmaAxis::Sabiduria => &mut self.sabiduria,
        };
        let v = if *slot < 0.0 && v > 0.0 { v / 1.5 } else { v };
        *slot = (*slot + v).clamp(-100.0, 100.0);
    }
    pub fn aura_color(&self) -> [f32; 3] {
        let c = (self.compasion / 100.0).max(0.0);
        let j = (self.justicia / 100.0).max(0.0);
        let s = (self.sabiduria / 100.0).max(0.0);
        [0.15 + 0.85 * c, 0.15 + 0.6 * j, 0.15 + 0.85 * s]
    }
    pub fn death_destination(&self) -> Dimension {
        if self.compasion > 40.0 && self.justicia > 20.0 {
            Dimension::Cielo
        } else if self.compasion < -40.0 {
            Dimension::Infierno
        } else {
            Dimension::Tierra
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum KarmaAxis {
    Compasion,
    Justicia,
    Sabiduria,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Dimension {
    Tierra,
    Cielo,
    Infierno,
}

impl Dimension {
    /// Bloques de cada dimensión (paletas distintas, misma API).
    pub fn sky_color(&self) -> [f32; 3] {
        match self {
            Self::Tierra => [0.45, 0.65, 0.90],
            Self::Cielo => [0.85, 0.90, 1.0],
            Self::Infierno => [0.45, 0.10, 0.08],
        }
    }
}

/// Portal de Ascenso (→Cielo, costoso) / Descenso (→Infierno, sacrificio).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PortalKind {
    Ascension,
    Descenso,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Portal {
    pub kind: PortalKind,
    pub frame: [[i32; 3]; 4], // esquinas del marco
    pub lit: bool,
}

impl Portal {
    /// Coste de encendido: Ascenso exige 8× cristal alto + karma+; Descenso
    /// exige ofrenda (el llamador descuenta del inventario).
    pub fn can_ignite(&self, karma: &Karma, has_offering: bool) -> bool {
        match self.kind {
            PortalKind::Ascension => karma.compasion > 0.0 && karma.justicia > 0.0 && has_offering,
            PortalKind::Descenso => has_offering,
        }
    }
    pub fn destination(&self) -> Dimension {
        match self.kind {
            PortalKind::Ascension => Dimension::Cielo,
            PortalKind::Descenso => Dimension::Infierno,
        }
    }
}

/// Punto de reaparición: cama (Tierra) / ancla del alma / origen.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Respawn {
    pub bed: Option<[i32; 3]>,
    pub anchor: Option<[i32; 3]>,
    pub origin: [i32; 3],
}

impl Respawn {
    pub fn on_death(&self, dim: Dimension, _karma: &Karma, seed: u64) -> ([i32; 3], Dimension) {
        match dim {
            Dimension::Tierra => (
                self.bed.or(self.anchor).unwrap_or(self.origin),
                Dimension::Tierra,
            ),
            Dimension::Cielo | Dimension::Infierno => {
                let h = rivaren_core::hash3(seed, self.origin[0], self.origin[1], self.origin[2]);
                let dx = (h % 512) as i32 - 256;
                let dz = ((h >> 16) % 512) as i32 - 256;
                ([self.origin[0] + dx, 120, self.origin[2] + dz], dim)
            }
        }
    }
    pub fn on_exit_hell_via_portal(&self) -> [i32; 3] {
        self.bed.or(self.anchor).unwrap_or(self.origin)
    }
}

/// Fragmento de historia de aldea (generación procedural narrativa).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VillageFragment {
    pub seed: u64,
    pub title: String,
    pub culture: String,
    pub problem: String,
}

pub fn village_fragment(seed: u64, chunk_x: i32, chunk_z: i32) -> VillageFragment {
    let h = rivaren_core::hash3(seed, chunk_x, 0, chunk_z);
    let titles = [
        ("Río Roto", "pesquera", "reparar el dique antes de la crecida"),
        ("Cristal Silente", "minera", "hallar el origen del zumbido"),
        ("Sin Nombre", "olvidadiza", "reconstruir la memoria en el archivo"),
        ("Brass del Viento", "pastora", "recuperar el rebaño de la meseta"),
        ("Hogar Hondo", "forjadora", "reencender la fragua madre"),
    ];
    let t = titles[(h % titles.len() as u64) as usize];
    VillageFragment {
        seed,
        title: format!("La Aldea del {}", t.0),
        culture: t.1.to_string(),
        problem: t.2.to_string(),
    }
}

/// Receta de crafteo original (datos en assets/data/recipes.json).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Recipe {
    pub input: Vec<(String, u32)>,
    pub output: (String, u32),
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn karma_death_routes() {
        let good = Karma { compasion: 80.0, justicia: 50.0, sabiduria: 0.0 };
        assert_eq!(good.death_destination(), Dimension::Cielo);
        let bad = Karma { compasion: -80.0, ..Default::default() };
        assert_eq!(bad.death_destination(), Dimension::Infierno);
    }
    #[test]
    fn portal_destination() {
        let p = Portal { kind: PortalKind::Ascension, frame: [[0; 3]; 4], lit: true };
        assert_eq!(p.destination(), Dimension::Cielo);
        let k = Karma { compasion: 10.0, justicia: 10.0, sabiduria: 0.0 };
        assert!(p.can_ignite(&k, true));
        assert!(!p.can_ignite(&k, false));
    }
}
