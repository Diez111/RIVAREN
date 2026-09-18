//! `rivaren-core`: tipos base, math, constantes. Cero dependencias pesadas.
//! Todo determinista, `#[repr(C)]`, Pod donde el hot path lo exige.

use bytemuck::{Pod, Zeroable};
use glam::{IVec3, Vec3};
use serde::{Deserialize, Serialize};

// ── Constantes globales ──────────────────────────────────────────────
/// Tamaño de chunk: 32³ (compromiso compresión SVDAG vs meshing).
pub const CHUNK_SIZE: usize = 32;
pub const CHUNK_VOLUME: usize = CHUNK_SIZE * CHUNK_SIZE * CHUNK_SIZE;
/// Ventana deslizante GPU: 512³ vóxeles direccionables.
pub const WINDOW_SIZE: i32 = 512;
/// Tick fijo de simulación: 20 TPS como Minecraft-like pero original.
pub const FIXED_TPS: u32 = 20;
pub const FIXED_DT: f32 = 1.0 / FIXED_TPS as f32;
/// Distancias LOD en chunks.
pub const LOD_DIST: [i32; 5] = [4, 12, 20, 30, i32::MAX];

pub type Seed = u64;
pub type FrameId = u64;
pub type BlockId = u16;
pub type ChunkPos = IVec3;

/// Aire = 0 siempre. Primer invariante del motor.
pub const AIR: BlockId = 0;

// ── Flags de bloque ──────────────────────────────────────────────────
bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
    pub struct BlockFlags: u8 {
        const OPAQUE      = 0b0000_0001;
        const TRANSLUCENT = 0b0000_0010;
        const EMISSIVE    = 0b0000_0100;
        const FLUID       = 0b0000_1000;
        const CLIMBABLE   = 0b0001_0000;
        const DYNAMIC     = 0b0010_0000;
    }
}

// ── Vértice comprimido 8 bytes ───────────────────────────────────────
/// 43 bits útiles + padding. Decodificado en vertex shader.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct PackedVertex {
    /// u:8 v:8 + textura:12 + padding:4
    pub uv_tex: u32,
    /// x:5 y:5 z:5 (dentro del chunk) + normal:3 + resto padding
    pub pos_packed: u16,
    /// ao:4 + sky:4
    pub ao_light: u8,
    pub _pad: u8,
}

impl PackedVertex {
    /// `ao`: 0..15, `sky`: 0..15 (luz de cielo horneada), `block`: 0..15.
    #[inline(always)]
    #[allow(clippy::too_many_arguments)]
    pub fn pack(x: u8, y: u8, z: u8, normal: u8, u: u8, v: u8, tex: u16, ao: u8, sky: u8, block: u8) -> Self {
        debug_assert!(x < 32 && y < 32 && z < 32 && normal < 6);
        let pos_packed = (x as u16) | ((y as u16) << 5) | ((z as u16) << 10) | ((normal as u16 & 7) << 15);
        let uv_tex = (u as u32) | ((v as u32) << 8) | ((tex as u32 & 0xFFF) << 16);
        let ao_light = (ao & 0xF) | ((sky & 0xF) << 4);
        Self { uv_tex, pos_packed, ao_light, _pad: block & 0xF }
    }
}

// ── Draw list de UI (renderer-agnóstico) ─────────────────────────────
/// Rectángulo con SDF rounded-rect, borde opcional y clip.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct UiQuad {
    /// min.x, min.y, max.x, max.y (píxeles, origen arriba-izquierda)
    pub rect: [f32; 4],
    /// r, g, b, a
    pub color: [f32; 4],
    /// radius, border, shadow, _
    pub params: [f32; 4],
    pub border_color: [f32; 4],
    /// min.x, min.y, max.x, max.y clip
    pub clip: [f32; 4],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextAlign {
    Left,
    Center,
    Right,
}

#[derive(Debug, Clone)]
pub struct UiText {
    pub text: String,
    pub x: f32,
    pub y: f32,
    pub size: f32,
    pub color: [f32; 4],
    pub align: TextAlign,
    pub max_width: f32,
    pub clip: [f32; 4],
    pub bold: bool,
}

#[derive(Debug, Clone, Default)]
pub struct DrawList {
    pub quads: Vec<UiQuad>,
    pub texts: Vec<UiText>,
}

impl DrawList {
    pub fn clear(&mut self) {
        self.quads.clear();
        self.texts.clear();
    }
    pub fn rect(&mut self, x: f32, y: f32, w: f32, h: f32, color: [f32; 4]) {
        self.quads.push(UiQuad {
            rect: [x, y, x + w, y + h],
            color,
            params: [0.0; 4],
            border_color: [0.0; 4],
            clip: [0.0, 0.0, 100000.0, 100000.0],
        });
    }
    pub fn rounded(&mut self, x: f32, y: f32, w: f32, h: f32, radius: f32, color: [f32; 4]) {
        self.quads.push(UiQuad {
            rect: [x, y, x + w, y + h],
            color,
            params: [radius, 0.0, 0.0, 0.0],
            border_color: [0.0; 4],
            clip: [0.0, 0.0, 100000.0, 100000.0],
        });
    }
    #[allow(clippy::too_many_arguments)]
    pub fn bordered(
        &mut self,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        radius: f32,
        border: f32,
        color: [f32; 4],
        border_color: [f32; 4],
    ) {
        self.quads.push(UiQuad {
            rect: [x, y, x + w, y + h],
            color,
            params: [radius, border, 0.0, 0.0],
            border_color,
            clip: [0.0, 0.0, 100000.0, 100000.0],
        });
    }
    pub fn text(&mut self, text: impl Into<String>, x: f32, y: f32, size: f32, color: [f32; 4]) {
        self.texts.push(UiText {
            text: text.into(),
            x,
            y,
            size,
            color,
            align: TextAlign::Left,
            max_width: f32::INFINITY,
            clip: [0.0, 0.0, 100000.0, 100000.0],
            bold: false,
        });
    }
    pub fn text_centered(&mut self, text: impl Into<String>, cx: f32, y: f32, size: f32, color: [f32; 4]) {
        self.texts.push(UiText {
            text: text.into(),
            x: cx,
            y,
            size,
            color,
            align: TextAlign::Center,
            max_width: f32::INFINITY,
            clip: [0.0, 0.0, 100000.0, 100000.0],
            bold: false,
        });
    }
    pub fn measure_hint(&self) -> usize {
        self.quads.len() + self.texts.len()
    }
}

// ── AABB ─────────────────────────────────────────────────────────────
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Aabb {
    pub min: Vec3,
    pub max: Vec3,
}

impl Aabb {
    #[inline(always)]
    pub fn from_center_half(center: Vec3, half: Vec3) -> Self {
        Self { min: center - half, max: center + half }
    }
    #[inline(always)]
    pub fn intersects(&self, other: &Self) -> bool {
        self.min.x < other.max.x && self.max.x > other.min.x
            && self.min.y < other.max.y && self.max.y > other.min.y
            && self.min.z < other.max.z && self.max.z > other.min.z
    }
}

// ── Fixed-point determinista para netcode ────────────────────────────
/// I24F8: 24 bits enteros + 8 fraccionarios. Determinista cross-platform.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Fixed(pub i32);

impl Fixed {
    pub const SCALE: i32 = 256;
    #[inline(always)]
    pub fn from_f32(v: f32) -> Self {
        Self((v * Self::SCALE as f32) as i32)
    }
    #[inline(always)]
    pub fn to_f32(self) -> f32 {
        self.0 as f32 / Self::SCALE as f32
    }
}

// ── Helpers deterministas ────────────────────────────────────────────
/// Hash splitmix64: base de todo el noise determinista. `f(seed,x,y,z)` puro.
#[inline(always)]
pub fn hash3(seed: Seed, x: i32, y: i32, z: i32) -> u64 {
    let mut h = seed.wrapping_add(0x9E3779B97F4A7C15);
    h = h.wrapping_add((x as u64).wrapping_mul(0xBF58476D1CE4E5B9));
    h = h.wrapping_add((y as u64).wrapping_mul(0x94D049BB133111EB));
    h = h.wrapping_add((z as u64).wrapping_mul(0xDA942042E4DD58B5));
    // splitmix64 finalizer
    h ^= h >> 30;
    h = h.wrapping_mul(0xBF58476D1CE4E5B9);
    h ^= h >> 27;
    h = h.wrapping_mul(0x94D049BB133111EB);
    h ^= h >> 31;
    h
}

/// Convierte hash a f32 en [0,1). Determinista en todas las plataformas.
#[inline(always)]
pub fn hash_to_unit(h: u64) -> f32 {
    // Usa 24 bits superiores → exacto en f32.
    ((h >> 40) as u32 as f32) / 16_777_216.0
}

#[inline(always)]
pub fn chunk_of(voxel: IVec3) -> ChunkPos {
    IVec3::new(
        voxel.x.div_euclid(CHUNK_SIZE as i32),
        voxel.y.div_euclid(CHUNK_SIZE as i32),
        voxel.z.div_euclid(CHUNK_SIZE as i32),
    )
}

#[inline(always)]
pub fn voxel_in_chunk(voxel: IVec3) -> (u8, u8, u8) {
    (
        voxel.x.rem_euclid(CHUNK_SIZE as i32) as u8,
        voxel.y.rem_euclid(CHUNK_SIZE as i32) as u8,
        voxel.z.rem_euclid(CHUNK_SIZE as i32) as u8,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hash_deterministic() {
        assert_eq!(hash3(123, 1, 2, 3), hash3(123, 1, 2, 3));
        assert_ne!(hash3(123, 1, 2, 3), hash3(124, 1, 2, 3));
    }
    #[test]
    fn chunk_math() {
        assert_eq!(chunk_of(IVec3::new(33, -1, 0)), IVec3::new(1, -1, 0));
        assert_eq!(Fixed::from_f32(1.5).to_f32(), 1.5);
    }
}
