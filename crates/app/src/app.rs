//! App: estado de pantallas, ventana winit, bucle de render, entrada.

use crate::game::{GameSession, PlayerInput};
use crate::screens;
use crate::save;
use anyhow::Result;
use glam::Vec3;
use rivaren_core::DrawList;
use rivaren_gameplay::{ItemStack, Settings, WorldConfig};
use rivaren_render::{QualityTier, Renderer, SceneParams};
use rivaren_ui::input::{InputState, Key};
use rivaren_ui::theme::Breakpoint;
use rivaren_ui::widgets::Ui;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{DeviceEvent, DeviceId, ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{CursorGrabMode, Window, WindowAttributes, WindowId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    MainMenu,
    WorldCreate,
    Settings,
    Loading,
    Game,
    Pause,
    Inventory,
    Crafting,
    Dialogue,
    Journal,
    Death,
    Mods,
    Credits,
}

pub struct App {
    pub screen: Screen,
    pub settings: Settings,
    pub world_cfg: WorldConfig,
    pub session: Option<GameSession>,
    pub renderer: Option<Renderer>,
    pub window: Option<Arc<Window>>,
    pub input: InputState,
    pub draw: DrawList,
    pub frame: u64,
    pub last: Instant,
    pub fps: f32,
    pub fps_acc: (f32, u32),
    pub debug: bool,
    pub cursor_held: bool,
    pub cursor_item: Option<ItemStack>,
    pub text_buffers: HashMap<u64, String>,
    pub mods_loaded: Vec<String>,
    pub error: Option<String>,
    pub save_name: String,
    pub settings_tab: usize,
    pub quickstart: bool,
    pub budget: crate::budget::FrameBudget,
    pub loading_started: Option<Instant>,
}

pub fn run() -> Result<()> {
    let quickstart = std::env::args().any(|a| a == "--quickstart");
    let event_loop = EventLoop::new()?;
    // Mods de datos (JSON) desde la carpeta de usuario.
    let mut mod_registry = rivaren_mods::ModRegistry::new();
    let mods_dir = save::data_dir().join("mods");
    match mod_registry.load_dir(&mods_dir) {
        Ok(n) if n > 0 => tracing::info!("{n} mod(s) cargados de {}", mods_dir.display()),
        _ => {}
    }
    {
        // Registra los bloques de los mods como items colocables (ids 200+).
        let mut entries = Vec::new();
        for (id, (block_id, def)) in mod_registry.blocks.iter() {
            let color = def
                .color
                .unwrap_or_else(|| rivaren_mods::mod_block_color(*block_id));
            entries.push((
                *block_id,
                def.name.clone(),
                *block_id,
                color,
            ));
            let _ = id;
        }
        if !entries.is_empty() {
            let n = rivaren_gameplay::items::register_modded_items(entries);
            tracing::info!("{n} bloques de mods registrados como items");
        }
    }
    let mods_loaded: Vec<String> = mod_registry
        .mods
        .iter()
        .map(|m| format!("{} v{} ({} bloques)", m.manifest.name, m.manifest.version, m.manifest.blocks.len()))
        .collect();
    let mut app = App {
        screen: Screen::MainMenu,
        settings: save::load_settings(),
        world_cfg: WorldConfig::default(),
        session: None,
        renderer: None,
        window: None,
        input: InputState::default(),
        draw: DrawList::default(),
        frame: 0,
        last: Instant::now(),
        fps: 0.0,
        fps_acc: (0.0, 0),
        debug: false,
        cursor_held: false,
        cursor_item: None,
        text_buffers: HashMap::new(),
        mods_loaded,
        error: None,
        save_name: "mundo".into(),
        settings_tab: 0,
        quickstart,
        budget: crate::budget::FrameBudget::new(60),
        loading_started: None,
    };
    app.text_buffers.insert(1000, String::new()); // seed
    app.text_buffers.insert(1001, app.world_cfg.name.clone());
    app.text_buffers.insert(2000, app.settings.game.ai_url.clone());
    app.text_buffers.insert(2001, app.settings.game.ai_model.clone());
    event_loop.run_app(&mut app)?;
    Ok(())
}

impl App {
    fn init_gpu(&mut self, event_loop: &ActiveEventLoop) -> Result<()> {
        let _ = &self.quickstart;
        let window = Arc::new(event_loop.create_window(
            WindowAttributes::default()
                .with_title("RIVAREN")
                .with_inner_size(LogicalSize::new(1366, 768))
                .with_min_inner_size(LogicalSize::new(640, 400)),
        )?);
        let size = window.inner_size();
        let tier = match self.settings.graphics.tier {
            0 => QualityTier::Potato,
            1 => QualityTier::Mobile,
            3 => QualityTier::Ultra,
            _ => QualityTier::Balanced,
        };
        let renderer = Renderer::new(Some(window.clone()), (size.width, size.height), tier)?;
        self.renderer = Some(renderer);
        self.window = Some(window);
        if self.quickstart {
            self.start_world("quickstart".into(), None);
        }
        if std::env::var("RIVAREN_DEBUG").is_ok() {
            eprintln!("[dbg] gpu listo");
        }
        Ok(())
    }

    pub fn set_cursor_held(&mut self, held: bool) {
        if let Some(w) = &self.window {
            self.cursor_held = held;
            if held {
                let _ = w
                    .set_cursor_grab(CursorGrabMode::Locked)
                    .or_else(|_| w.set_cursor_grab(CursorGrabMode::Confined));
                w.set_cursor_visible(false);
            } else {
                let _ = w.set_cursor_grab(CursorGrabMode::None);
                w.set_cursor_visible(true);
            }
        }
    }

    pub fn start_world(&mut self, name: String, data: Option<save::SaveData>) {
        let (seed, cfg, inventory, karma, quests_done, edits, pos, dim) = match data {
            Some(d) => (
                d.seed,
                d.config.clone(),
                d.inventory.clone(),
                d.karma,
                d.completed_quests.clone(),
                save::edits_to_map(&d.edits),
                Some(d.player_pos),
                d.dimension,
            ),
            None => {
                let seed: u64 = self
                    .text_buffers
                    .get(&1000)
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(self.world_cfg.seed);
                (
                    seed,
                    self.world_cfg.clone(),
                    rivaren_gameplay::Inventory::default(),
                    rivaren_gameplay::Karma::default(),
                    vec![],
                    HashMap::new(),
                    None,
                    self.world_cfg.start_dimension,
                )
            }
        };
        let mut session = GameSession::new(seed, cfg, &self.settings);
        session.inventory = inventory;
        session.karma = karma;
        for q in quests_done {
            if let Some(quest) = session.quests.quests.iter_mut().find(|x| x.id == q) {
                quest.state = rivaren_gameplay::QuestState::Done;
            }
            session.quests.completed.push(match q.as_str() {
                "primer_aliento" => "primer_aliento",
                "voz_del_vergel" => "voz_del_vergel",
                "pulso_perdido" => "pulso_perdido",
                "descenso_voluntario" => "descenso_voluntario",
                "redencion" => "redencion",
                _ => "primer_aliento",
            });
        }
        session.quests.unlock_available();
        session.edits = edits;
        if let Some(p) = pos {
            session.player.pos = Vec3::from_array(p);
        }
        session.dimension = dim;
        if let Some(r) = &mut self.renderer {
            let props = r.ctx.adapter.get_info();
            let tier = match self.settings.graphics.tier {
                0 => QualityTier::Potato,
                1 => QualityTier::Mobile,
                3 => QualityTier::Ultra,
                _ => QualityTier::Balanced,
            };
            let _ = props;
            r.tier = tier;
        }
        self.session = Some(session);
        self.screen = Screen::Loading;
        self.loading_started = Some(Instant::now());
        self.save_name = name;
    }

    pub fn save_current(&mut self) {
        let Some(s) = &self.session else {
            return;
        };
        let data = save::SaveData {
            version: 1,
            seed: s.seed,
            config: s.config.clone(),
            dimension: s.dimension,
            time_of_day: s.time_of_day,
            player_pos: s.player.pos.to_array(),
            player_yaw: s.player.yaw,
            player_pitch: s.player.pitch,
            inventory: s.inventory.clone(),
            karma: s.karma,
            completed_quests: s.quests.completed.iter().map(|q| q.to_string()).collect(),
            talked: s.talked.clone(),
            edits: s.edits.iter().map(|(k, v)| (*k, *v)).collect(),
            respawn_bed: s.respawn.bed,
            respawn_anchor: s.respawn.anchor,
            respawn_origin: s.respawn.origin,
        };
        if let Err(e) = save::save_world(&self.save_name, &data) {
            tracing::error!("guardado falló: {e}");
            self.error = Some(format!("No se pudo guardar: {e}"));
        } else {
            if let Some(s) = self.session.as_mut() {
                s.message = Some(("Mundo guardado.".into(), 3.0));
            }
        }
    }

    fn player_input(&self) -> PlayerInput {
        PlayerInput {
            forward: self.input.key(Key::W),
            back: self.input.key(Key::S),
            left: self.input.key(Key::A),
            right: self.input.key(Key::D),
            jump: self.input.key(Key::Space),
            sneak: self.input.key(Key::Shift),
            sprint: self.input.key(Key::Ctrl),
        }
    }

    fn update_game(&mut self, dt: f32) {
        // Hotbar.
        let digits = [
            Key::Digit1,
            Key::Digit2,
            Key::Digit3,
            Key::Digit4,
            Key::Digit5,
            Key::Digit6,
            Key::Digit7,
            Key::Digit8,
            Key::Digit9,
        ];
        for (i, k) in digits.iter().enumerate() {
            if self.input.key_pressed(*k) {
                if let Some(s) = self.session.as_mut() {
                    s.inventory.selected = i;
                }
            }
        }
        if self.input.wheel.abs() > 0.0 {
            if let Some(s) = self.session.as_mut() {
                let dir = -self.input.wheel.signum() as i32;
                let sel = (s.inventory.selected as i32 + dir).rem_euclid(9);
                s.inventory.selected = sel as usize;
            }
        }
        // Pantallas.
        if self.input.key_pressed(Key::Escape) {
            self.screen = Screen::Pause;
            self.set_cursor_held(false);
            return;
        }
        if self.input.key_pressed(Key::E) {
            self.screen = Screen::Inventory;
            self.set_cursor_held(false);
            return;
        }
        if self.input.key_pressed(Key::C) {
            self.screen = Screen::Crafting;
            self.set_cursor_held(false);
            return;
        }
        if self.input.key_pressed(Key::J) || self.input.key_pressed(Key::R) {
            self.screen = Screen::Journal;
            self.set_cursor_held(false);
            return;
        }
        if self.input.key_pressed(Key::F3) {
            self.debug = !self.debug;
        }
        if self.input.key_pressed(Key::F) {
            if let Some(s) = self.session.as_mut() {
                s.interact();
                if s.dialogue.is_some() {
                    self.screen = Screen::Dialogue;
                    self.set_cursor_held(false);
                }
            }
        }
        // Rotación con flechas (sin ratón).
        if let Some(s) = self.session.as_mut() {
            let rot = 2.4 * dt;
            if self.input.key(Key::Left) {
                s.player.yaw -= rot;
            }
            if self.input.key(Key::Right) {
                s.player.yaw += rot;
            }
            if self.input.key(Key::Up) {
                s.player.pitch = (s.player.pitch + rot).clamp(-1.55, 1.55);
            }
            if self.input.key(Key::Down) {
                s.player.pitch = (s.player.pitch - rot).clamp(-1.55, 1.55);
            }
        }
        // Física + streaming.
        let pin = self.player_input();
        let mut edits_snapshot: Vec<([i32; 3], u16)> = Vec::new();
        if let Some(s) = self.session.as_mut() {
            s.tick_player(&pin, dt);
            s.tick(dt);
            edits_snapshot = s.edits.iter().map(|(k, v)| (*k, *v)).collect();
        }
        if let (Some(renderer), Some(s)) = (self.renderer.as_mut(), self.session.as_mut()) {
            s.update_streaming(renderer);
        }
        // Interacción con bloques.
        if let (Some(renderer), Some(s)) = (self.renderer.as_mut(), self.session.as_mut()) {
            if self.input.mouse_down && self.cursor_held {
                // Prioriza golpear mobs; si no hay, rompe bloque.
                if !s.attack_mob() {
                    s.try_break(dt, renderer);
                }
            }
            if self.input.mouse_pressed && self.cursor_held {
                s.try_place(renderer);
            }
        }
        // Sonidos de paso.
        if let Some(s) = self.session.as_mut() {
            if pin.forward || pin.back || pin.left || pin.right {
                s.step_timer -= dt;
                if s.step_timer <= 0.0 {
                    s.step_timer = 0.42;
                    let listener = s.player.eye().to_array();
                    let fwd = s.player.forward().to_array();
                    s.audio.play_at(
                        rivaren_audio::Sfx::Step,
                        [s.player.pos.x, s.player.pos.y, s.player.pos.z],
                        listener,
                        fwd,
                    );
                }
            }
        }
        // Muerte.
        if let Some(s) = self.session.as_ref() {
            if s.health <= 0.0 {
                self.screen = Screen::Death;
                self.set_cursor_held(false);
            }
        }
        let _ = edits_snapshot;
    }

    fn build_ui(&mut self) {
        // Resolución para layout responsivo.
        if let Some(r) = self.renderer.as_ref() {
            let (w, h) = r.ctx.size();
            self.input.screen = (w as f32, h as f32);
        } else if self.input.screen.0 < 1.0 {
            self.input.screen = (1280.0, 720.0);
        }
        let mut draw = std::mem::take(&mut self.draw);
        draw.clear(); // Cada frame reconstruye la UI desde cero.
        let mut input = std::mem::take(&mut self.input);
        let bp = Breakpoint::of(input.screen.0);
        {
            let mut ui = Ui::new(&mut draw, &input, bp, self.frame);
            screens::draw(self, &mut ui);
            let _ = &mut ui;
        }
        self.draw = draw;
        input.begin_frame();
        self.input = input;
    }

    fn render_frame(&mut self) {
        if std::env::var("RIVAREN_DEBUG").is_ok() && self.frame < 3 {
            eprintln!("[dbg] render_frame {}", self.frame);
        }
        let now = Instant::now();
        let dt = (now - self.last).as_secs_f32().clamp(0.0, 0.1);
        self.last = now;
        self.frame += 1;
        self.fps_acc.0 += dt;
        self.fps_acc.1 += 1;
        if self.fps_acc.0 >= 0.5 {
            self.fps = self.fps_acc.1 as f32 / self.fps_acc.0;
            self.fps_acc = (0.0, 0);
            // Resolución dinámica: ajusta la escala de render al presupuesto.
            let scale = self.budget.adapt_resolution(self.fps, self.settings.graphics.render_scale);
            if (scale - self.settings.graphics.render_scale).abs() > 0.01 {
                self.settings.graphics.render_scale = scale;
            }
        }

        if self.screen == Screen::Game {
            self.update_game(dt);
        } else if self.screen == Screen::Loading {
            if let (Some(r), Some(s)) = (self.renderer.as_mut(), self.session.as_mut()) {
                s.update_streaming(r);
                let elapsed = self
                    .loading_started
                    .map(|t| t.elapsed().as_secs_f32())
                    .unwrap_or(0.0);
                if s.stats_chunks > 120 || elapsed > 6.0 {
                    self.screen = Screen::Game;
                    self.set_cursor_held(true);
                }
            }
        }
        self.build_ui();

        // Escena para el renderer.
        let (aspect, cam, scene) = if let Some(s) = &self.session {
            let (w, h) = self
                .renderer
                .as_ref()
                .map(|r| r.ctx.size())
                .unwrap_or((1280, 720));
            let aspect = w as f32 / h.max(1) as f32;
            let cam = s.player.camera(aspect);
            let scene = SceneParams {
                camera: cam,
                time_of_day: s.time_of_day,
                underwater: s.player.pos.y < 60.0 && s.dimension == rivaren_gameplay::Dimension::Tierra && s.block_at(s.player.pos.x.floor() as i32, s.player.pos.y.floor() as i32, s.player.pos.z.floor() as i32) == 20,
                weather: s.weather,
                exposure: 1.0,
                bloom: self.settings.graphics.bloom,
                sharpen: self.settings.graphics.sharpen,
                taa: self.settings.graphics.taa && s.dimension != rivaren_gameplay::Dimension::Cielo,
                render_scale: self.settings.graphics.render_scale,
                fog_density: self.settings.graphics.fog,
            };
            (aspect, cam, scene)
        } else {
            let (w, h) = self
                .renderer
                .as_ref()
                .map(|r| r.ctx.size())
                .unwrap_or((1280, 720));
            let aspect = w as f32 / h.max(1) as f32;
            let cam = rivaren_render::Camera {
                pos: Vec3::new(0.0, 120.0, 0.0),
                yaw: self.frame as f32 * 0.003,
                pitch: -0.2,
                aspect,
                ..Default::default()
            };
            (
                aspect,
                cam,
                SceneParams {
                    camera: cam,
                    time_of_day: 0.35,
                    ..Default::default()
                },
            )
        };
        let _ = (aspect, cam);
        if let (Some(r), Some(session)) = (self.renderer.as_mut(), self.session.as_ref()) {
            let mut entities: Vec<rivaren_render::EntityInstance> =
                Vec::with_capacity(session.mobs.len() + session.npcs.len());
            for mob in &session.mobs {
                let size = mob.species.size();
                entities.push(rivaren_render::EntityInstance {
                    pos: [mob.pos.x, mob.pos.y + size[1] * 0.5, mob.pos.z],
                    _pad0: 0.0,
                    size,
                    _pad1: 0.0,
                    color: mob.species.color(),
                });
            }
            for npc in &session.npcs {
                let color = match npc.kind {
                    "Iluminado" => [0.95, 0.92, 0.70, 1.0],
                    "Penitente" => [0.75, 0.30, 0.25, 1.0],
                    _ => [0.7, 0.7, 0.7, 1.0],
                };
                entities.push(rivaren_render::EntityInstance {
                    pos: [npc.pos.x, npc.pos.y + 0.9, npc.pos.z],
                    _pad0: 0.0,
                    size: [0.7, 1.8, 0.7],
                    _pad1: 0.0,
                    color,
                });
            }
            r.set_entities(&entities);
            if let Err(e) = r.render(&scene, &self.draw) {
                tracing::error!("render: {e:#}");
            }
        } else if let Some(r) = self.renderer.as_mut() {
            r.set_entities(&[]);
            if let Err(e) = r.render(&scene, &self.draw) {
                tracing::error!("render: {e:#}");
            }
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if std::env::var("RIVAREN_DEBUG").is_ok() {
            eprintln!("[dbg] resumed");
        }
        event_loop.set_control_flow(winit::event_loop::ControlFlow::Poll);
        if self.renderer.is_none() {
            if let Err(e) = self.init_gpu(event_loop) {
                tracing::error!("GPU init: {e:#}");
                self.error = Some(format!("GPU: {e:#}"));
                event_loop.exit();
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                if self.session.is_some() {
                    self.save_current();
                }
                let _ = save::save_settings(&self.settings);
                event_loop.exit();
            }
            WindowEvent::Resized(size) => {
                if let Some(r) = self.renderer.as_mut() {
                    r.resize(size.width, size.height);
                }
            }
            WindowEvent::RedrawRequested => {
                // El render continuo vive en about_to_wait (ControlFlow::Poll):
                // en Wayland los frame-callbacks no llegan si la ventana está tapada.
            }
            WindowEvent::KeyboardInput { event, .. } => {
                let pressed = event.state == ElementState::Pressed;
                if let Some(key) = map_key(event.physical_key) {
                    if pressed {
                        self.input.keys_down.insert(key);
                        self.input.keys_pressed.insert(key);
                    } else {
                        self.input.keys_down.remove(&key);
                    }
                }
                // Texto para campos de texto (solo en menús).
                if pressed && !self.cursor_held {
                    if let winit::keyboard::Key::Character(s) = &event.logical_key {
                        let allowed = self.screen != Screen::Game;
                        if allowed {
                            self.input.text_input.push_str(s);
                        }
                    }
                    if event.physical_key == PhysicalKey::Code(KeyCode::Enter) {
                        self.input.keys_pressed.insert(Key::Enter);
                    }
                    if event.physical_key == PhysicalKey::Code(KeyCode::Backspace) {
                        self.input.text_input.push('\u{8}');
                    }
                }
            }
            WindowEvent::MouseInput { state, button, .. } => match button {
                MouseButton::Left => {
                    let pressed = state == ElementState::Pressed;
                    if pressed {
                        self.input.mouse_pressed = true;
                    } else {
                        self.input.mouse_released = true;
                    }
                    self.input.mouse_down = pressed;
                }
                MouseButton::Right
                    if state == ElementState::Pressed => {
                        self.input.mouse_pressed = true;
                        // Colocar/interactuar.
                        if self.screen == Screen::Game {
                            if let (Some(renderer), Some(s)) =
                                (self.renderer.as_mut(), self.session.as_mut())
                            {
                                s.try_place(renderer);
                            }
                        }
                    }
                _ => {}
            },
            WindowEvent::CursorMoved { position, .. } => {
                self.input.mouse_x = position.x as f32;
                self.input.mouse_y = position.y as f32;
            }
            WindowEvent::CursorLeft { .. } => {
                self.input.mouse_x = -1000.0;
                self.input.mouse_y = -1000.0;
            }
            WindowEvent::MouseWheel { delta, .. } => {
                self.input.wheel = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y,
                    MouseScrollDelta::PixelDelta(p) => p.y as f32 * 0.1,
                };
            }
            WindowEvent::Touch(touch) => {
                use winit::event::TouchPhase;
                self.input.mouse_x = touch.location.x as f32;
                self.input.mouse_y = touch.location.y as f32;
                match touch.phase {
                    TouchPhase::Started => {
                        self.input.mouse_down = true;
                        self.input.mouse_pressed = true;
                        self.input.touch = Some((self.input.mouse_x, self.input.mouse_y));
                    }
                    TouchPhase::Ended | TouchPhase::Cancelled => {
                        self.input.mouse_down = false;
                        self.input.mouse_released = true;
                        self.input.touch = None;
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }

    fn device_event(&mut self, _el: &ActiveEventLoop, _id: DeviceId, event: DeviceEvent) {
        if let DeviceEvent::MouseMotion { delta } = event {
            if self.cursor_held {
                if let Some(s) = self.session.as_mut() {
                    let sens = self.settings.game.sensitivity;
                    s.player.yaw += delta.0 as f32 * sens * 0.003;
                    s.player.pitch =
                        (s.player.pitch - delta.1 as f32 * sens * 0.003).clamp(-1.55, 1.55);
                }
            }
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if self.renderer.is_some() {
            self.render_frame();
        }
    }
}

fn map_key(pk: PhysicalKey) -> Option<Key> {
    let PhysicalKey::Code(code) = pk else {
        return None;
    };
    Some(match code {
        KeyCode::KeyW => Key::W,
        KeyCode::KeyA => Key::A,
        KeyCode::KeyS => Key::S,
        KeyCode::KeyD => Key::D,
        KeyCode::Space => Key::Space,
        KeyCode::ShiftLeft | KeyCode::ShiftRight => Key::Shift,
        KeyCode::ControlLeft | KeyCode::ControlRight => Key::Ctrl,
        KeyCode::KeyE => Key::E,
        KeyCode::Escape => Key::Escape,
        KeyCode::Tab => Key::Tab,
        KeyCode::KeyF => Key::F,
        KeyCode::KeyJ => Key::J,
        KeyCode::KeyC => Key::C,
        KeyCode::KeyR => Key::R,
        KeyCode::KeyT => Key::T,
        KeyCode::F3 => Key::F3,
        KeyCode::F5 => Key::F5,
        KeyCode::Digit1 => Key::Digit1,
        KeyCode::Digit2 => Key::Digit2,
        KeyCode::Digit3 => Key::Digit3,
        KeyCode::Digit4 => Key::Digit4,
        KeyCode::Digit5 => Key::Digit5,
        KeyCode::Digit6 => Key::Digit6,
        KeyCode::Digit7 => Key::Digit7,
        KeyCode::Digit8 => Key::Digit8,
        KeyCode::Digit9 => Key::Digit9,
        KeyCode::ArrowUp => Key::Up,
        KeyCode::ArrowDown => Key::Down,
        KeyCode::ArrowLeft => Key::Left,
        KeyCode::ArrowRight => Key::Right,
        KeyCode::Enter => Key::Enter,
        KeyCode::Backspace => Key::Backspace,
        _ => return None,
    })
}
