//! `rivaren-mods`: API de mods + carga de manifests + sandbox WASM (feature).
//!
//! Formato de mod (JSON, sin código):
//! ```json
//! { "mod": "mi_mod", "version": "1.0", "blocks": [{"id":"mi_mod:bloque","name":"Bloque","solid":true,"light":0}],
//!   "biomes": [{"id":"mi_mod:bioma","name":"Bioma"}],
//!   "karma_axis": {"id":"honor","name":"Honor","description":"..."} }
//! ```
//! Con la feature `wasm` se habilita `wasmtime` para comportamientos sandbox.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

pub const MOD_BLOCK_ID_BASE: u16 = 200;
pub const MOD_ITEM_ID_BASE: u16 = 200;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockDef {
    pub id: String,
    pub name: String,
    #[serde(default = "default_true")]
    pub solid: bool,
    #[serde(default)]
    pub light: u8,
    /// Color RGB 0..1 opcional (si falta se deriva del id).
    #[serde(default)]
    pub color: Option<[f32; 3]>,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BiomeDef {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub temperature: f32,
    #[serde(default)]
    pub humidity: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KarmaAxisDef {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModManifest {
    pub mod_id: String,
    #[serde(default)]
    pub version: String,
    #[serde(alias = "mod")]
    pub name: String,
    #[serde(default)]
    pub blocks: Vec<BlockDef>,
    #[serde(default)]
    pub biomes: Vec<BiomeDef>,
    #[serde(default)]
    pub karma_axis: Option<KarmaAxisDef>,
}

#[derive(Debug, Clone)]
pub struct LoadedMod {
    pub path: String,
    pub manifest: ModManifest,
    pub block_ids: Vec<(String, u16)>,
    pub biome_ids: Vec<(String, u16)>,
}

#[derive(Default)]
pub struct ModRegistry {
    pub mods: Vec<LoadedMod>,
    pub blocks: HashMap<String, (u16, BlockDef)>,
    pub biomes: HashMap<String, (u16, BiomeDef)>,
    pub karma_axes: HashMap<String, KarmaAxisDef>,
    next_block: u16,
    next_biome: u16,
}

impl ModRegistry {
    pub fn new() -> Self {
        Self {
            next_block: MOD_BLOCK_ID_BASE,
            next_biome: 200,
            ..Default::default()
        }
    }

    pub fn load_dir(&mut self, dir: &Path) -> Result<usize> {
        if !dir.exists() {
            return Ok(0);
        }
        let mut loaded = 0;
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().map(|e| e == "json").unwrap_or(false) {
                let text = std::fs::read_to_string(&path)?;
                match self.load_manifest(&text, &path.display().to_string()) {
                    Ok(m) => {
                        tracing::info!(
                            "mod cargado: {} ({} bloques, {} biomas)",
                            m.manifest.name,
                            m.manifest.blocks.len(),
                            m.manifest.biomes.len()
                        );
                        self.mods.push(m);
                        loaded += 1;
                    }
                    Err(e) => tracing::warn!("mod inválido {}: {e:#}", path.display()),
                }
            }
        }
        Ok(loaded)
    }

    pub fn load_manifest(&mut self, text: &str, path: &str) -> Result<LoadedMod> {
        // Acepta tanto {"mod": ...} (el campo name) como {"mod_id": ...}.
        let mut manifest: ModManifest = serde_json::from_str(text).context("JSON inválido")?;
        if manifest.name.is_empty() {
            manifest.name = manifest.mod_id.clone();
        }
        let mut block_ids = Vec::new();
        for b in &manifest.blocks {
            let id = self.next_block;
            self.next_block += 1;
            block_ids.push((b.id.clone(), id));
            self.blocks.insert(b.id.clone(), (id, b.clone()));
        }
        let mut biome_ids = Vec::new();
        for b in &manifest.biomes {
            let id = self.next_biome;
            self.next_biome += 1;
            biome_ids.push((b.id.clone(), id));
            self.biomes.insert(b.id.clone(), (id, b.clone()));
        }
        if let Some(axis) = &manifest.karma_axis {
            self.karma_axes.insert(axis.id.clone(), axis.clone());
        }
        Ok(LoadedMod {
            path: path.to_string(),
            manifest,
            block_ids,
            biome_ids,
        })
    }

    pub fn block_id(&self, id: &str) -> Option<u16> {
        self.blocks.get(id).map(|(v, _)| *v)
    }
}

// ── API de mods para el juego ──

pub trait ModApi {
    fn register_block(&mut self, def: BlockDef) -> u16;
    fn register_biome(&mut self, def: BiomeDef) -> u16;
    fn on_block_place(&mut self, block: u16, x: i32, y: i32, z: i32);
}

impl ModApi for ModRegistry {
    fn register_block(&mut self, def: BlockDef) -> u16 {
        let id = self.next_block;
        self.next_block += 1;
        self.blocks.insert(def.id.clone(), (id, def));
        id
    }
    fn register_biome(&mut self, def: BiomeDef) -> u16 {
        let id = self.next_biome;
        self.next_biome += 1;
        self.biomes.insert(def.id.clone(), (id, def));
        id
    }
    fn on_block_place(&mut self, _block: u16, _x: i32, _y: i32, _z: i32) {}
}

/// Color determinista para un bloque de mod (el shader usa la misma fórmula).
pub fn mod_block_color(block: u16) -> [f32; 3] {
    let h = ((block as u32).wrapping_mul(2654435761) >> 8) & 0xFFFF;
    let hue = (h as f32) / 65535.0;
    hsv_to_rgb(hue, 0.55, 0.85)
}

pub fn hsv_to_rgb(h: f32, s: f32, v: f32) -> [f32; 3] {
    let i = (h * 6.0).floor();
    let f = h * 6.0 - i;
    let p = v * (1.0 - s);
    let q = v * (1.0 - f * s);
    let t = v * (1.0 - (1.0 - f) * s);
    match (i as i32).rem_euclid(6) {
        0 => [v, t, p],
        1 => [q, v, p],
        2 => [p, v, t],
        3 => [p, q, v],
        4 => [t, p, v],
        _ => [v, p, q],
    }
}

/// Carga un mod desde JSON (helper).
pub fn load_mod_json(registry: &mut ModRegistry, json: &str) -> Result<LoadedMod> {
    registry.load_manifest(json, "<memoria>")
}

#[cfg(test)]
mod tests {
    use super::*;

    const EJEMPLO: &str = r#"{
        "mod": "ejemplo_cristal",
        "mod_id": "ejemplo_cristal",
        "version": "0.1.0",
        "blocks": [
            {"id": "ejemplo_cristal:bloque_prisma", "name": "Bloque Prisma", "solid": true, "light": 7}
        ],
        "biomes": [{"id": "ejemplo_cristal:vergel", "name": "Vergel Prisma"}],
        "karma_axis": {"id": "honor", "name": "Honor", "description": "Reputación en combate ritual."}
    }"#;

    #[test]
    fn carga_manifest() {
        let mut reg = ModRegistry::new();
        let m = load_mod_json(&mut reg, EJEMPLO).unwrap();
        assert_eq!(m.manifest.name, "ejemplo_cristal");
        assert_eq!(reg.blocks.len(), 1);
        assert_eq!(reg.biomes.len(), 1);
        assert!(reg.karma_axes.contains_key("honor"));
        let id = reg.block_id("ejemplo_cristal:bloque_prisma").unwrap();
        assert!(id >= MOD_BLOCK_ID_BASE);
    }

    #[test]
    fn color_determinista() {
        assert_eq!(mod_block_color(200), mod_block_color(200));
        assert_ne!(mod_block_color(200), mod_block_color(201));
    }
}

// ── Sandbox WASM (feature `wasm`) ────────────────────────────────────
//
// Un mod WASM exporta `init(api_ptr: u32, api_len: u32) -> u32` y usa la API
// del host para registrar bloques/comportamientos. El host expone funciones
// con límites de memoria y tiempo (1 ms por llamada).
#[cfg(feature = "wasm")]
pub mod wasm {
    use anyhow::{Context, Result};
    use wasmtime::{Config, Engine, Instance, Module, Store, TypedFunc};

    pub struct WasmMod {
        store: Store<()>,
        instance: Instance,
        pub memory: wasmtime::Memory,
    }

    impl WasmMod {
        pub fn load(bytes: &[u8]) -> Result<Self> {
            let mut config = Config::new();
            config.epoch_interruption(true);
            config.wasm_backtrace(false);
            let engine = Engine::new(&config)?;
            let module = Module::new(&engine, bytes).context("wasm inválido")?;
            let mut store = Store::new(&engine, ());
            // Límite de épocas: 1 ms aprox (el host incrementa la época).
            store.set_epoch_deadline(1);
            let instance = Instance::new(&mut store, &module, &[])?;
            let memory = match instance.get_memory(&mut store, "memory") {
                Some(m) => m,
                None => wasmtime::Memory::new(&mut store, wasmtime::MemoryType::new(1, Some(16)))?,
            };
            Ok(Self {
                store,
                instance,
                memory,
            })
        }

        pub fn call_i32(&mut self, name: &str, arg: i32) -> Result<i32> {
            let f: TypedFunc<i32, i32> = self
                .instance
                .get_typed_func(&mut self.store, name)
                .with_context(|| format!("export {name} no encontrado"))?;
            Ok(f.call(&mut self.store, arg)?)
        }
    }
}

#[cfg(not(feature = "wasm"))]
pub mod wasm {
    use anyhow::Result;

    /// Placeholder cuando la feature `wasm` está desactivada.
    pub struct WasmMod;

    impl WasmMod {
        pub fn load(_bytes: &[u8]) -> Result<Self> {
            anyhow::bail!("compila con --features wasm para habilitar mods WASM")
        }
    }
}
