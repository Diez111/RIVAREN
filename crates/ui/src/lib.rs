//! `rivaren-ui`: HUD, inventario, brújula moral, mapa del alma.
//! Render desacoplado (glyphon en Fase 4); aquí el modelo de datos + lógica.

use serde::{Deserialize, Serialize};

/// Brújula moral: aguja 3D en espacio karma (no números).
#[derive(Debug, Clone, Copy)]
pub struct MoralCompass {
    pub dir: [f32; 3],
}

impl MoralCompass {
    pub fn from_karma(k: &rivaren_gameplay::Karma) -> Self {
        let v = glam::Vec3::new(k.compasion, k.justicia, k.sabiduria);
        let n = if v.length_squared() > 1e-6 { v.normalize() } else { glam::Vec3::Z };
        Self { dir: n.into() }
    }
}

/// Mapa del Alma: 1 base + 3 POIs, vinculado al jugador.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SoulMap {
    pub base: Option<[i32; 3]>,
    pub pois: Vec<[i32; 3]>,
}

impl SoulMap {
    pub fn set_base(&mut self, p: [i32; 3]) {
        self.base = Some(p);
    }
    pub fn add_poi(&mut self, p: [i32; 3]) -> bool {
        if self.pois.len() >= 3 {
            return false;
        }
        self.pois.push(p);
        true
    }
}
