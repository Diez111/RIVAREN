//! Pipeline de generación amortizado: jobs <1ms con deadline por frame.
//! Job1 bioma → Job2 height → Job3 cuevas → Job4 fill → Job5 structures.

use glam::IVec3;
use rivaren_core::{BlockId, CHUNK_SIZE, CHUNK_VOLUME, FrameId, Seed, AIR};
use super::biomes::sample_biome;
use super::sdf::world_sdf;

/// Buffer de construcción de chunk: denso 32³ u16, cero allocs tras `new`.
pub struct ChunkBuilder {
    pub chunk: IVec3,
    pub voxels: Box<[BlockId; CHUNK_VOLUME]>,
}

impl ChunkBuilder {
    pub fn new(chunk: IVec3) -> Self {
        Self { chunk, voxels: Box::new([AIR; CHUNK_VOLUME]) }
    }
    #[inline(always)]
    fn idx(x: usize, y: usize, z: usize) -> usize {
        (y * CHUNK_SIZE + z) * CHUNK_SIZE + x
    }
    #[inline(always)]
    pub fn set_local(&mut self, x: i32, y: i32, z: i32, v: BlockId) {
        if x < 0 || y < 0 || z < 0 || x >= 32 || y >= 32 || z >= 32 {
            return;
        }
        self.voxels[Self::idx(x as usize, y as usize, z as usize)] = v;
    }
    #[inline(always)]
    pub fn get_local(&self, x: usize, y: usize, z: usize) -> BlockId {
        self.voxels[Self::idx(x, y, z)]
    }
}

#[derive(Debug, Clone, Copy)]
pub struct JobDeadline(pub FrameId);

/// Genera un chunk completo de forma determinista.
/// Llama a los jobs en orden; cada job chequea `cancel: &AtomicBool`.
pub fn generate_chunk(
    seed: Seed,
    chunk: IVec3,
    cancel: &std::sync::atomic::AtomicBool,
) -> ChunkBuilder {
    use std::sync::atomic::Ordering;
    let mut b = ChunkBuilder::new(chunk);
    let base = IVec3::new(chunk.x * 32, chunk.y * 32, chunk.z * 32);

    // Job1+2 fusionados y HOISTED: altura + bioma una vez por columna (x,z),
    // no por vóxel. Ahorra ~32x fBm (de 7ms → ~2ms). Cache 32×32 en stack.
    let mut heights = [[0.0f32; 32]; 32];
    let mut biomes = [[0u16; 32]; 32];
    for z in 0..32 {
        for x in 0..32 {
            let wx = (base.x + x as i32) as f32;
            let wz = (base.z + z as i32) as f32;
            heights[z][x] = super::sdf::surface_height(seed, wx, wz);
            biomes[z][x] = sample_biome(seed, wx, wz);
        }
        if cancel.load(Ordering::Relaxed) {
            break;
        }
    }
    // Job4 fill: SDF → bloque. y<h → sólido con capas top/filler/piedra.
    for y in 0..32 {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        let wy = (base.y + y as i32) as f32;
        for z in 0..32 {
            for x in 0..32 {
                let wx = (base.x + x as i32) as f32;
                let wz = (base.z + z as i32) as f32;
                let h = heights[z][x];
                // Early-out barato: si wy muy por encima/debajo, evita SDF caro.
                // Banda de ±24 alrededor de la superficie; fuera = trivial.
                let sdf = if wy > h + 24.0 {
                    // Cielo: solo agua si bajo nivel del mar.
                    10.0
                } else if wy < h - 48.0 {
                    // Profundo: sólido salvo cueva (evalúa cuevas igual).
                    world_sdf(seed, wx, wy, wz)
                } else {
                    world_sdf(seed, wx, wy, wz)
                };
                if sdf <= 0.0 {
                    let biome = biomes[z][x];
                    let _ = (h, biome);
                    // Capas finas se aplican después con un pase por columna
                    // (el bloqueo del SDF no coincide 1:1 con la altura suave).
                    b.voxels[ChunkBuilder::idx(x, y, z)] = 4; // piedra base
                } else if wy < 62.0 && wy > 40.0 {
                    // Nivel del mar aproximado → agua (id 20).
                    b.voxels[ChunkBuilder::idx(x, y, z)] = 20;
                }
            }
        }
    }
    // Estratos: solo la PRIMERA capa sólida desde arriba lleva hierba/tierra;
    // los techos de cueva y estantes interiores quedan de piedra.
    for z in 0..32 {
        for x in 0..32 {
            let biome = biomes[z][x];
            let mut layer = 0i32;
            let mut in_solid = false;
            let mut surface_done = false;
            for y in (0..32).rev() {
                let v = b.voxels[ChunkBuilder::idx(x, y, z)];
                if v == 4 {
                    if !in_solid {
                        in_solid = true;
                        layer = 0;
                    }
                    if !surface_done {
                        let block: BlockId = match layer {
                            0 => biome_surface_top(biome),
                            1..=3 => 2, // tierra ferral
                            _ => 4,
                        };
                        b.voxels[ChunkBuilder::idx(x, y, z)] = block;
                        layer += 1;
                    }
                } else if v == AIR || v == 20 {
                    if in_solid {
                        surface_done = true;
                    }
                    in_solid = false;
                }
            }
        }
    }
    // Job5 structures
    for s in super::structures::all_structures() {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        // Solo si el chunk está en rango de superficie para aldeas.
        let _ = s.try_generate(seed, chunk, &mut b);
    }
    b
}

#[inline(always)]
fn biome_surface_top(biome: u16) -> BlockId {
    match biome {
        2 => 6,  // arena canto
        9 => 14, // fondo abisal
        _ => 1,  // hierba ferral
    }
}

/// Descriptor de job con deadline (para `app::jobs::JobSystem`).
#[derive(Debug)]
pub struct GenerationJob {
    pub chunk: IVec3,
    pub deadline: JobDeadline,
    pub stage: u8, // 0..6
}
