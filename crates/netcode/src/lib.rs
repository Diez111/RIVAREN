//! `rivaren-netcode`: rollback determinista + predicción + replicación por deltas.
//! Simulación en fixed-point (core::Fixed); re-simulación <2ms/8 frames.

use rivaren_core::Fixed;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Input {
    pub frame: u64,
    pub move_x: i8,
    pub move_z: i8,
    pub jump: bool,
    pub action: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    pub frame: u64,
    pub player: [Fixed; 3],
}

pub struct Rollback {
    inputs: VecDeque<Input>,
    snaps: VecDeque<Snapshot>,
    pub current: u64,
    pub confirmed: u64,
}

impl Rollback {
    pub fn new() -> Self {
        Self { inputs: VecDeque::with_capacity(256), snaps: VecDeque::with_capacity(64), current: 0, confirmed: 0 }
    }
    pub fn push_input(&mut self, i: Input) {
        self.inputs.push_back(i);
        if self.inputs.len() > 256 {
            self.inputs.pop_front();
        }
    }
    pub fn confirm(&mut self, frame: u64) {
        self.confirmed = self.confirmed.max(frame);
        while self.snaps.front().map(|s| s.frame < frame.saturating_sub(32)).unwrap_or(false) {
            self.snaps.pop_front();
        }
    }
    /// Rebobina al frame y re-simula (el llamador re-ejecuta la física).
    pub fn rollback_to(&mut self, frame: u64) -> Vec<Input> {
        self.current = frame;
        self.inputs.iter().filter(|i| i.frame >= frame).copied().collect()
    }
}

impl Default for Rollback {
    fn default() -> Self {
        Self::new()
    }
}
