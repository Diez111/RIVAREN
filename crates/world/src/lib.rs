//! `rivaren-world`: generación procedural infinita 3D.
//! El mundo NO se almacena, se calcula: `f(seed, x, y, z)` puro + determinista.
//!
//! Pipeline amortizado por jobs de <1ms con deadlines (ver `pipeline.rs`).
//! Optimizaciones: quick-noise manual (sin tablas), amortized lattice cache,
//! SIMD vía `wide` (8 vóxeles por instrucción), cero allocs en hot path.

pub mod biomes;
pub mod caves;
pub mod noise;
pub mod pipeline;
pub mod sdf;
pub mod structures;

pub use biomes::{Biome, BiomeId, CaveBiome};
pub use caves::caves_sdf;
pub use pipeline::{ChunkBuilder, GenerationJob, JobDeadline};
pub use sdf::{terrain_sdf, world_sdf};
pub use structures::StructureGenerator;
