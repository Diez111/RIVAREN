//! `rivaren-audio`: kira + sonidos sintetizados proceduralmente (100% original).
//! Música y SFX generados en memoria: sin assets con copyright.

use kira::sound::static_sound::{StaticSoundData, StaticSoundSettings};
use kira::{AudioManager, AudioManagerSettings, Decibels, DefaultBackend, Frame, Tween};
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Sfx {
    Click,
    Step,
    Place,
    Break,
    Hurt,
    Craft,
    Portal,
    LevelUp,
    UiOpen,
    UiClose,
    Rain,
}

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

pub struct AudioEngine {
    manager: Option<AudioManager<DefaultBackend>>,
    sfx: HashMap<Sfx, StaticSoundData>,
    pub master: f32,
    pub music: f32,
    pub sfx_volume: f32,
    music_playing: bool,
}

impl Default for AudioEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioEngine {
    pub fn new() -> Self {
        let manager = match AudioManager::<DefaultBackend>::new(AudioManagerSettings::default()) {
            Ok(m) => Some(m),
            Err(e) => {
                tracing::warn!("audio deshabilitado: {e}");
                None
            }
        };
        let mut sfx = HashMap::new();
        sfx.insert(Sfx::Click, synth_click());
        sfx.insert(Sfx::Step, synth_step());
        sfx.insert(Sfx::Place, synth_place());
        sfx.insert(Sfx::Break, synth_break());
        sfx.insert(Sfx::Hurt, synth_hurt());
        sfx.insert(Sfx::Craft, synth_craft());
        sfx.insert(Sfx::Portal, synth_portal());
        sfx.insert(Sfx::LevelUp, synth_level_up());
        sfx.insert(Sfx::UiOpen, synth_ui(520.0));
        sfx.insert(Sfx::UiClose, synth_ui(320.0));
        sfx.insert(Sfx::Rain, synth_rain());
        Self {
            manager,
            sfx,
            master: 0.8,
            music: 0.5,
            sfx_volume: 0.8,
            music_playing: false,
        }
    }

    pub fn set_volumes(&mut self, master: f32, music: f32, sfx: f32) {
        self.master = master.clamp(0.0, 1.0);
        self.music = music.clamp(0.0, 1.0);
        self.sfx_volume = sfx.clamp(0.0, 1.0);
    }

    pub fn play(&mut self, kind: Sfx, gain: f32, pan: f32) {
        let Some(manager) = self.manager.as_mut() else {
            return;
        };
        let Some(base) = self.sfx.get(&kind) else {
            return;
        };
        let vol = (gain * self.sfx_volume * self.master).clamp(0.0, 1.0);
        if vol < 0.01 {
            return;
        }
        let db = if vol > 0.0 {
            20.0 * vol.log10()
        } else {
            -80.0
        };
        let sound = base
            .clone()
            .volume(Decibels(db))
            .panning(pan.clamp(-1.0, 1.0));
        let _ = manager.play(sound);
    }

    /// Volumen y pan desde una fuente posicional respecto al oyente.
    pub fn play_at(&mut self, kind: Sfx, src: [f32; 3], listener: [f32; 3], forward: [f32; 3]) {
        let dx = src[0] - listener[0];
        let dy = src[1] - listener[1];
        let dz = src[2] - listener[2];
        let dist = (dx * dx + dy * dy + dz * dz).sqrt();
        if dist > 48.0 {
            return;
        }
        let att = (1.0 - dist / 48.0).powi(2);
        let right = [forward[2], 0.0, -forward[0]];
        let pan = if dist > 0.01 {
            ((dx * right[0] + dz * right[2]) / dist).clamp(-1.0, 1.0)
        } else {
            0.0
        };
        self.play(kind, att, pan);
    }

    pub fn start_music(&mut self, soundscape: Soundscape) {
        if self.music_playing {
            return;
        }
        let Some(manager) = self.manager.as_mut() else {
            return;
        };
        let data = synth_ambient(soundscape);
        let vol = (self.music * self.master * 0.6).clamp(0.0, 1.0);
        let db = if vol > 0.0 {
            20.0 * vol.log10()
        } else {
            -80.0
        };
        let sound = data.volume(Decibels(db));
        let _ = manager.play(sound);
        self.music_playing = true;
    }

    pub fn is_enabled(&self) -> bool {
        self.manager.is_some()
    }
}

// ── Síntesis procedural (WAV en memoria como StaticSoundData) ──

const SR: u32 = 44100;

fn frames_from(samples: Vec<f32>) -> StaticSoundData {
    let frames: Arc<[Frame]> = samples
        .iter()
        .map(|s| Frame {
            left: *s,
            right: *s,
        })
        .collect::<Vec<_>>()
        .into();
    StaticSoundData {
        sample_rate: SR,
        frames,
        settings: StaticSoundSettings::default(),
        slice: None,
    }
}

fn tone(freq: f32, dur: f32, shape: fn(f32) -> f32, decay: f32) -> Vec<f32> {
    let n = (SR as f32 * dur) as usize;
    (0..n)
        .map(|i| {
            let t = i as f32 / SR as f32;
            let env = (-t * decay).exp();
            shape(t * freq * std::f32::consts::TAU) * env
        })
        .collect()
}

fn synth_click() -> StaticSoundData {
    frames_from(tone(760.0, 0.07, |p| p.sin(), 40.0))
}

fn synth_ui(freq: f32) -> StaticSoundData {
    frames_from(tone(freq, 0.09, |p| p.sin() + 0.35 * (p * 2.0).sin(), 22.0))
}

fn synth_step() -> StaticSoundData {
    let n = (SR as f32 * 0.10) as usize;
    let mut rng: u64 = 0x1234_5678;
    let samples: Vec<f32> = (0..n)
        .map(|i| {
            rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
            let noise = ((rng >> 33) as f32 / 2147483648.0) - 1.0;
            let t = i as f32 / SR as f32;
            let env = (-t * 45.0).exp();
            let body = (t * 140.0 * std::f32::consts::TAU).sin() * 0.5;
            (noise * 0.35 + body) * env * 0.7
        })
        .collect();
    frames_from(samples)
}

fn synth_place() -> StaticSoundData {
    frames_from(tone(220.0, 0.12, |p| p.sin() * 0.7 + (p * 1.5).sin() * 0.3, 30.0))
}

fn synth_break() -> StaticSoundData {
    let n = (SR as f32 * 0.18) as usize;
    let mut rng: u64 = 0xDEAD_BEEF;
    let samples: Vec<f32> = (0..n)
        .map(|i| {
            rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
            let noise = ((rng >> 33) as f32 / 2147483648.0) - 1.0;
            let t = i as f32 / SR as f32;
            let env = (-t * 22.0).exp();
            noise * env * 0.6
        })
        .collect();
    frames_from(samples)
}

fn synth_hurt() -> StaticSoundData {
    let n = (SR as f32 * 0.25) as usize;
    let samples: Vec<f32> = (0..n)
        .map(|i| {
            let t = i as f32 / SR as f32;
            let f = 380.0 - 220.0 * (t / 0.25);
            let env = (-t * 12.0).exp();
            (t * f * std::f32::consts::TAU).sin() * env * 0.8
        })
        .collect();
    frames_from(samples)
}

fn synth_craft() -> StaticSoundData {
    let mut a = tone(520.0, 0.10, |p| p.sin(), 25.0);
    let b = tone(780.0, 0.12, |p| p.sin(), 20.0);
    a.extend(b);
    frames_from(a)
}

fn synth_level_up() -> StaticSoundData {
    let mut out = Vec::new();
    for (i, f) in [440.0f32, 554.0, 659.0].iter().enumerate() {
        let mut part = tone(*f, 0.12, |p| p.sin(), 14.0);
        let _ = i;
        out.append(&mut part);
    }
    frames_from(out)
}

fn synth_portal() -> StaticSoundData {
    let n = (SR as f32 * 1.2) as usize;
    let samples: Vec<f32> = (0..n)
        .map(|i| {
            let t = i as f32 / SR as f32;
            let f = 180.0 + 700.0 * (t / 1.2);
            let env = (1.0 - (t / 1.2)).max(0.0);
            (t * f * std::f32::consts::TAU).sin() * env * 0.5
                + (t * f * 1.5 * std::f32::consts::TAU).sin() * env * 0.2
        })
        .collect();
    frames_from(samples)
}

fn synth_rain() -> StaticSoundData {
    let n = (SR as f32 * 0.6) as usize;
    let mut rng: u64 = 0xABCD_EF01;
    let samples: Vec<f32> = (0..n)
        .map(|i| {
            rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
            let noise = ((rng >> 33) as f32 / 2147483648.0) - 1.0;
            let t = i as f32 / SR as f32;
            let env = 0.4 + 0.6 * (t * 3.0 * std::f32::consts::TAU).sin().abs();
            noise * env * 0.15
        })
        .collect();
    frames_from(samples)
}

/// Pad ambiental de 8s en loop, distinto por dimensión/bioma.
fn synth_ambient(scape: Soundscape) -> StaticSoundData {
    let (root, third, fifth) = match scape {
        Soundscape::Pradera => (110.0, 138.6, 164.8),
        Soundscape::Cueva => (82.4, 98.0, 123.5),
        Soundscape::Cielo => (146.8, 185.0, 220.0),
        Soundscape::Infierno => (65.4, 77.8, 92.5),
        Soundscape::Oceano => (98.0, 123.5, 147.0),
    };
    let dur = 8.0f32;
    let n = (SR as f32 * dur) as usize;
    let mut a: u64 = 0x1;
    let samples: Vec<f32> = (0..n)
        .map(|i| {
            let t = i as f32 / SR as f32;
            a = a.wrapping_mul(6364136223846793005).wrapping_add(1);
            let shimmer = ((a >> 40) as f32 / 16777216.0) * 0.02;
            let lfo1 = (t * 0.07 * std::f32::consts::TAU).sin() * 0.5 + 0.5;
            let lfo2 = (t * 0.11 * std::f32::consts::TAU).sin() * 0.5 + 0.5;
            let v = (t * root * std::f32::consts::TAU).sin() * 0.5 * (0.4 + 0.6 * lfo1)
                + (t * third * std::f32::consts::TAU).sin() * 0.3 * (0.3 + 0.7 * lfo2)
                + (t * fifth * std::f32::consts::TAU).sin() * 0.2
                + shimmer;
            v * 0.18
        })
        .collect();
    let mut data = frames_from(samples);
    data.settings = StaticSoundSettings::new().loop_region(..);
    let _ = Tween::default();
    data
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn synth_generates_frames() {
        let d = synth_click();
        assert_eq!(d.sample_rate, SR);
        assert!(!d.frames.is_empty());
    }
    #[test]
    fn ambient_loops() {
        let d = synth_ambient(Soundscape::Cielo);
        assert!(d.frames.len() > SR as usize);
    }
}
