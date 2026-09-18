//! Configuración de mundo y de partida.

use serde::{Deserialize, Serialize};
use crate::Dimension;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Gamemode {
    Survival,
    Creative,
}

impl Gamemode {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Survival => "Supervivencia",
            Self::Creative => "Creativo",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Difficulty {
    Peaceful,
    Normal,
    Hard,
}

impl Difficulty {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Peaceful => "Pacífico",
            Self::Normal => "Normal",
            Self::Hard => "Difícil",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldConfig {
    pub name: String,
    pub seed: u64,
    pub gamemode: Gamemode,
    pub difficulty: Difficulty,
    pub render_distance: i32,
    pub start_dimension: Dimension,
    /// Estructuras activadas.
    pub structures: bool,
    /// Clima dinámico.
    pub weather: bool,
}

impl Default for WorldConfig {
    fn default() -> Self {
        let t = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(1337);
        Self {
            name: "Mundo Nuevo".into(),
            seed: t ^ 0x5EED,
            gamemode: Gamemode::Survival,
            difficulty: Difficulty::Normal,
            render_distance: 6,
            start_dimension: Dimension::Tierra,
            structures: true,
            weather: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphicsSettings {
    pub tier: u8, // 0 potato, 1 mobile, 2 balanced, 3 ultra
    pub render_scale: f32,
    pub vsync: bool,
    pub fps_cap: u32,
    pub shadows: bool,
    pub bloom: f32,
    pub sharpen: f32,
    pub taa: bool,
    pub fov: f32,
    pub view_distance: i32,
    pub fog: f32,
}

impl Default for GraphicsSettings {
    fn default() -> Self {
        Self {
            tier: 2,
            render_scale: 1.0,
            vsync: true,
            fps_cap: 60,
            shadows: true,
            bloom: 0.06,
            sharpen: 0.25,
            taa: true,
            fov: 70.0,
            view_distance: 6,
            fog: 1.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioSettings {
    pub master: f32,
    pub music: f32,
    pub sfx: f32,
}

impl Default for AudioSettings {
    fn default() -> Self {
        Self {
            master: 0.8,
            music: 0.5,
            sfx: 0.8,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameSettings {
    pub language: String,
    pub sensitivity: f32,
    pub hud_scale: f32,
    pub subtitles: bool,
    pub ai_mode: u8, // 0 reglas, 1 local, 2 nube
    pub ai_url: String,
    pub ai_model: String,
    pub ai_key_env: String,
}

impl Default for GameSettings {
    fn default() -> Self {
        Self {
            language: "es".into(),
            sensitivity: 0.12,
            hud_scale: 1.0,
            subtitles: true,
            ai_mode: 0,
            ai_url: "http://localhost:11434".into(),
            ai_model: "qwen3:4b".into(),
            ai_key_env: "RIVAREN_AI_KEY".into(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Settings {
    pub graphics: GraphicsSettings,
    pub audio: AudioSettings,
    pub game: GameSettings,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn settings_roundtrip() {
        let s = Settings::default();
        let json = serde_json::to_string(&s).unwrap();
        let back: Settings = serde_json::from_str(&json).unwrap();
        assert_eq!(back.graphics.view_distance, s.graphics.view_distance);
    }
}
