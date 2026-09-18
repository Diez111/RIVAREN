//! `rivaren-meshing`: binary greedy meshing + LOD + re-meshing incremental.
//! 65–90µs por chunk (referencia inspirateur/binary-greedy-meshing).
//! Técnica: u64 bitmask por columna (64 vóxeles/instrucción),
//! runs vía trailing_zeros, fusión en quads, vértices PackedVertex 8B.

pub mod greedy;
pub mod incremental;
pub mod light;
pub mod lod;

pub use greedy::{MeshData, Quad, compute_opaque_mask, greedy_mesh, greedy_mesh_lit};
pub use incremental::DirtySet;
pub use light::bake_lighting;
pub use lod::{LodLevel, downsample_2x, select_lod};
