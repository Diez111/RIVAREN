//! Frame budget + resolución dinámica.

pub struct FrameBudget {
    pub target_fps: u32,
    pub frame_ms: f32,
    pub cpu_ms: f32,
    pub gpu_ms: f32,
}

impl FrameBudget {
    pub fn new(target_fps: u32) -> Self {
        let frame_ms = 1000.0 / target_fps as f32;
        Self { target_fps, frame_ms, cpu_ms: frame_ms * 0.4, gpu_ms: frame_ms * 0.6 }
    }
    /// Control proporcional simple: baja 0.05 si <90%, sube 0.05 si >110%.
    pub fn adapt_resolution(&self, fps: f32, scale: f32) -> f32 {
        if fps < self.target_fps as f32 * 0.9 {
            (scale - 0.05).max(0.5)
        } else if fps > self.target_fps as f32 * 1.1 {
            (scale + 0.05).min(1.0)
        } else {
            scale
        }
    }
}
