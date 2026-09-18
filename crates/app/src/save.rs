//! Persistencia: ajustes, mundos (delta de ediciones + estado del jugador).

use anyhow::Result;
use rivaren_gameplay::{Dimension, Gamemode, Inventory, Karma, Settings, WorldConfig};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveData {
    pub version: u32,
    pub seed: u64,
    pub config: WorldConfig,
    pub dimension: Dimension,
    pub time_of_day: f32,
    pub player_pos: [f32; 3],
    pub player_yaw: f32,
    pub player_pitch: f32,
    pub inventory: Inventory,
    pub karma: Karma,
    pub completed_quests: Vec<String>,
    pub talked: Vec<String>,
    /// Log de modificaciones: pos → bloque.
    pub edits: Vec<([i32; 3], u16)>,
    pub respawn_bed: Option<[i32; 3]>,
    pub respawn_anchor: Option<[i32; 3]>,
    pub respawn_origin: [i32; 3],
}

pub fn config_dir() -> PathBuf {
    if let Some(d) = directories::ProjectDirs::from("dev", "rivaren", "rivaren") {
        d.config_dir().to_path_buf()
    } else {
        PathBuf::from(".rivaren")
    }
}

pub fn data_dir() -> PathBuf {
    if let Some(d) = directories::ProjectDirs::from("dev", "rivaren", "rivaren") {
        d.data_dir().to_path_buf()
    } else {
        PathBuf::from(".rivaren")
    }
}

pub fn save_settings(settings: &Settings) -> Result<()> {
    let dir = config_dir();
    std::fs::create_dir_all(&dir)?;
    let path = dir.join("settings.json");
    let json = serde_json::to_string_pretty(settings)?;
    std::fs::write(path, json)?;
    Ok(())
}

pub fn load_settings() -> Settings {
    let path = config_dir().join("settings.json");
    match std::fs::read_to_string(&path) {
        Ok(json) => serde_json::from_str(&json).unwrap_or_default(),
        Err(_) => Settings::default(),
    }
}

pub fn saves_dir() -> PathBuf {
    data_dir().join("saves")
}

pub fn list_saves() -> Vec<String> {
    let dir = saves_dir();
    let mut out = Vec::new();
    if let Ok(rd) = std::fs::read_dir(&dir) {
        for e in rd.flatten() {
            if let Some(name) = e.file_name().to_str() {
                if let Some(stripped) = name.strip_suffix(".rvr") {
                    out.push(stripped.to_string());
                }
            }
        }
    }
    out.sort();
    out
}

pub fn save_world(name: &str, data: &SaveData) -> Result<()> {
    let dir = saves_dir();
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(format!("{name}.rvr"));
    let bytes = bincode::serialize(data)?;
    let compressed = zstd::encode_all(bytes.as_slice(), 3)?;
    std::fs::write(path, compressed)?;
    Ok(())
}

pub fn load_world(name: &str) -> Result<SaveData> {
    let path = saves_dir().join(format!("{name}.rvr"));
    let compressed = std::fs::read(path)?;
    let bytes = zstd::decode_all(compressed.as_slice())?;
    let data: SaveData = bincode::deserialize(&bytes)?;
    Ok(data)
}

pub fn edits_to_map(edits: &[([i32; 3], u16)]) -> HashMap<[i32; 3], u16> {
    edits.iter().copied().collect()
}

pub fn default_save(seed: u64, config: &WorldConfig) -> SaveData {
    SaveData {
        version: 1,
        seed,
        config: config.clone(),
        dimension: config.start_dimension,
        time_of_day: 0.32,
        player_pos: [0.0, 100.0, 0.0],
        player_yaw: 0.0,
        player_pitch: 0.0,
        inventory: Inventory::default(),
        karma: Karma::default(),
        completed_quests: Vec::new(),
        talked: Vec::new(),
        edits: Vec::new(),
        respawn_bed: None,
        respawn_anchor: None,
        respawn_origin: [0, 100, 0],
    }
}

impl SaveData {
    pub fn gamemode_label(&self) -> &'static str {
        match self.config.gamemode {
            Gamemode::Survival => "Supervivencia",
            Gamemode::Creative => "Creativo",
        }
    }
}
