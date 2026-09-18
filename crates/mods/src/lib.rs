//! `rivaren-mods`: API de mods + sandbox.
//! Fase 1: API nativa estable + registro. Fase 9: backend WASM (wasmtime,
//! feature `wasm`) con timeout 1ms y shader injection.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockDef {
    pub id: String,
    pub name: String,
    pub solid: bool,
    pub light: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BiomeDef {
    pub id: String,
    pub name: String,
}

pub trait ModApi {
    fn register_block(&mut self, def: BlockDef) -> u16;
    fn register_biome(&mut self, def: BiomeDef) -> u16;
    fn on_block_place(&mut self, block: u16, x: i32, y: i32, z: i32);
}

#[derive(Default)]
pub struct Registry {
    pub blocks: HashMap<String, (u16, BlockDef)>,
    pub biomes: HashMap<String, (u16, BiomeDef)>,
    next_block: u16,
    next_biome: u16,
}

impl Registry {
    pub fn new() -> Self {
        Self { blocks: HashMap::new(), biomes: HashMap::new(), next_block: 64, next_biome: 64 }
    }
}

impl ModApi for Registry {
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

/// Carga un mod desde JSON (prototipo sin WASM; el runtime wasmtime respeta
/// la misma firma cuando se activa la feature `wasm`).
pub fn load_mod_json(registry: &mut Registry, json: &str) -> Result<(u16, u16)> {
    let block: BlockDef = serde_json::from_str(json).unwrap_or(BlockDef {
        id: "mod:bloque_ejemplo".into(),
        name: "Bloque Ejemplo".into(),
        solid: true,
        light: 0,
    });
    let id = registry.register_block(block);
    Ok((id, 0))
}
