//! Sesión de juego: streaming de chunks, jugador, interacción, NPCs, IA, audio.

use glam::{IVec3, Vec3};
use rivaren_ai::{AiEvent, AiService};
use rivaren_audio::{AudioEngine, Sfx, Soundscape};
use rivaren_core::{AIR, CHUNK_VOLUME, hash3};
use rivaren_gameplay::{
    Dimension, Inventory, ItemStack, Karma, KarmaAxis, ObjectiveCtx, QuestLog, Respawn,
    WorldConfig, item_def, item_for_block,
};
use rivaren_gameplay::dialogue::{DialogueTree, NpcMemory, tree_for};
use rivaren_meshing::{MeshData, bake_lighting, greedy_mesh_lit};
use rivaren_physics::pulso::{GateMode, PulsoKind, PulsoWorld};
use rivaren_render::{Camera, Renderer};
use std::collections::{HashMap, HashSet};
use std::sync::atomic::AtomicBool;

pub const CHUNK: i32 = 32;
const PLAYER_HALF: f32 = 0.3;
const PLAYER_HEIGHT: f32 = 1.8;
const EYE: f32 = 1.62;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Player {
    pub pos: Vec3,
    pub vel: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub on_ground: bool,
    pub flying: bool,
    pub in_water: bool,
}

impl Default for Player {
    fn default() -> Self {
        Self {
            pos: Vec3::new(0.0, 100.0, 0.0),
            vel: Vec3::ZERO,
            yaw: 0.0,
            pitch: 0.0,
            on_ground: false,
            flying: false,
            in_water: false,
        }
    }
}

impl Player {
    pub fn eye(&self) -> Vec3 {
        self.pos + Vec3::Y * EYE
    }
    pub fn forward(&self) -> Vec3 {
        let cp = self.pitch.cos();
        Vec3::new(self.yaw.sin() * cp, self.pitch.sin(), -self.yaw.cos() * cp).normalize()
    }
    pub fn camera(&self, aspect: f32) -> Camera {
        Camera {
            pos: self.eye(),
            yaw: self.yaw,
            pitch: self.pitch,
            aspect,
            ..Default::default()
        }
    }
}

pub struct ChunkData {
    pub voxels: Box<[u16; CHUNK_VOLUME]>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MobSpecies {
    Uro,        // pasivo, lana
    Jabali,
    ZorroBruma,
    Acechador,  // hostil de noche
    PenitenteErrante,
}

impl MobSpecies {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Uro => "Uro lanudo",
            Self::Jabali => "Jabalí moteado",
            Self::ZorroBruma => "Zorro de bruma",
            Self::Acechador => "Acechador",
            Self::PenitenteErrante => "Penitente errante",
        }
    }
    pub fn color(&self) -> [f32; 4] {
        match self {
            Self::Uro => [0.85, 0.82, 0.78, 1.0],
            Self::Jabali => [0.55, 0.40, 0.30, 1.0],
            Self::ZorroBruma => [0.75, 0.55, 0.40, 1.0],
            Self::Acechador => [0.25, 0.10, 0.30, 1.0],
            Self::PenitenteErrante => [0.80, 0.30, 0.25, 1.0],
        }
    }
    pub fn size(&self) -> [f32; 3] {
        match self {
            Self::Uro => [0.9, 1.3, 1.6],
            Self::Jabali => [0.8, 0.9, 1.2],
            Self::ZorroBruma => [0.6, 0.7, 1.0],
            Self::Acechador => [0.8, 1.8, 0.8],
            Self::PenitenteErrante => [0.7, 1.8, 0.7],
        }
    }
    pub fn hostile(&self) -> bool {
        matches!(self, Self::Acechador)
    }
}

#[derive(Debug, Clone)]
pub struct MobEntity {
    pub species: MobSpecies,
    pub pos: Vec3,
    pub vel: Vec3,
    pub yaw: f32,
    pub health: f32,
    pub wander_timer: f32,
    pub on_ground: bool,
    pub id: u32,
}

pub struct NpcEntity {
    pub kind: &'static str,
    pub pos: Vec3,
    pub name: String,
    pub memory: NpcMemory,
    pub tree: DialogueTree,
}

#[derive(Debug, Clone)]
pub struct DialogueSession {
    pub npc_index: usize,
    pub node: &'static str,
    pub history: Vec<(String, String)>,
    /// Texto del LLM en curso (streaming).
    pub ai_pending: bool,
    pub ai_request: Option<u64>,
    pub ai_text: String,
    pub input: String,
}

pub struct BreakTarget {
    pub pos: IVec3,
    pub progress: f32,
}

pub struct GameSession {
    pub seed: u64,
    pub config: WorldConfig,
    pub player: Player,
    pub inventory: Inventory,
    pub karma: Karma,
    pub quests: QuestLog,
    pub respawn: Respawn,
    pub chunks: HashMap<IVec3, ChunkData>,
    pub time_of_day: f32,
    pub weather: f32,
    pub weather_target: f32,
    pub dimension: Dimension,
    gen_tx: flume::Sender<(IVec3, Box<[u16; CHUNK_VOLUME]>, MeshData)>,
    gen_rx: flume::Receiver<(IVec3, Box<[u16; CHUNK_VOLUME]>, MeshData)>,
    pending: HashSet<IVec3>,
    pub npcs: Vec<NpcEntity>,
    pub mobs: Vec<MobEntity>,
    pub next_mob_id: u32,
    mob_spawn_timer: f32,
    pub dialogue: Option<DialogueSession>,
    pub ai: AiService,
    pub audio: AudioEngine,
    pub break_target: Option<BreakTarget>,
    pub crafted: Vec<u16>,
    pub talked: Vec<String>,
    pub dirty_pending: Vec<IVec3>,
    pub stats_chunks: usize,
    pub message: Option<(String, f32)>,
    pub dimension_switch_flash: f32,
    pub edits: HashMap<[i32; 3], u16>,
    pub pulso: PulsoWorld,
    pub health: f32,
    pub hunger: f32,
    pub step_timer: f32,
    pub fall_start: f32,
    pub pulso_accum: f32,
    pub tick_count: u64,
}

impl GameSession {
    pub fn new(seed: u64, config: WorldConfig, settings: &rivaren_gameplay::Settings) -> Self {
        let (gen_tx, gen_rx) = flume::unbounded();
        let ai = AiService::new(rivaren_ai::AiConfig {
            mode: match settings.game.ai_mode {
                1 => rivaren_ai::AiMode::Local,
                2 => rivaren_ai::AiMode::Cloud,
                _ => rivaren_ai::AiMode::Rules,
            },
            url: settings.game.ai_url.clone(),
            model: settings.game.ai_model.clone(),
            api_key_env: settings.game.ai_key_env.clone(),
            ..Default::default()
        });
        let mut audio = AudioEngine::new();
        audio.set_volumes(
            settings.audio.master,
            settings.audio.music,
            settings.audio.sfx,
        );
        audio.start_music(Soundscape::Pradera);
        let start_dim = config.start_dimension;
        let spawn = Self::find_spawn(seed, start_dim);
        Self {
            seed,
            config,
            player: Player {
                pos: spawn,
                ..Default::default()
            },
            inventory: {
                let mut inv = Inventory::default();
                inv.add(ItemStack::with_count(50, 1));
                inv.add(ItemStack::with_count(4, 32));
                inv.add(ItemStack::with_count(1, 32));
                inv.add(ItemStack::with_count(45, 16));
                inv.add(ItemStack::with_count(70, 12));
                inv.add(ItemStack::with_count(71, 2));
                inv.add(ItemStack::with_count(74, 2));
                inv.add(ItemStack::with_count(30, 8));
                inv.add(ItemStack::with_count(42, 2));
                inv.add(ItemStack::with_count(61, 5));
                // Componentes de Pulso para probar el sistema.
                inv.add(ItemStack::with_count(80, 32));
                inv.add(ItemStack::with_count(81, 8));
                inv.add(ItemStack::with_count(82, 4));
                inv.add(ItemStack::with_count(83, 4));
                inv.add(ItemStack::with_count(84, 2));
                inv.add(ItemStack::with_count(85, 2));
                inv.add(ItemStack::with_count(86, 4));
                // Bloques de mods (si hay).
                for (id, _) in rivaren_gameplay::items::modded_items() {
                    inv.add(ItemStack::with_count(*id, 16));
                }
                inv
            },
            karma: Karma::default(),
            quests: QuestLog::campaign(),
            respawn: Respawn {
                bed: None,
                anchor: None,
                origin: [spawn.x as i32, spawn.y as i32, spawn.z as i32],
            },
            chunks: HashMap::new(),
            time_of_day: 0.32,
            weather: 0.0,
            weather_target: 0.0,
            dimension: start_dim,
            gen_tx,
            gen_rx,
            pending: HashSet::new(),
            npcs: Vec::new(),
            mobs: Vec::new(),
            next_mob_id: 1,
            mob_spawn_timer: 2.0,
            dialogue: None,
            ai,
            audio,
            break_target: None,
            crafted: Vec::new(),
            talked: Vec::new(),
            dirty_pending: Vec::new(),
            stats_chunks: 0,
            message: Some((
                "Bienvenido a RIVAREN. WASD mover, ratón mirar, F hablar, E inventario, R diario."
                    .into(),
                8.0,
            )),
            dimension_switch_flash: 0.0,
            edits: HashMap::new(),
            pulso: PulsoWorld::new(),
            health: 20.0,
            hunger: 20.0,
            step_timer: 0.0,
            fall_start: 0.0,
            pulso_accum: 0.0,
            tick_count: 0,
        }
    }

    pub fn find_spawn(seed: u64, dim: Dimension) -> Vec3 {
        match dim {
            Dimension::Tierra => {
                // Busca tierra firme sobre el nivel del mar en espiral.
                for r in 0i32..24 {
                    for dz in -r..=r {
                        for dx in -r..=r {
                            if dx.abs() != r && dz.abs() != r {
                                continue;
                            }
                            let x = dx as f32 * 32.0 + 0.5;
                            let z = dz as f32 * 32.0 + 0.5;
                            let h = rivaren_world::sdf::surface_height(seed, x, z);
                            if (63.0..150.0).contains(&h) {
                                return Vec3::new(x, h + 2.0, z);
                            }
                        }
                    }
                }
                let h = rivaren_world::sdf::surface_height(seed, 0.5, 0.5).max(66.0);
                Vec3::new(0.5, h + 2.0, 0.5)
            }
            Dimension::Cielo => Vec3::new(0.5, 140.0, 0.5),
            Dimension::Infierno => Vec3::new(0.5, 40.0, 0.5),
        }
    }

    // ── Streaming ────────────────────────────────────────────────

    pub fn update_streaming(&mut self, renderer: &mut Renderer) {
        let pc = IVec3::new(
            (self.player.pos.x / CHUNK as f32).floor() as i32,
            (self.player.pos.y / CHUNK as f32).floor() as i32,
            (self.player.pos.z / CHUNK as f32).floor() as i32,
        );
        let r = self.config.render_distance.clamp(2, 12);
        // Genera peticiones.
        for dz in -r..=r {
            for dx in -r..=r {
                for dy in -1..=2 {
                    let key = pc + IVec3::new(dx, dy, dz);
                    if self.chunks.contains_key(&key) || self.pending.contains(&key) {
                        continue;
                    }
                    if self.pending.len() > 96 {
                        break;
                    }
                    self.pending.insert(key);
                    let seed = self.seed;
                    let tx = self.gen_tx.clone();
                    let dim = self.dimension;
                    rayon::spawn(move || {
                        let cancel = AtomicBool::new(false);
                        let b = match dim {
                            Dimension::Tierra => {
                                rivaren_world::pipeline::generate_chunk(seed, key, &cancel)
                            }
                            Dimension::Cielo => generate_sky_chunk(seed, key),
                            Dimension::Infierno => generate_hell_chunk(seed, key),
                        };
                        let light = bake_lighting(&b.voxels);
                        let mut mesh = MeshData::default();
                        greedy_mesh_lit(&b.voxels, &light, &mut mesh);
                        let _ = tx.send((key, b.voxels, mesh));
                    });
                }
            }
        }
        // Sube resultados (presupuesto por frame).
        let mut uploaded = 0;
        while let Ok((key, mut voxels, mut mesh)) = self.gen_rx.try_recv() {
            self.pending.remove(&key);
            // Reaplica ediciones locales del jugador (delta log) sobre el chunk nuevo.
            let base = IVec3::new(key.x * 32, key.y * 32, key.z * 32);
            let mut edited = false;
            for (p, v) in &self.edits {
                let d = IVec3::new(p[0] - base.x, p[1] - base.y, p[2] - base.z);
                if d.x >= 0 && d.y >= 0 && d.z >= 0 && d.x < 32 && d.y < 32 && d.z < 32 {
                    let idx = (d.y as usize * 32 + d.z as usize) * 32 + d.x as usize;
                    if voxels[idx] != *v {
                        voxels[idx] = *v;
                        edited = true;
                    }
                }
            }
            if edited {
                let light = bake_lighting(&voxels);
                mesh = MeshData::default();
                greedy_mesh_lit(&voxels, &light, &mut mesh);
            }
            if uploaded < 6 {
                renderer.upload_chunk(key, &mesh);
                self.chunks.insert(key, ChunkData { voxels });
                uploaded += 1;
            } else {
                // Reenvía al final de la cola para el próximo frame.
                let _ = self.gen_tx.send((key, voxels, mesh));
                break;
            }
        }
        // Re-meshea chunks con cambios de Pulso (presupuesto por frame).
        let pending = std::mem::take(&mut self.dirty_pending);
        let mut remeshed = 0;
        let mut keep = Vec::new();
        for key in pending {
            if remeshed < 2 {
                if self.chunks.contains_key(&key) {
                    self.remesh(key, renderer);
                    remeshed += 1;
                }
            } else {
                keep.push(key);
            }
        }
        self.dirty_pending = keep;
        // Evicta lejanos.
        let mut to_remove = Vec::new();
        for key in self.chunks.keys() {
            let d = (key.x - pc.x).abs().max((key.z - pc.z).abs());
            let dy = (key.y - pc.y).abs();
            if d > r + 2 || dy > 4 {
                to_remove.push(*key);
            }
        }
        for key in to_remove {
            self.chunks.remove(&key);
            renderer.remove_chunk(key);
        }
        self.stats_chunks = self.chunks.len();
    }

    // ── Acceso a bloques ─────────────────────────────────────────

    #[inline]
    pub fn block_at(&self, x: i32, y: i32, z: i32) -> u16 {
        let cx = x.div_euclid(CHUNK);
        let cy = y.div_euclid(CHUNK);
        let cz = z.div_euclid(CHUNK);
        let key = IVec3::new(cx, cy, cz);
        match self.chunks.get(&key) {
            Some(c) => {
                let lx = x.rem_euclid(CHUNK) as usize;
                let ly = y.rem_euclid(CHUNK) as usize;
                let lz = z.rem_euclid(CHUNK) as usize;
                c.voxels[(ly * 32 + lz) * 32 + lx]
            }
            None => 0,
        }
    }

    pub fn set_block(&mut self, x: i32, y: i32, z: i32, v: u16) -> bool {
        let key = IVec3::new(
            x.div_euclid(CHUNK),
            y.div_euclid(CHUNK),
            z.div_euclid(CHUNK),
        );
        let Some(c) = self.chunks.get_mut(&key) else {
            return false;
        };
        let lx = x.rem_euclid(CHUNK) as usize;
        let ly = y.rem_euclid(CHUNK) as usize;
        let lz = z.rem_euclid(CHUNK) as usize;
        c.voxels[(ly * 32 + lz) * 32 + lx] = v;
        self.edits.insert([x, y, z], v);
        true
    }

    /// Re-meshea un chunk (tras edición).
    pub fn remesh(&mut self, key: IVec3, renderer: &mut Renderer) {
        let Some(c) = self.chunks.get(&key) else {
            return;
        };
        let light = bake_lighting(&c.voxels);
        let mut mesh = MeshData::default();
        greedy_mesh_lit(&c.voxels, &light, &mut mesh);
        renderer.upload_chunk(key, &mesh);
    }

    /// Tick del sistema Pulso: aplica cambios de estado a los bloques.
    pub fn tick_pulso(&mut self) -> usize {
        let changes = self.pulso.tick_step(256);
        let mut applied = 0;
        for (pos, power) in changes {
            let Some(node) = self.pulso.nodes.get(&pos).copied() else {
                continue;
            };
            let want = pulso_block_for_state(node.kind, power);
            if want == 0 {
                continue;
            }
            if self.block_at(pos[0], pos[1], pos[2]) != want {
                self.set_block(pos[0], pos[1], pos[2], want);
                applied += 1;
                let keys = self.mark_dirty(pos[0], pos[1], pos[2]);
                self.dirty_pending.extend(keys);
            }
        }
        applied
    }

    /// Marca dirty el chunk del bloque + vecinos si toca borde.
    pub fn mark_dirty(&mut self, x: i32, y: i32, z: i32) -> Vec<IVec3> {
        let key = IVec3::new(
            x.div_euclid(CHUNK),
            y.div_euclid(CHUNK),
            z.div_euclid(CHUNK),
        );
        let mut keys = vec![key];
        let lx = x.rem_euclid(CHUNK);
        let ly = y.rem_euclid(CHUNK);
        let lz = z.rem_euclid(CHUNK);
        if lx == 0 {
            keys.push(key + IVec3::new(-1, 0, 0));
        }
        if lx == 31 {
            keys.push(key + IVec3::new(1, 0, 0));
        }
        if ly == 0 {
            keys.push(key + IVec3::new(0, -1, 0));
        }
        if ly == 31 {
            keys.push(key + IVec3::new(0, 1, 0));
        }
        if lz == 0 {
            keys.push(key + IVec3::new(0, 0, -1));
        }
        if lz == 31 {
            keys.push(key + IVec3::new(0, 0, 1));
        }
        keys
    }

    // ── Física del jugador ───────────────────────────────────────

    pub fn tick_player(&mut self, input: &PlayerInput, dt: f32) {
        let speed = if self.player.flying {
            14.0
        } else if input.sprint {
            7.0
        } else {
            4.6
        };
        let mut wish = Vec3::ZERO;
        let fwd = Vec3::new(self.player.yaw.sin(), 0.0, -self.player.yaw.cos());
        let right = Vec3::new(fwd.z, 0.0, -fwd.x);
        if input.forward {
            wish += fwd;
        }
        if input.back {
            wish -= fwd;
        }
        if input.left {
            wish -= right;
        }
        if input.right {
            wish += right;
        }
        if wish.length_squared() > 0.0 {
            wish = wish.normalize() * speed;
        }
        let in_water = self.block_at(
            self.player.pos.x.floor() as i32,
            self.player.pos.y.floor() as i32,
            self.player.pos.z.floor() as i32,
        ) == 20;
        self.player.in_water = in_water;
        if self.player.flying {
            self.player.vel.y = if input.jump {
                8.0
            } else if input.sneak {
                -8.0
            } else {
                0.0
            };
            self.player.vel.x = wish.x;
            self.player.vel.z = wish.z;
        } else {
            let accel = if self.player.on_ground { 0.35 } else { 0.12 };
            self.player.vel.x += (wish.x - self.player.vel.x) * accel;
            self.player.vel.z += (wish.z - self.player.vel.z) * accel;
            if in_water {
                self.player.vel.y = self.player.vel.y * 0.8 - 1.2 * dt;
                if input.jump {
                    self.player.vel.y = 3.2;
                }
            } else {
                self.player.vel.y -= 32.0 * dt;
                if input.jump && self.player.on_ground {
                    self.player.vel.y = 9.0;
                    self.player.on_ground = false;
                }
            }
        }
        // Movimiento con colisiones por eje.
        let half = Vec3::new(PLAYER_HALF, PLAYER_HEIGHT * 0.5, PLAYER_HALF);
        let center = self.player.pos + Vec3::Y * (PLAYER_HEIGHT * 0.5);
        let delta = self.player.vel * dt;
        let mut new_center = center;
        self.player.on_ground = false;
        for axis in 0..3 {
            let mut trial = new_center;
            trial[axis] += delta[axis];
            if !self.aabb_hits(trial, half) {
                new_center = trial;
            } else {
                if axis == 1 && delta[axis] < 0.0 {
                    self.player.on_ground = true;
                }
                self.player.vel[axis] = 0.0;
                // Intenta step-up (1 bloque).
                if axis != 1 {
                    let mut step = new_center;
                    step.y += 1.02;
                    if !self.aabb_hits(step, half) && self.player.on_ground {
                        let mut full = step;
                        full[axis] += delta[axis];
                        if !self.aabb_hits(full, half) {
                            new_center = full;
                        }
                    }
                }
            }
        }
        if self.player.vel.y < -0.1 {
            self.fall_start = self.fall_start.max(self.player.pos.y);
        }
        let was_air = !self.player.on_ground;
        self.player.pos = new_center - Vec3::Y * (PLAYER_HEIGHT * 0.5);
        if self.player.on_ground && was_air {
            let fall = (self.fall_start - self.player.pos.y).max(0.0);
            self.fall_start = self.player.pos.y;
            if fall > 4.0 && !self.player.flying {
                let dmg = ((fall - 4.0) * 1.2).min(18.0);
                self.health = (self.health - dmg).max(0.0);
                self.audio.play(Sfx::Hurt, 0.9, 0.0);
            }
        }
        // Vacío infinito: si cae demasiado, respawn.
        if self.player.pos.y < -320.0 {
            self.player.pos = Self::find_spawn(self.seed, self.dimension);
            self.player.vel = Vec3::ZERO;
            self.damage_player(4.0, "El vacío te devuelve");
        }
    }

    fn aabb_hits(&self, center: Vec3, half: Vec3) -> bool {
        let min = center - half;
        let max = center + half;
        let x0 = (min.x - 0.5).floor() as i32;
        let x1 = (max.x + 0.5).ceil() as i32;
        let y0 = min.y.floor() as i32;
        let y1 = max.y.ceil() as i32;
        let z0 = (min.z - 0.5).floor() as i32;
        let z1 = (max.z + 0.5).ceil() as i32;
        for y in y0..=y1 {
            for z in z0..=z1 {
                for x in x0..=x1 {
                    let b = self.block_at(x, y, z);
                    if b != AIR && b != 20 {
                        return true;
                    }
                }
            }
        }
        false
    }

    // ── Raycast e interacción ────────────────────────────────────

    pub fn raycast(&self, max_dist: f32) -> Option<(IVec3, IVec3)> {
        let origin = self.player.eye();
        let dir = self.player.forward();
        let mut pos = origin.floor().as_ivec3();
        let step = IVec3::new(
            if dir.x > 0.0 { 1 } else { -1 },
            if dir.y > 0.0 { 1 } else { -1 },
            if dir.z > 0.0 { 1 } else { -1 },
        );
        let t_delta = Vec3::new(
            (1.0 / dir.x).abs(),
            (1.0 / dir.y).abs(),
            (1.0 / dir.z).abs(),
        );
        let mut t_max = Vec3::new(
            if dir.x > 0.0 {
                (pos.x as f32 + 1.0 - origin.x) / dir.x
            } else {
                (pos.x as f32 - origin.x) / dir.x
            },
            if dir.y > 0.0 {
                (pos.y as f32 + 1.0 - origin.y) / dir.y
            } else {
                (pos.y as f32 - origin.y) / dir.y
            },
            if dir.z > 0.0 {
                (pos.z as f32 + 1.0 - origin.z) / dir.z
            } else {
                (pos.z as f32 - origin.z) / dir.z
            },
        );
        let mut normal = IVec3::ZERO;
        let mut t = 0.0;
        while t <= max_dist {
            let b = self.block_at(pos.x, pos.y, pos.z);
            if b != AIR && b != 20 {
                return Some((pos, normal));
            }
            if t_max.x < t_max.y && t_max.x < t_max.z {
                pos.x += step.x;
                t = t_max.x;
                t_max.x += t_delta.x;
                normal = IVec3::new(-step.x, 0, 0);
            } else if t_max.y < t_max.z {
                pos.y += step.y;
                t = t_max.y;
                t_max.y += t_delta.y;
                normal = IVec3::new(0, -step.y, 0);
            } else {
                pos.z += step.z;
                t = t_max.z;
                t_max.z += t_delta.z;
                normal = IVec3::new(0, 0, -step.z);
            }
        }
        None
    }

    /// Rompe el bloque apuntado (con progreso en supervivencia).
    pub fn try_break(&mut self, dt: f32, renderer: &mut Renderer) {
        let Some((pos, _)) = self.raycast(5.5) else {
            self.break_target = None;
            return;
        };
        let block = self.block_at(pos.x, pos.y, pos.z);
        let creative = matches!(self.config.gamemode, rivaren_gameplay::Gamemode::Creative);
        let hardness = match block {
            4 | 21 | 30 => 1.5,
            2 | 1 | 6 => 0.6,
            _ => 0.4,
        };
        let tool_bonus = match self.inventory.held() {
            Some(s) => {
                let d = item_def(s.id);
                if matches!(d.tool, rivaren_gameplay::ToolKind::Pickaxe) && matches!(block, 4 | 21 | 30)
                {
                    4.0
                } else {
                    1.0
                }
            }
            None => 1.0,
        };
        let speed = if creative {
            20.0
        } else {
            tool_bonus / hardness
        };
        let progress = self.break_target.as_ref().map(|b| b.progress).unwrap_or(0.0);
        let progress = if self.break_target.as_ref().map(|b| b.pos) == Some(pos) {
            progress + dt * speed
        } else {
            dt * speed
        };
        if progress >= 1.0 {
            self.set_block(pos.x, pos.y, pos.z, AIR);
            self.pulso.remove([pos.x, pos.y, pos.z]);
            let item = item_for_block(block);
            if item != 0 {
                self.inventory.add(ItemStack::new(item));
                if item == 4 {
                    self.crafted.push(4);
                }
            }
            if block == 30 {
                self.karma.add(KarmaAxis::Sabiduria, 1.0);
            }
            for key in self.mark_dirty(pos.x, pos.y, pos.z) {
                if self.chunks.contains_key(&key) {
                    self.remesh(key, renderer);
                }
            }
            let listener = self.player.eye().to_array();
            self.audio
                .play_at(Sfx::Break, pos.as_vec3().to_array(), listener, self.player.forward().to_array());
            self.break_target = None;
        } else {
            self.break_target = Some(BreakTarget { pos, progress });
        }
    }

    pub fn try_place(&mut self, renderer: &mut Renderer) {
        let Some((pos, normal)) = self.raycast(5.5) else {
            return;
        };
        let Some(held) = self.inventory.held() else {
            return;
        };
        let def = item_def(held.id);
        if def.places_block == 0 {
            return;
        }
        let target = pos + normal;
        if self.block_at(target.x, target.y, target.z) != AIR {
            return;
        }
        // No colocar dentro del jugador.
        let center = self.player.pos + Vec3::Y * (PLAYER_HEIGHT * 0.5);
        let half = Vec3::new(PLAYER_HALF, PLAYER_HEIGHT * 0.5, PLAYER_HALF);
        let bmin = target.as_vec3() - Vec3::splat(0.0);
        let bmax = bmin + Vec3::splat(1.0);
        let pmin = center - half;
        let pmax = center + half;
        let overlaps = pmin.x < bmax.x && pmax.x > bmin.x && pmin.y < bmax.y && pmax.y > bmin.y && pmin.z < bmax.z && pmax.z > bmin.z;
        if overlaps {
            return;
        }
        self.set_block(target.x, target.y, target.z, def.places_block);
        if let Some(kind) = pulso_kind_for_block(def.places_block) {
            self.pulso
                .place([target.x, target.y, target.z], kind);
        }
        if let Some(s) = self.inventory.held_mut() {
            s.count = s.count.saturating_sub(1);
            if s.count == 0 {
                self.inventory.slots[self.inventory.selected] = None;
            }
        }
        for key in self.mark_dirty(target.x, target.y, target.z) {
            if self.chunks.contains_key(&key) {
                self.remesh(key, renderer);
            }
        }
        let listener = self.player.eye().to_array();
        self.audio.play_at(
            Sfx::Place,
            target.as_vec3().to_array(),
            listener,
            self.player.forward().to_array(),
        );
    }

    pub fn interact(&mut self) {
        if let Some((pos, _)) = self.raycast(5.5) {
            let b = self.block_at(pos.x, pos.y, pos.z);
            // Pulso: palanca / botón.
            if matches!(b, 82 | 90 | 83) {
                self.pulso.toggle([pos.x, pos.y, pos.z]);
                self.tick_pulso();
                self.audio.play(Sfx::Click, 0.7, 0.0);
                return;
            }
            // Portal → cambio de dimensión.
            if b == 30 {
                self.use_portal();
                return;
            }
        }
        // NPC cercano.
        let mut best: Option<(usize, f32)> = None;
        for (i, npc) in self.npcs.iter().enumerate() {
            let d = npc.pos.distance(self.player.eye());
            if d < 4.0 && best.map(|(_, bd)| d < bd).unwrap_or(true) {
                best = Some((i, d));
            }
        }
        if let Some((i, _)) = best {
            self.start_dialogue(i);
        }
    }

    pub fn use_portal(&mut self) {
        match self.dimension {
            Dimension::Tierra => {
                let has = self.inventory.consume(43, 1);
                if !has {
                    self.message = Some(("Necesitas un Núcleo de Penitente para el Descenso.".into(), 4.0));
                    return;
                }
                self.switch_dimension(Dimension::Infierno);
            }
            Dimension::Infierno => {
                self.switch_dimension(Dimension::Tierra);
                self.message = Some(("Regresas a la Tierra; tu hogar sigue intacto.".into(), 5.0));
                self.karma.add(KarmaAxis::Compasion, 5.0);
            }
            Dimension::Cielo => {
                self.switch_dimension(Dimension::Tierra);
                self.message = Some(("Desciendes a un lugar lejano de la Tierra.".into(), 5.0));
            }
        }
    }

    pub fn switch_dimension(&mut self, dim: Dimension) {
        self.dimension = dim;
        self.chunks.clear();
        self.pending.clear();
        self.npcs.clear();
        self.player.pos = Self::find_spawn(self.seed, dim);
        self.player.vel = Vec3::ZERO;
        self.dimension_switch_flash = 1.0;
        if dim == Dimension::Infierno {
            self.spawn_npc("Penitente", self.player.pos + Vec3::new(3.0, 0.0, 0.0));
            self.audio.start_music(Soundscape::Infierno);
        } else if dim == Dimension::Cielo {
            self.spawn_npc("Iluminado", self.player.pos + Vec3::new(3.0, 0.0, 0.0));
            self.audio.start_music(Soundscape::Cielo);
        } else {
            self.audio.start_music(Soundscape::Pradera);
        }
        self.audio.play(Sfx::Portal, 0.9, 0.0);
        self.message = Some((
            match dim {
                Dimension::Tierra => "Tierra de los Vivos",
                Dimension::Cielo => "Cielo: la dimensión de la sabiduría",
                Dimension::Infierno => "Infierno: la dimensión de la prueba",
            }
            .into(),
            5.0,
        ));
    }

    pub fn spawn_npc(&mut self, kind: &'static str, pos: Vec3) {
        let name = match kind {
            "Iluminado" => "Iluminado Sereno",
            "Penitente" => "Penitente Errante",
            _ => "Aldeano",
        };
        self.npcs.push(NpcEntity {
            kind,
            pos,
            name: name.to_string(),
            memory: NpcMemory {
                npc: kind.to_string(),
                ..Default::default()
            },
            tree: tree_for(kind),
        });
    }

    pub fn damage_player(&mut self, amount: f32, cause: &str) {
        let _ = amount;
        self.message = Some((cause.to_string(), 3.0));
        self.audio.play(Sfx::Hurt, 0.8, 0.0);
    }

    // ── Diálogo + IA ─────────────────────────────────────────────

    pub fn start_dialogue(&mut self, npc_index: usize) {
        let Some(npc) = self.npcs.get(npc_index) else {
            return;
        };
        let node = npc.tree.root;
        self.npcs[npc_index].memory.times_talked += 1;
        self.dialogue = Some(DialogueSession {
            npc_index,
            node,
            history: Vec::new(),
            ai_pending: false,
            ai_request: None,
            ai_text: String::new(),
            input: String::new(),
        });
        let name = self.npcs[npc_index].kind.to_string();
        if !self.talked.iter().any(|t| t == &name) {
            self.talked.push(name.clone());
            if name == "Penitente" {
                self.karma.add(KarmaAxis::Compasion, 10.0);
            }
            if name == "Iluminado" {
                self.karma.add(KarmaAxis::Sabiduria, 10.0);
            }
        }
        self.audio.play(Sfx::UiOpen, 0.6, 0.0);
    }

    pub fn dialogue_choose(&mut self, option_index: usize) {
        let Some(dlg) = self.dialogue.as_ref() else {
            return;
        };
        let npc_index = dlg.npc_index;
        let node_id = dlg.node;
        let Some(npc) = self.npcs.get(npc_index) else {
            return;
        };
        let Some(node) = npc.tree.node(node_id) else {
            return;
        };
        let Some(opt) = node.options.get(option_index) else {
            return;
        };
        let action = opt.action;
        let next = opt.next;
        let opt_label = opt.label.to_string();
        if let Some(d) = self.dialogue.as_mut() {
            d.history.push((npc.name.clone(), node.text.to_string()));
            d.history.push(("Tú".into(), opt_label));
        }
        match action {
            rivaren_gameplay::DialogueAction::StartQuest(q) => {
                self.quests.start(q);
                self.quests.unlock_available();
                self.message = Some((format!("Nueva misión: {q}"), 4.0));
            }
            rivaren_gameplay::DialogueAction::CompleteQuest => {
                self.karma.add(KarmaAxis::Justicia, 8.0);
                self.inventory.add(ItemStack::with_count(41, 3));
                self.message = Some(("El Penitente te entrega cristales resonantes.".into(), 5.0));
            }
            rivaren_gameplay::DialogueAction::AcceptBlessing => {
                self.inventory.add(ItemStack::with_count(42, 2));
                self.karma.add(KarmaAxis::Compasion, 5.0);
            }
            rivaren_gameplay::DialogueAction::RequestAid | rivaren_gameplay::DialogueAction::None => {}
        }
        match next {
            Some(id) => {
                if let Some(d) = self.dialogue.as_mut() {
                    d.node = id;
                }
            }
            None => self.close_dialogue(),
        }
    }

    pub fn close_dialogue(&mut self) {
        self.dialogue = None;
        self.audio.play(Sfx::UiClose, 0.5, 0.0);
    }

    /// Envía una pregunta libre al LLM (modo local/nube) o a reglas.
    pub fn dialogue_ask(&mut self, text: String) {
        let Some(dlg) = self.dialogue.as_ref() else {
            return;
        };
        let npc_index = dlg.npc_index;
        let Some(npc) = self.npcs.get(npc_index) else {
            return;
        };
        let ctx = rivaren_ai::NpcContext {
            npc_name: npc.name.clone(),
            npc_role: npc.kind.to_string(),
            personality: match npc.kind {
                "Iluminado" => "sereno y enigmático",
                "Penitente" => "quebrado pero honesto",
                _ => "curioso",
            }
            .into(),
            world_lore: "La civilización perdida construyó portales-espejo entre la Tierra, el Cielo y el Infierno; el Pulso movía sus máquinas.".into(),
            compasion: self.karma.compasion,
            justicia: self.karma.justicia,
            sabiduria: self.karma.sabiduria,
            dimension: match self.dimension {
                Dimension::Tierra => "Tierra de los Vivos",
                Dimension::Cielo => "Cielo",
                Dimension::Infierno => "Infierno",
            },
            time_of_day: if (0.25..0.75).contains(&self.time_of_day) {
                "día"
            } else {
                "noche"
            },
            weather: if self.weather > 0.5 { "tormenta" } else { "despejado" },
            nearby_village: None,
            memory: npc.memory.clone(),
        };
        let messages = rivaren_ai::build_messages(&ctx, &text);
        let id = self.ai.ask(messages);
        if let Some(d) = self.dialogue.as_mut() {
            d.ai_pending = true;
            d.ai_request = Some(id);
            d.ai_text.clear();
        }
    }

    pub fn poll_ai(&mut self) {
        for ev in self.ai.poll() {
            match ev {
                AiEvent::Token { id, text } => {
                    if let Some(d) = self.dialogue.as_mut() {
                        if d.ai_request == Some(id) {
                            d.ai_text.push_str(&text);
                        }
                    }
                }
                AiEvent::Done { id, full } => {
                    let npc_name = self
                        .dialogue
                        .as_ref()
                        .and_then(|d| self.npcs.get(d.npc_index))
                        .map(|n| n.name.clone());
                    if let Some(d) = self.dialogue.as_mut() {
                        if d.ai_request == Some(id) {
                            d.ai_pending = false;
                            d.ai_request = None;
                            let q = d.input.clone();
                            d.input.clear();
                            if let Some(name) = npc_name {
                                d.history.push(("Tú".into(), q));
                                d.history.push((name, full.clone()));
                            }
                        }
                    }
                    // El NPC recuerda el tema.
                    if let Some(d) = self.dialogue.as_ref() {
                        let idx = d.npc_index;
                        let fact = format!("El jugador preguntó: {}", full.chars().take(60).collect::<String>());
                        if let Some(npc) = self.npcs.get_mut(idx) {
                            npc.memory.remember(&fact);
                        }
                    }
                }
                AiEvent::Error { id, message } => {
                    tracing::warn!("IA error: {message}");
                    if let Some(d) = self.dialogue.as_mut() {
                        if d.ai_request == Some(id) {
                            d.ai_pending = false;
                            d.ai_request = None;
                            d.ai_text = "(El NPC guarda silencio un momento...)".into();
                        }
                    }
                }
            }
        }
    }

    // ── Tick global ──────────────────────────────────────────────

    /// IA de mobs: deambulan, caen y huyen/atacan (hostiles).
    pub fn tick_mobs(&mut self, dt: f32) {
        let player = self.player.pos;
        let player_eye = self.player.eye();
        let tick_count = self.tick_count;
        let time = self.time_of_day;
        let mut hits = 0.0f32;
        let mut despawn = Vec::new();
        // Snapshot local (caja de 96³ alrededor del jugador) para colisión sin préstamos.
        let cx = player.x.floor() as i32;
        let cy = player.y.floor() as i32;
        let cz = player.z.floor() as i32;
        let mut solid_map: HashSet<(i32, i32, i32)> = HashSet::new();
        let r = 48;
        for y in (cy - r..=cy + r).step_by(2) {
            for z in (cz - r..=cz + r).step_by(2) {
                for x in (cx - r..=cx + r).step_by(2) {
                    let b = self.block_at(x, y, z);
                    if b != 0 && b != 20 {
                        solid_map.insert((x, y, z));
                    }
                }
            }
        }
        let solid = |gx: i32, gy: i32, gz: i32| -> bool { solid_map.contains(&(gx, gy, gz)) };
        for (i, mob) in self.mobs.iter_mut().enumerate() {
            let d = mob.pos.distance(player);
            if d > 128.0 {
                despawn.push(i);
                continue;
            }
            // Tick amortizado por distancia.
            let rate = if d < 32.0 { 1 } else { 2 };
            if !tick_count.is_multiple_of(rate) {
                continue;
            }
            mob.wander_timer -= dt * rate as f32;
            if mob.wander_timer <= 0.0 {
                mob.wander_timer = 2.0 + (mob.id as f32 * 0.37).sin().abs() * 4.0;
                let a = (mob.id as f32 * 1.13 + time * 6.28).sin();
                mob.yaw = a * std::f32::consts::TAU;
            }
            let speed = if mob.species.hostile() && d < 16.0 { 3.2 } else { 1.4 };
            let dir = Vec3::new(mob.yaw.sin(), 0.0, -mob.yaw.cos());
            let mut wish = dir * speed;
            // Hostiles se acercan al jugador si está cerca.
            if mob.species.hostile() && d < 24.0 {
                let to = (player - mob.pos).normalize_or_zero();
                wish = to * speed;
                mob.yaw = to.x.atan2(-to.z);
                if d < 1.4 {
                    hits += 6.0 * dt * rate as f32;
                }
            }
            mob.vel.x += (wish.x - mob.vel.x) * 0.2;
            mob.vel.z += (wish.z - mob.vel.z) * 0.2;
            mob.vel.y -= 26.0 * dt * rate as f32;
            // Movimiento con colisión sencilla por ejes.
            let half = Vec3::new(0.35, mob.species.size()[1] * 0.5, 0.35);
            let center = mob.pos + Vec3::Y * half.y;
            for axis in 0..3 {
                let mut t = center;
                t[axis] += mob.vel[axis] * dt * rate as f32;
                if aabb_hits_blocks(&solid, t, half) {
                    if axis == 1 && mob.vel[axis] < 0.0 {
                        mob.on_ground = true;
                    }
                    mob.vel[axis] = 0.0;
                    continue;
                }
                let mut c = center;
                c[axis] = t[axis];
                mob.pos = c - Vec3::Y * half.y;
            }
            let _ = player_eye;
        }
        for i in despawn.into_iter().rev() {
            let mob = self.mobs.remove(i);
            let _ = mob;
        }
        if hits > 0.0 {
            self.health = (self.health - hits).max(0.0);
            if self.health <= 0.0 {
                self.audio.play(Sfx::Hurt, 1.0, 0.0);
            }
        }
    }

    /// Intenta spawnear un mob sobre una columna con superficie sólida.
    pub fn try_spawn_mob(&mut self, species: MobSpecies) -> bool {
        if self.mobs.len() >= 40 {
            return false;
        }
        let a = (self.next_mob_id as f32 * 2.399).sin();
        let b = (self.next_mob_id as f32 * 1.618).cos();
        let dist = 14.0 + (self.next_mob_id as f32 * 0.7).sin().abs() * 18.0;
        let x = (self.player.pos.x + a * dist).floor() as i32;
        let z = (self.player.pos.z + b * dist).floor() as i32;
        let mut y = (self.player.pos.y + 6.0).floor() as i32;
        let floor_limit = (self.player.pos.y as i32 - 64).max(-128);
        // Baja hasta encontrar suelo sólido.
        while y > floor_limit {
            let below = self.block_at(x, y - 1, z);
            let here = self.block_at(x, y, z);
            if below != 0 && below != 20 && (here == 0 || here == 20) {
                break;
            }
            y -= 1;
        }
        if y <= floor_limit {
            // Sin suelo cercano: cae al nivel del jugador si hay aire.
            let fy = (self.player.pos.y + 1.0).floor() as i32;
            if self.block_at(x, fy, z) != 0 {
                return false;
            }
            y = fy;
        }
        self.mobs.push(MobEntity {
            species,
            pos: Vec3::new(x as f32 + 0.5, y as f32, z as f32 + 0.5),
            vel: Vec3::ZERO,
            yaw: 0.0,
            health: 10.0,
            wander_timer: 1.0,
            on_ground: false,
            id: self.next_mob_id,
        });
        self.next_mob_id += 1;
        true
    }

    /// Golpea al mob más cercano en la línea de visión (clic izquierdo).
    pub fn attack_mob(&mut self) -> bool {
        let eye = self.player.eye();
        let fwd = self.player.forward();
        let mut best: Option<(usize, f32)> = None;
        for (i, mob) in self.mobs.iter().enumerate() {
            let to = mob.pos + Vec3::Y * mob.species.size()[1] * 0.5 - eye;
            let dist = to.length();
            if dist > 4.5 {
                continue;
            }
            let dot = to.normalize().dot(fwd);
            if dot > 0.94 && best.map(|(_, bd)| dist < bd).unwrap_or(true) {
                best = Some((i, dist));
            }
        }
        let Some((i, _)) = best else {
            return false;
        };
        let dmg = match self.inventory.held() {
            Some(s) => {
                let td = item_def(s.id);
                if matches!(td.tool, rivaren_gameplay::ToolKind::Blade) {
                    6.0
                } else {
                    2.0
                }
            }
            None => 1.5,
        };
        let mut died = false;
        {
            let mob = &mut self.mobs[i];
            mob.health -= dmg;
            if mob.health <= 0.0 {
                died = true;
            }
        }
        self.audio.play(Sfx::Hurt, 0.6, 0.0);
        if died {
            let mob = self.mobs.remove(i);
            match mob.species {
                MobSpecies::Uro => {
                    self.inventory.add(ItemStack::with_count(40, 2));
                    self.karma.add(KarmaAxis::Compasion, -2.0);
                }
                MobSpecies::Jabali => {
                    self.inventory.add(ItemStack::with_count(60, 2));
                    self.karma.add(KarmaAxis::Compasion, -1.0);
                }
                MobSpecies::ZorroBruma => {
                    self.inventory.add(ItemStack::with_count(45, 1));
                    self.karma.add(KarmaAxis::Sabiduria, 2.0);
                }
                MobSpecies::Acechador => {
                    self.inventory.add(ItemStack::with_count(43, 1));
                    self.karma.add(KarmaAxis::Justicia, 4.0);
                }
                MobSpecies::PenitenteErrante => {
                    self.inventory.add(ItemStack::with_count(41, 1));
                    self.karma.add(KarmaAxis::Compasion, 2.0);
                }
            }
        }
        true
    }

    pub fn tick(&mut self, dt: f32) {
        self.tick_count += 1;
        self.tick_mobs(dt);
        // Spawn de mobs con presupuesto (cada ~4 s uno).
        self.mob_spawn_timer -= dt;
        if self.mob_spawn_timer <= 0.0 {
            self.mob_spawn_timer = 4.0;
            let night = !(0.25..0.75).contains(&self.time_of_day);
            let species = if night && self.tick_count.is_multiple_of(3) {
                MobSpecies::Acechador
            } else {
                match self.next_mob_id % 4 {
                    0 => MobSpecies::Uro,
                    1 => MobSpecies::Jabali,
                    2 => MobSpecies::ZorroBruma,
                    _ => MobSpecies::Jabali,
                }
            };
            if self.dimension == Dimension::Tierra {
                self.try_spawn_mob(species);
            } else if self.dimension == Dimension::Infierno && self.tick_count.is_multiple_of(2) {
                self.try_spawn_mob(MobSpecies::PenitenteErrante);
            }
        }
        // Pulso a 20 TPS.
        self.pulso_accum += dt;
        if self.pulso_accum >= 0.05 {
            self.pulso_accum = 0.0;
            self.tick_pulso();
        }
        // Tiempo: 1 día = 20 min reales.
        self.time_of_day = (self.time_of_day + dt / 1200.0) % 1.0;
        // Clima.
        let h = hash3(self.seed, (self.time_of_day * 1000.0) as i32, 0, 0);
        if h % 1000 < 4 {
            self.weather_target = ((h >> 10) % 100) as f32 / 100.0;
        }
        self.weather += (self.weather_target - self.weather) * dt * 0.05;
        if self.dimension_switch_flash > 0.0 {
            self.dimension_switch_flash = (self.dimension_switch_flash - dt * 1.5).max(0.0);
        }
        if let Some((_, ttl)) = self.message.as_mut() {
            *ttl -= dt;
            if *ttl <= 0.0 {
                self.message = None;
            }
        }
        // Objetivos.
        let ctx = ObjectiveCtx {
            items: (0..80)
                .map(|i| (i as u16, self.inventory.count_of(i as u16)))
                .filter(|(_, c)| *c > 0)
                .collect(),
            crafted: self.crafted.clone(),
            talked: self.talked.clone(),
            dimension: match self.dimension {
                Dimension::Tierra => 0,
                Dimension::Infierno => 1,
                Dimension::Cielo => 2,
            },
            compasion: self.karma.compasion,
            justicia: self.karma.justicia,
        };
        let done = self.quests.evaluate(&ctx);
        for q in done {
            self.message = Some((format!("Misión completada: {q}"), 6.0));
            self.karma.add(KarmaAxis::Sabiduria, 5.0);
            self.audio.play(Sfx::LevelUp, 0.8, 0.0);
            if q == "equilibrio" {
                self.message = Some(("Has restaurado el Equilibrio. RIVAREN respira.".into(), 12.0));
            }
        }
        self.quests.unlock_available();
        self.poll_ai();
    }

    pub fn player_input_forward(&self) -> [f32; 3] {
        self.player.forward().to_array()
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct PlayerInput {
    pub forward: bool,
    pub back: bool,
    pub left: bool,
    pub right: bool,
    pub jump: bool,
    pub sneak: bool,
    pub sprint: bool,
}

/// Colisión AABB contra bloques vía función (evita préstamos de &self).
pub fn aabb_hits_blocks(
    solid: &impl Fn(i32, i32, i32) -> bool,
    center: Vec3,
    half: Vec3,
) -> bool {
    let min = center - half;
    let max = center + half;
    for y in min.y.floor() as i32..=max.y.ceil() as i32 {
        for z in min.z.floor() as i32..=max.z.ceil() as i32 {
            for x in min.x.floor() as i32..=max.x.ceil() as i32 {
                if solid(x, y, z) {
                    return true;
                }
            }
        }
    }
    false
}

/// Mapea bloque colocado a componente de Pulso.
pub fn pulso_kind_for_block(b: u16) -> Option<PulsoKind> {
    match b {
        80 => Some(PulsoKind::Cable),
        81 | 89 => Some(PulsoKind::Torch),
        82 | 90 => Some(PulsoKind::Lever),
        83 => Some(PulsoKind::Button),
        84 => Some(PulsoKind::Gate(GateMode::Or)),
        85 | 88 => Some(PulsoKind::Block),
        _ => None,
    }
}

/// Bloque que representa el estado actual del componente.
pub fn pulso_block_for_state(kind: PulsoKind, power: u8) -> u16 {
    let on = power > 0;
    match kind {
        PulsoKind::Cable => 80,
        PulsoKind::Torch => {
            if on {
                81
            } else {
                89
            }
        }
        PulsoKind::Lever => {
            if on {
                90
            } else {
                82
            }
        }
        PulsoKind::Button => 83,
        PulsoKind::Gate(_) => 84,
        PulsoKind::Lamp => {
            if on {
                87
            } else {
                86
            }
        }
        PulsoKind::Block => 85,
    }
}

// ── Generadores de dimensiones alternativas ──────────────────────────

fn generate_sky_chunk(seed: u64, chunk: IVec3) -> rivaren_world::pipeline::ChunkBuilder {
    use rivaren_world::pipeline::ChunkBuilder;
    let mut b = ChunkBuilder::new(chunk);
    // Islas flotantes de cristal: bandas con ruido.
    let base_y = chunk.y * 32;
    for y in 0..32 {
        let wy = base_y + y as i32;
        if !(90..=220).contains(&wy) {
            continue;
        }
        for z in 0..32 {
            for x in 0..32 {
                let wx = chunk.x * 32 + x as i32;
                let wz = chunk.z * 32 + z as i32;
                let n = rivaren_world::noise::fbm_3d(seed ^ 0x5C1, wx as f32 / 64.0, wy as f32 / 40.0, wz as f32 / 64.0, 3);
                let band = ((wy as f32 - 140.0) / 40.0).sin() * 0.3;
                if n + band > 0.25 {
                    b.voxels[(y * 32 + z) * 32 + x] = 4;
                }
            }
        }
    }
    b
}

fn generate_hell_chunk(seed: u64, chunk: IVec3) -> rivaren_world::pipeline::ChunkBuilder {
    use rivaren_world::pipeline::ChunkBuilder;
    let mut b = ChunkBuilder::new(chunk);
    let base_y = chunk.y * 32;
    for y in 0..32 {
        let wy = base_y + y as i32;
        for z in 0..32 {
            for x in 0..32 {
                let wx = chunk.x * 32 + x as i32;
                let wz = chunk.z * 32 + z as i32;
                let n = rivaren_world::noise::fbm_3d(seed ^ 0x4E11, wx as f32 / 96.0, wy as f32 / 48.0, wz as f32 / 96.0, 3);
                if wy < 30 + (n * 20.0) as i32 {
                    b.voxels[(y * 32 + z) * 32 + x] = 4;
                }
                if wy <= 8 {
                    b.voxels[(y * 32 + z) * 32 + x] = 20; // lava/agua infernal
                }
            }
        }
    }
    b
}
