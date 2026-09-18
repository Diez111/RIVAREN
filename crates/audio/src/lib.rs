//! `rivaren-audio`: stub con soundscape por bioma/dimensión (Fase 1).
//! Fase 7+: backend rodio/kira con HRTF simple + oclusión por SDF.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Soundscape {
    Pradera,
    Cueva,
    Cielo,
    Infierno,
    Oceano,
}

pub fn soundscape_for(biome: u16, depth: i32) -> Soundscape {
    if depth < -16 {
        return Soundscape::Cueva;
    }
    match biome {
        9 => Soundscape::Oceano,
        _ => Soundscape::Pradera,
    }
}
