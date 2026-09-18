//! `rivaren-render`: backend wgpu (Vulkan/Metal/DX12) GPU-driven.
//! Frame: cull compute → LOD select → indirect args → raymarch cercanos
//!        → raster medios/lejos → sombras+GI → post (bloom/tonemap/FSR).

pub mod device;
pub mod indirect;
pub mod pipelines;

pub use device::{RenderDevice, RenderConfig, QualityTier};
pub use indirect::IndirectDrawArgs;
