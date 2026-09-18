//! `rivaren-compression`: SVDAG + SSVDAG + AADF + BrickMaps + SlidingWindow.
//! Objetivo: 0.08–0.12 bits/vóxel no vacío. Chunk 32³ → 2–8 KB.

pub mod aadf;
pub mod bricks;
pub mod svdag;
pub mod window;

pub use aadf::{AadfNode, build_aadf};
pub use bricks::{BrickPool, BRICKS_PER_CHUNK, BRICK_SIZE, BRICK_VOLUME};
pub use svdag::{Svdag, SvdagBuilder};
pub use window::{SlidingWindow, WindowDelta};
