//! Todas las pantallas del juego dibujadas con el toolkit UI propio.
//! Responsive: se adapta a móvil/tablet/desktop sin desbordar.

use crate::app::{App, Screen};
use crate::save;
use rivaren_gameplay::{
    difficulty_label_cycle, gamemode_cycle, item_def, try_craft, Objective, QuestState,
};
use rivaren_ui::layout::{Dir, Rect, Size, Stack};
use rivaren_ui::theme::*;
use rivaren_ui::widgets::{SlotView, Ui};

/// HUD aislado para capturas/tests (sin App completo).
pub fn draw_hud_only(session: &crate::game::GameSession, ui: &mut Ui, fps: f32) {
    let r = screen_rect(ui);
    let cx = r.w * 0.5;
    let cy = r.h * 0.5;
    ui.draw.rect(cx - 9.0, cy - 1.0, 18.0, 2.0, [1.0, 1.0, 1.0, 0.8]);
    ui.draw.rect(cx - 1.0, cy - 9.0, 2.0, 18.0, [1.0, 1.0, 1.0, 0.8]);
    let slot = 46.0;
    let total = slot * 9.0 + 8.0 * 8.0;
    let hx = cx - total * 0.5;
    let hy = r.h - slot - 18.0;
    for i in 0..9 {
        let rect = Rect::new(hx + i as f32 * (slot + 8.0), hy, slot, slot);
        let view = session
            .inventory
            .slots
            .get(i)
            .and_then(|x| x.as_ref())
            .map(slot_view);
        ui.item_slot(4000 + i as u64, rect, view.as_ref(), i == session.inventory.selected);
    }
    let bar_w = 160.0;
    ui.progress(
        Rect::new(cx - bar_w - 12.0, hy - 22.0, bar_w, 12.0),
        session.health / 20.0,
        [0.85, 0.25, 0.28, 1.0],
    );
    ui.progress(
        Rect::new(cx + 12.0, hy - 22.0, bar_w, 12.0),
        session.hunger / 20.0,
        [0.85, 0.62, 0.25, 1.0],
    );
    let kx = r.w - 190.0;
    ui.draw.rounded(kx, 20.0, 170.0, 74.0, RADIUS_SMALL, [0.05, 0.05, 0.07, 0.72]);
    ui.draw.text("Balanza del Alma", kx + 10.0, 26.0, SMALL_SIZE, ui.palette.text_dim);
    let bars = [
        ("Compasión", session.karma.compasion, ui.palette.compassion),
        ("Justicia", session.karma.justicia, ui.palette.justice),
        ("Sabiduría", session.karma.sabiduria, ui.palette.wisdom),
    ];
    for (i, (label, v, color)) in bars.iter().enumerate() {
        let y = 44.0 + i as f32 * 15.0;
        ui.draw.text(*label, kx + 10.0, y, 11.0, ui.palette.text_dim);
        let track = Rect::new(kx + 80.0, y + 2.0, 80.0, 7.0);
        ui.draw.rounded(track.x, track.y, track.w, track.h, 3.0, [0.18, 0.18, 0.22, 1.0]);
        let t = ((v + 100.0) / 200.0).clamp(0.0, 1.0);
        ui.draw.rounded(track.x, track.y, track.w * t, track.h, 3.0, *color);
    }
    ui.draw.text(
        format!("{fps:.0} FPS | {} chunks | {:?}", session.stats_chunks, session.dimension),
        kx + 10.0, 100.0, SMALL_SIZE, ui.palette.text,
    );
}

pub fn draw(app: &mut App, ui: &mut Ui) {
    match app.screen {
        Screen::MainMenu => main_menu(app, ui),
        Screen::WorldCreate => world_create(app, ui),
        Screen::Settings => settings_screen(app, ui),
        Screen::Loading => loading(app, ui),
        Screen::Game => hud(app, ui),
        Screen::Pause => pause(app, ui),
        Screen::Inventory => inventory(app, ui, false),
        Screen::Crafting => inventory(app, ui, true),
        Screen::Dialogue => dialogue(app, ui),
        Screen::Journal => journal(app, ui),
        Screen::Death => death(app, ui),
        Screen::Mods => mods(app, ui),
        Screen::Credits => credits(app, ui),
    }
}

fn screen_rect(ui: &Ui) -> Rect {
    Rect::new(0.0, 0.0, ui.input.screen.0, ui.input.screen.1)
}

fn background(ui: &mut Ui) {
    let r = screen_rect(ui);
    ui.draw
        .rect(0.0, 0.0, r.w, r.h, [0.045, 0.05, 0.075, 1.0]);
    // Degradado decorativo + "estrellas" deterministas.
    let n = ((r.w * r.h) / 9000.0) as u32;
    let mut h: u64 = 0x9E3779B97F4A7C15;
    for _ in 0..n.min(240) {
        h = h.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        let x = (h >> 16) as f32 / 2.8e14 * r.w;
        let y = ((h >> 40) as f32 / 1.6e7) % (r.h * 0.7);
        let a = 0.05 + ((h >> 8) & 0xFF) as f32 / 255.0 * 0.12;
        ui.draw.rect(x, y, 2.0, 2.0, [0.9, 0.9, 1.0, a]);
    }
    // Banda inferior.
    ui.draw.rect(0.0, r.h * 0.72, r.w, r.h * 0.28, [0.02, 0.03, 0.05, 0.65]);
}

// ── Menú principal ───────────────────────────────────────────────

fn main_menu(app: &mut App, ui: &mut Ui) {
    background(ui);
    let r = screen_rect(ui);
    let card = Stack::centered_card(r, 460.0, 560.0);
    ui.draw.text_centered(
        "RIVAREN",
        card.center_x(),
        card.y + 28.0,
        TITLE_SIZE * (ui.breakpoint.scale() * 1.05),
        ui.palette.text_title,
    );
    ui.draw.text_centered(
        "un mundo que se calcula, no se almacena",
        card.center_x(),
        card.y + 92.0,
        SMALL_SIZE,
        ui.palette.text_dim,
    );
    let mut col = Stack::new(
        Rect::new(card.x + 60.0, card.y + 140.0, card.w - 120.0, card.h - 180.0),
        Dir::Column,
        12.0,
    );
    let b = |col: &mut Stack, ui: &mut Ui, _app: &mut App, id: u64, label: &str, primary: bool| -> bool {
        let rect = col.next(Size::Fixed(ui.breakpoint.touch_target().max(40.0)));
        if primary {
            ui.button_primary(id, rect, label).clicked
        } else {
            ui.button(id, rect, label).clicked
        }
    };
    col.next(Size::Fill);
    if b(&mut col, ui, app, 1, "Jugar", true) {
        if app.session.is_some() {
            app.screen = Screen::Game;
            app.set_cursor_held(true);
        } else {
            app.screen = Screen::WorldCreate;
        }
    }
    let has_saves = !save::list_saves().is_empty();
    if has_saves && b(&mut col, ui, app, 2, "Cargar partida", false) {
        let list = save::list_saves();
        if let Some(name) = list.first() {
            match save::load_world(name) {
                Ok(data) => app.start_world(name.clone(), Some(data)),
                Err(e) => app.error = Some(format!("No se pudo cargar: {e}")),
            }
        }
    }
    if b(&mut col, ui, app, 3, "Nuevo mundo", false) {
        app.screen = Screen::WorldCreate;
    }
    if b(&mut col, ui, app, 4, "Ajustes", false) {
        app.screen = Screen::Settings;
    }
    if b(&mut col, ui, app, 5, "Mods", false) {
        app.mods_loaded = list_mods();
        app.screen = Screen::Mods;
    }
    if b(&mut col, ui, app, 6, "Créditos", false) {
        app.screen = Screen::Credits;
    }
    if b(&mut col, ui, app, 7, "Salir", false) {
        let _ = rivaren_render::Renderer::new;
        std::process::exit(0);
    }
    col.next(Size::Fill);
    if let Some(err) = &app.error {
        ui.draw.text_centered(err, r.center_x(), r.h - 40.0, SMALL_SIZE, ui.palette.danger);
    }
}

fn list_mods() -> Vec<String> {
    let dir = save::data_dir().join("mods");
    let mut out = Vec::new();
    if let Ok(rd) = std::fs::read_dir(&dir) {
        for e in rd.flatten() {
            if let Some(n) = e.file_name().to_str() {
                out.push(n.to_string());
            }
        }
    }
    out
}

fn mods(app: &mut App, ui: &mut Ui) {
    background(ui);
    let r = screen_rect(ui);
    let card = Stack::centered_card(r, 620.0, 520.0);
    ui.panel_titled(card, "Mods");
    let list = Rect::new(card.x + SPACE, card.y + 60.0, card.w - SPACE * 2.0, card.h - 140.0);
    ui.draw
        .rounded(list.x, list.y, list.w, list.h, RADIUS_SMALL, ui.palette.bg_deep);
    if app.mods_loaded.is_empty() {
        ui.draw.text(
            "No hay mods instalados. Coloca archivos .json en la carpeta de datos:",
            list.x + 12.0,
            list.y + 12.0,
            SMALL_SIZE,
            ui.palette.text_dim,
        );
        ui.draw.text(
            save::data_dir().join("mods").display().to_string(),
            list.x + 12.0,
            list.y + 34.0,
            SMALL_SIZE,
            ui.palette.accent,
        );
        ui.draw.text(
            "Formato: ver assets/data/mod_ejemplo.json (bloques, biomas, ejes de karma).",
            list.x + 12.0,
            list.y + 56.0,
            SMALL_SIZE,
            ui.palette.text_dim,
        );
    } else {
        for (i, m) in app.mods_loaded.iter().enumerate() {
            ui.draw.text(
                m,
                list.x + 12.0,
                list.y + 12.0 + i as f32 * 24.0,
                BODY_SIZE,
                ui.palette.text,
            );
        }
    }
    let btn = Rect::new(card.x + SPACE, card.y + card.h - 60.0, 180.0, 40.0);
    if ui.button(100, btn, "Volver").clicked {
        app.screen = Screen::MainMenu;
    }
}

fn credits(app: &mut App, ui: &mut Ui) {
    background(ui);
    let r = screen_rect(ui);
    let card = Stack::centered_card(r, 560.0, 480.0);
    ui.panel_titled(card, "Créditos");
    let lines = [
        "RIVAREN — motor de vóxeles infinito en Rust + wgpu/Vulkan",
        "",
        "Diseño y narrativa original: karma, Cielo, Tierra, Infierno.",
        "Sin assets ni nombres de terceros: todo el contenido es propio.",
        "",
        "Tecnología: wgpu 29, winit, glyphon, kira, rayon, SVDAG.",
        "Fuentes: DejaVu (Bitstream Vera) y Liberation (OFL).",
        "",
        "Gracias por jugar. El mundo se calcula, no se almacena.",
    ];
    for (i, l) in lines.iter().enumerate() {
        ui.draw.text(
            *l,
            card.x + SPACE,
            card.y + 64.0 + i as f32 * 24.0,
            SMALL_SIZE,
            ui.palette.text,
        );
    }
    let btn = Rect::new(card.x + SPACE, card.y + card.h - 60.0, 180.0, 40.0);
    if ui.button(110, btn, "Volver").clicked {
        app.screen = Screen::MainMenu;
    }
}

// ── Crear mundo ──────────────────────────────────────────────────

fn world_create(app: &mut App, ui: &mut Ui) {
    background(ui);
    let r = screen_rect(ui);
    let card = Stack::centered_card(r, 620.0, 560.0);
    ui.panel_titled(card, "Crear mundo");
    let mut col = Stack::new(
        Rect::new(card.x + SPACE * 2.0, card.y + 64.0, card.w - SPACE * 4.0, card.h - 140.0),
        Dir::Column,
        14.0,
    );
    // Nombre.
    let name_rect = col.next(Size::Fixed(48.0));
    ui.draw.text("Nombre", name_rect.x, name_rect.y - 2.0, SMALL_SIZE, ui.palette.text_dim);
    let name_field = Rect::new(name_rect.x, name_rect.y + 18.0, name_rect.w, 34.0);
    let mut name = app
        .text_buffers
        .get(&1001)
        .cloned()
        .unwrap_or_else(|| "Mundo Nuevo".into());
    if ui.text_field(1001, name_field, &mut name, "Mundo Nuevo").changed {
        app.world_cfg.name = name.clone();
    }
    app.text_buffers.insert(1001, name.clone());
    // Semilla.
    let seed_rect = col.next(Size::Fixed(48.0));
    ui.draw.text("Semilla", seed_rect.x, seed_rect.y - 2.0, SMALL_SIZE, ui.palette.text_dim);
    let seed_field = Rect::new(seed_rect.x, seed_rect.y + 18.0, seed_rect.w - 130.0, 34.0);
    let mut seed = app
        .text_buffers
        .get(&1000)
        .cloned()
        .unwrap_or_default();
    if seed.is_empty() {
        seed = app.world_cfg.seed.to_string();
    }
    if ui.text_field(1000, seed_field, &mut seed, "Semilla").changed {
        if let Ok(v) = seed.parse::<u64>() {
            app.world_cfg.seed = v;
        }
    }
    app.text_buffers.insert(1000, seed.clone());
    let rand_btn = Rect::new(seed_field.x + seed_field.w + 8.0, seed_field.y, 114.0, 34.0);
    if ui.button(1010, rand_btn, "Aleatoria").clicked {
        let s = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(1337);
        app.world_cfg.seed = s;
        app.text_buffers.insert(1000, s.to_string());
    }
    // Modo de juego / dificultad.
    let modes = col.next(Size::Fixed(40.0));
    let half = modes.w * 0.5 - 6.0;
    let gm = Rect::new(modes.x, modes.y, half, modes.h);
    let df = Rect::new(modes.x + half + 12.0, modes.y, half, modes.h);
    if ui
        .button(1011, gm, &format!("Modo: {}", app.world_cfg.gamemode.label()))
        .clicked
    {
        app.world_cfg.gamemode = gamemode_cycle(app.world_cfg.gamemode);
    }
    if ui
        .button(1012, df, &format!("Dificultad: {}", app.world_cfg.difficulty.label()))
        .clicked
    {
        app.world_cfg.difficulty = difficulty_label_cycle(app.world_cfg.difficulty);
    }
    // Dimension inicial.
    let dims = col.next(Size::Fixed(40.0));
    let dlabels = ["Tierra", "Cielo", "Infierno"];
    let mut dsel = match app.world_cfg.start_dimension {
        rivaren_gameplay::Dimension::Tierra => 0,
        rivaren_gameplay::Dimension::Cielo => 1,
        rivaren_gameplay::Dimension::Infierno => 2,
    };
    if ui.tabs(1020, dims, &dlabels, &mut dsel).changed {
        app.world_cfg.start_dimension = match dsel {
            1 => rivaren_gameplay::Dimension::Cielo,
            2 => rivaren_gameplay::Dimension::Infierno,
            _ => rivaren_gameplay::Dimension::Tierra,
        };
    }
    // Distancia de render.
    let dist = col.next(Size::Fixed(52.0));
    let mut rd = app.world_cfg.render_distance as f32;
    if ui.slider(1030, dist, "Distancia de render (chunks)", &mut rd, 2.0, 12.0).changed {
        app.world_cfg.render_distance = rd.round() as i32;
    }
    // Botones.
    col.next(Size::Fill);
    let btns = col.next(Size::Fixed(44.0));
    let bw = (btns.w - 12.0) * 0.5;
    if ui.button_primary(1040, Rect::new(btns.x, btns.y, bw, btns.h), "Crear y jugar").clicked {
        let name = app.text_buffers.get(&1001).cloned().unwrap_or_default();
        let name = if name.trim().is_empty() { "Mundo Nuevo".to_string() } else { name };
        app.start_world(name, None);
    }
    if ui.button(1041, Rect::new(btns.x + bw + 12.0, btns.y, bw, btns.h), "Volver").clicked {
        app.screen = Screen::MainMenu;
    }
}

// ── Ajustes ──────────────────────────────────────────────────────

fn settings_screen(app: &mut App, ui: &mut Ui) {
    background(ui);
    let r = screen_rect(ui);
    let card = Stack::centered_card(r, 720.0, 620.0);
    ui.panel_titled(card, "Ajustes");
    let tabs_rect = Rect::new(card.x + SPACE, card.y + 56.0, card.w - SPACE * 2.0, 36.0);
    let mut tab = app.settings_tab;
    let labels = ["Gráficos", "Sonido", "Juego", "IA"];
    if ui.tabs(3000, tabs_rect, &labels, &mut tab).changed {
        app.settings_tab = tab;
    }
    let body = Rect::new(
        card.x + SPACE * 2.0,
        card.y + 108.0,
        card.w - SPACE * 4.0,
        card.h - 190.0,
    );
    match tab {
        0 => {
            let g = &mut app.settings.graphics;
            let mut col = Stack::new(body, Dir::Column, 12.0);
            let s = col.next(Size::Fixed(44.0));
            let mut tier = g.tier as f32;
            if ui.slider(3010, s, "Calidad", &mut tier, 0.0, 3.0).changed {
                g.tier = tier.round() as u8;
            }
            let s = col.next(Size::Fixed(44.0));
            if ui
                .slider(3011, s, "Escala de render", &mut g.render_scale, 0.5, 1.0)
                .changed
            {
                if let Some(rr) = app.renderer.as_mut() {
                    rr.tier = match g.tier {
                        0 => rivaren_render::QualityTier::Potato,
                        1 => rivaren_render::QualityTier::Mobile,
                        3 => rivaren_render::QualityTier::Ultra,
                        _ => rivaren_render::QualityTier::Balanced,
                    };
                }
            }
            let s = col.next(Size::Fixed(44.0));
            ui.slider(3012, s, "FOV", &mut g.fov, 50.0, 110.0);
            let s = col.next(Size::Fixed(44.0));
            let mut dist = g.view_distance as f32;
            if ui.slider(3013, s, "Distancia de chunks", &mut dist, 2.0, 12.0).changed {
                g.view_distance = dist.round() as i32;
                app.world_cfg.render_distance = g.view_distance;
            }
            let s = col.next(Size::Fixed(44.0));
            ui.slider(3014, s, "Bloom", &mut g.bloom, 0.0, 0.3);
            let s = col.next(Size::Fixed(44.0));
            ui.slider(3015, s, "Nitidez", &mut g.sharpen, 0.0, 1.0);
            let s = col.next(Size::Fixed(36.0));
            let half = s.w * 0.5 - 6.0;
            ui.toggle(3016, Rect::new(s.x, s.y, half, s.h), "TAA", &mut g.taa);
            ui.toggle(3017, Rect::new(s.x + half + 12.0, s.y, half, s.h), "VSync", &mut g.vsync);
        }
        1 => {
            let a = &mut app.settings.audio;
            let mut col = Stack::new(body, Dir::Column, 12.0);
            let s = col.next(Size::Fixed(44.0));
            ui.slider(3020, s, "Volumen maestro", &mut a.master, 0.0, 1.0);
            let s = col.next(Size::Fixed(44.0));
            ui.slider(3021, s, "Música", &mut a.music, 0.0, 1.0);
            let s = col.next(Size::Fixed(44.0));
            ui.slider(3022, s, "Efectos", &mut a.sfx, 0.0, 1.0);
            let s = col.next(Size::Fixed(30.0));
            ui.draw.text(
                "La música y efectos son sintetizados en tiempo real (100% original).",
                s.x,
                s.y,
                SMALL_SIZE,
                ui.palette.text_dim,
            );
            if let Some(session) = app.session.as_mut() {
                session
                    .audio
                    .set_volumes(a.master, a.music, a.sfx);
            }
        }
        2 => {
            let g = &mut app.settings.game;
            let mut col = Stack::new(body, Dir::Column, 12.0);
            let s = col.next(Size::Fixed(44.0));
            let mut sens = g.sensitivity;
            if ui.slider(3030, s, "Sensibilidad", &mut sens, 0.02, 0.5).changed {
                g.sensitivity = sens;
            }
            let s = col.next(Size::Fixed(44.0));
            let mut hud = g.hud_scale;
            if ui.slider(3031, s, "Escala de HUD", &mut hud, 0.7, 1.5).changed {
                g.hud_scale = hud;
            }
            let s = col.next(Size::Fixed(36.0));
            ui.toggle(3032, s, "Subtítulos", &mut g.subtitles);
            let s = col.next(Size::Fixed(30.0));
            ui.draw.text(
                "Teclas: WASD mover, ratón mirar, E inventario, C mesa, R diario, F hablar, 1-9 hotbar.",
                s.x,
                s.y,
                SMALL_SIZE,
                ui.palette.text_dim,
            );
        }
        _ => {
            let g = &mut app.settings.game;
            let mut col = Stack::new(body, Dir::Column, 12.0);
            let s = col.next(Size::Fixed(40.0));
            let mut mode = g.ai_mode as f32;
            if ui.slider(3040, s, "Modo de IA", &mut mode, 0.0, 2.0).changed {
                g.ai_mode = mode.round() as u8;
                if let Some(session) = app.session.as_mut() {
                    session.ai.config.mode = match g.ai_mode {
                        1 => rivaren_ai::AiMode::Local,
                        2 => rivaren_ai::AiMode::Cloud,
                        _ => rivaren_ai::AiMode::Rules,
                    };
                }
            }
            let s = col.next(Size::Fixed(30.0));
            ui.draw.text(
                match g.ai_mode {
                    0 => "Modo reglas: diálogos deterministas, sin red.",
                    1 => "Modo local: Ollama o llama.cpp en esta máquina.",
                    _ => "Modo nube: API OpenAI-compatible (clave por variable de entorno).",
                },
                s.x,
                s.y,
                SMALL_SIZE,
                ui.palette.accent,
            );
            let s = col.next(Size::Fixed(48.0));
            ui.draw.text("URL", s.x, s.y, SMALL_SIZE, ui.palette.text_dim);
            let f = Rect::new(s.x, s.y + 18.0, s.w, 32.0);
            let mut url = app.text_buffers.get(&2000).cloned().unwrap_or_else(|| g.ai_url.clone());
            if ui.text_field(2000, f, &mut url, "http://localhost:11434").changed {
                g.ai_url = url.clone();
                if let Some(session) = app.session.as_mut() {
                    session.ai.config.url = url.clone();
                }
            }
            app.text_buffers.insert(2000, url);
            let s = col.next(Size::Fixed(48.0));
            ui.draw.text("Modelo", s.x, s.y, SMALL_SIZE, ui.palette.text_dim);
            let f = Rect::new(s.x, s.y + 18.0, s.w, 32.0);
            let mut model = app.text_buffers.get(&2001).cloned().unwrap_or_else(|| g.ai_model.clone());
            if ui.text_field(2001, f, &mut model, "qwen3:4b").changed {
                g.ai_model = model.clone();
                if let Some(session) = app.session.as_mut() {
                    session.ai.config.model = model.clone();
                }
            }
            app.text_buffers.insert(2001, model);
        }
    }
    // Botones.
    let btns = Rect::new(card.x + SPACE, card.y + card.h - 64.0, card.w - SPACE * 2.0, 44.0);
    let bw = (btns.w - 24.0) / 3.0;
    if ui.button_primary(3050, Rect::new(btns.x, btns.y, bw, btns.h), "Guardar").clicked {
        let _ = save::save_settings(&app.settings);
        if let Some(s) = app.session.as_mut() {
            s.message = Some(("Ajustes guardados.".into(), 3.0));
        }
    }
    let mut back = false;
    if ui.button(3051, Rect::new(btns.x + bw + 12.0, btns.y, bw, btns.h), "Volver").clicked {
        back = true;
    }
    if ui.button(3052, Rect::new(btns.x + (bw + 12.0) * 2.0, btns.y, bw, btns.h), "Restablecer").clicked {
        app.settings = Default::default();
    }
    if back {
        app.screen = if app.session.is_some() { Screen::Pause } else { Screen::MainMenu };
    }
}

// ── Carga ────────────────────────────────────────────────────────

fn loading(app: &mut App, ui: &mut Ui) {
    background(ui);
    let r = screen_rect(ui);
    let card = Stack::centered_card(r, 520.0, 220.0);
    ui.panel(card);
    ui.draw.text_centered(
        "Generando mundo...",
        card.center_x(),
        card.y + 40.0,
        HEADING_SIZE,
        ui.palette.text_title,
    );
    let loaded = app.session.as_ref().map(|s| s.stats_chunks).unwrap_or(0) as f32;
    let target = 300.0f32;
    let p = (loaded / target).clamp(0.0, 1.0);
    let bar = Rect::new(card.x + 40.0, card.y + 100.0, card.w - 80.0, 18.0);
    ui.progress(bar, p, ui.palette.accent);
    ui.draw.text_centered(
        format!("{:.0}%  ({} chunks)", p * 100.0, loaded as u32),
        card.center_x(),
        card.y + 130.0,
        SMALL_SIZE,
        ui.palette.text_dim,
    );
    if loaded > 40.0 {
        app.screen = Screen::Game;
        app.set_cursor_held(true);
    }
}

// ── HUD ──────────────────────────────────────────────────────────

fn hud(app: &mut App, ui: &mut Ui) {
    struct Snap {
        health: f32,
        hunger: f32,
        karma: rivaren_gameplay::Karma,
        time_of_day: f32,
        weather: f32,
        message: Option<(String, f32)>,
        hotbar: Vec<Option<SlotView>>,
        selected: usize,
        break_progress: f32,
        npc_prompt: Option<String>,
        flash: f32,
        stats_chunks: usize,
        pos: glam::Vec3,
        dimension: rivaren_gameplay::Dimension,
        edits: usize,
        in_dialogue: bool,
        debug_lines: Vec<String>,
    }
    let Some(s) = app.session.as_ref() else {
        return;
    };
    let snap = Snap {
        health: s.health,
        hunger: s.hunger,
        karma: s.karma,
        time_of_day: s.time_of_day,
        weather: s.weather,
        message: s.message.clone(),
        hotbar: s
            .inventory
            .slots
            .iter()
            .take(9)
            .map(|x| x.as_ref().map(slot_view))
            .collect(),
        selected: s.inventory.selected,
        break_progress: s.break_target.as_ref().map(|b| b.progress).unwrap_or(0.0),
        npc_prompt: {
            let mut p = None;
            for npc in &s.npcs {
                if npc.pos.distance(s.player.eye()) < 4.0 {
                    p = Some(format!("[F] Hablar con {}", npc.name));
                }
            }
            p
        },
        flash: s.dimension_switch_flash,
        stats_chunks: s.stats_chunks,
        pos: s.player.pos,
        dimension: s.dimension,
        edits: s.edits.len(),
        in_dialogue: s.dialogue.is_some(),
        debug_lines: if app.debug {
            vec![
                format!("RIVAREN {} — FPS {:.0}", env!("CARGO_PKG_VERSION"), app.fps),
                format!("xyz {:.1} {:.1} {:.1}", s.player.pos.x, s.player.pos.y, s.player.pos.z),
                format!(
                    "chunk {} {} {} | chunks {}",
                    (s.player.pos.x / 32.0).floor() as i32,
                    (s.player.pos.y / 32.0).floor() as i32,
                    (s.player.pos.z / 32.0).floor() as i32,
                    s.stats_chunks
                ),
                format!(
                    "dim {:?} | karma {:.0}/{:.0}/{:.0} | edits {}",
                    s.dimension, s.karma.compasion, s.karma.justicia, s.karma.sabiduria, s.edits.len()
                ),
                format!(
                    "mobs {} | NPCs {} | {}",
                    s.mobs.len(),
                    s.npcs.len(),
                    s.mobs
                        .iter()
                        .min_by(|a, b| {
                            a.pos
                                .distance(s.player.pos)
                                .partial_cmp(&b.pos.distance(s.player.pos))
                                .unwrap_or(std::cmp::Ordering::Equal)
                        })
                        .map(|m| format!("cerca: {}", m.species.name()))
                        .unwrap_or_else(|| "sin mobs".into())
                ),
                format!(
                    "draws {} | quads {} | textos {} | presupuesto {:.0} ms (CPU {:.0} / GPU {:.0})",
                    app.renderer.as_ref().map(|r| r.stats.draw_calls).unwrap_or(0),
                    app.renderer.as_ref().map(|r| r.stats.quads_ui).unwrap_or(0),
                    app.renderer.as_ref().map(|r| r.stats.texts_ui).unwrap_or(0),
                    app.budget.frame_ms,
                    app.budget.cpu_ms,
                    app.budget.gpu_ms,
                ),
            ]
        } else {
            Vec::new()
        },
    };
    let _ = s;

    let r = screen_rect(ui);
    let scale = app.settings.game.hud_scale;
    let cx = r.w * 0.5;
    let cy = r.h * 0.5;
    // Crosshair.
    ui.draw.rect(cx - 9.0, cy - 1.0, 18.0, 2.0, [1.0, 1.0, 1.0, 0.75]);
    ui.draw.rect(cx - 1.0, cy - 9.0, 2.0, 18.0, [1.0, 1.0, 1.0, 0.75]);
    if snap.break_progress > 0.0 {
        ui.draw.rect(cx - 30.0, cy + 22.0, 60.0 * snap.break_progress, 4.0, [0.95, 0.8, 0.3, 0.9]);
    }
    // Hotbar.
    let slot = 46.0 * scale;
    let total = slot * 9.0 + 8.0 * 8.0;
    let hx = cx - total * 0.5;
    let hy = r.h - slot - 18.0 * scale;
    let mut clicked_hotbar: Option<usize> = None;
    for i in 0..9 {
        let rect = Rect::new(hx + i as f32 * (slot + 8.0), hy, slot, slot);
        let view = snap.hotbar.get(i).and_then(|x| x.clone());
        let resp = ui.item_slot(4000 + i as u64, rect, view.as_ref(), i == snap.selected);
        if resp.clicked {
            clicked_hotbar = Some(i);
        }
        if resp.hovered {
            if let Some(v) = &view {
                ui.tooltip(rect, &v.label);
            }
        }
    }
    if let Some(i) = clicked_hotbar {
        if let Some(sm) = app.session.as_mut() {
            sm.inventory.selected = i;
        }
    }
    // Vida y hambre.
    let bar_w = 160.0 * scale;
    let bar_h = 12.0 * scale;
    ui.progress(Rect::new(cx - bar_w - 12.0, hy - bar_h - 10.0, bar_w, bar_h), snap.health / 20.0, [0.85, 0.25, 0.28, 1.0]);
    ui.progress(Rect::new(cx + 12.0, hy - bar_h - 10.0, bar_w, bar_h), snap.hunger / 20.0, [0.85, 0.62, 0.25, 1.0]);
    // Karma.
    let kx = r.w - 190.0 * scale;
    let ky = 20.0 * scale;
    ui.draw.rounded(kx, ky, 170.0 * scale, 74.0 * scale, RADIUS_SMALL, [0.05, 0.05, 0.07, 0.72]);
    ui.draw.text("Balanza del Alma", kx + 10.0, ky + 6.0, SMALL_SIZE * scale, ui.palette.text_dim);
    let bars = [
        ("Compasión", snap.karma.compasion, ui.palette.compassion),
        ("Justicia", snap.karma.justicia, ui.palette.justice),
        ("Sabiduría", snap.karma.sabiduria, ui.palette.wisdom),
    ];
    for (i, (label, v, color)) in bars.iter().enumerate() {
        let y = ky + 24.0 + i as f32 * 15.0 * scale;
        ui.draw.text(*label, kx + 10.0, y, 11.0 * scale, ui.palette.text_dim);
        let track = Rect::new(kx + 80.0 * scale, y + 2.0, 80.0 * scale, 7.0 * scale);
        ui.draw.rounded(track.x, track.y, track.w, track.h, 3.0, [0.18, 0.18, 0.22, 1.0]);
        let t = ((v + 100.0) / 200.0).clamp(0.0, 1.0);
        ui.draw.rounded(track.x, track.y, track.w * t, track.h, 3.0, *color);
    }
    let day = (snap.time_of_day * 24.0 + 6.0) % 24.0;
    let label = format!(
        "{:02.0}:{:02.0}  {}",
        day.floor(),
        (day.fract() * 60.0) as u32,
        if snap.weather > 0.5 { "Lluvia" } else { "Despejado" }
    );
    ui.draw.text(label, kx + 10.0, ky + 74.0 * scale + 6.0, SMALL_SIZE * scale, ui.palette.text);
    if let Some((msg, ttl)) = &snap.message {
        let a = ttl.min(1.0);
        ui.draw.text_centered(msg, cx, r.h * 0.30, BODY_SIZE * scale, [1.0, 0.95, 0.8, a]);
    }
    if let Some(p) = &snap.npc_prompt {
        ui.draw.text_centered(p, cx, r.h * 0.58, BODY_SIZE * scale, ui.palette.text_title);
    }
    if app.debug {
        let w = 420.0;
        ui.draw.rect(8.0, 8.0, w, 18.0 + snap.debug_lines.len() as f32 * 18.0, [0.0, 0.0, 0.0, 0.6]);
        for (i, l) in snap.debug_lines.iter().enumerate() {
            ui.draw.text(l, 16.0, 14.0 + i as f32 * 18.0, 14.0, [0.85, 0.95, 0.85, 1.0]);
        }
    }
    if snap.in_dialogue {
        ui.draw.text_centered("En conversación...", cx, r.h - 90.0, SMALL_SIZE, ui.palette.accent);
    }
    if snap.flash > 0.0 {
        ui.draw.rect(0.0, 0.0, r.w, r.h, [0.05, 0.02, 0.08, snap.flash * 0.8]);
    }
    let _ = &snap.stats_chunks;
    let _ = (snap.pos, snap.dimension, snap.edits);
}

fn slot_view(st: &rivaren_gameplay::ItemStack) -> SlotView {
    let def = item_def(st.id);
    SlotView {
        color: [def.color[0], def.color[1], def.color[2], 1.0],
        label: def.name.to_string(),
        count: st.count,
        durability: if def.durability > 0 {
            st.durability as f32 / def.durability as f32
        } else {
            1.0
        },
    }
}

// ── Pausa ────────────────────────────────────────────────────────

fn pause(app: &mut App, ui: &mut Ui) {
    let r = screen_rect(ui);
    ui.dim(r, 0.55);
    let card = Stack::centered_card(r, 380.0, 440.0);
    ui.panel_titled(card, "Pausa");
    let mut col = Stack::new(
        Rect::new(card.x + 40.0, card.y + 70.0, card.w - 80.0, card.h - 110.0),
        Dir::Column,
        10.0,
    );
    col.next(Size::Fill);
    if ui
        .button_primary(5000, col.next(Size::Fixed(42.0)), "Continuar")
        .clicked
    {
        app.screen = Screen::Game;
        app.set_cursor_held(true);
    }
    if ui.button(5001, col.next(Size::Fixed(42.0)), "Ajustes").clicked {
        app.settings_tab = 0;
        app.screen = Screen::Settings;
    }
    if ui.button(5002, col.next(Size::Fixed(42.0)), "Guardar").clicked {
        app.save_current();
    }
    if ui.button(5003, col.next(Size::Fixed(42.0)), "Salir al menú").clicked {
        app.save_current();
        app.session = None;
        app.screen = Screen::MainMenu;
        app.set_cursor_held(false);
    }
    col.next(Size::Fill);
    let _ = &mut col;
}

// ── Inventario y crafteo ─────────────────────────────────────────

fn inventory(app: &mut App, ui: &mut Ui, table: bool) {
    let r = screen_rect(ui);
    ui.dim(r, 0.5);
    let Some(session_ref) = app.session.as_ref() else {
        return;
    };
    let inv = session_ref.inventory.clone();
    let cols = if table { 3 } else { 2 };
    let cell = 46.0;
    let gap = 6.0;
    let grid_w = 9.0 * (cell + gap) - gap;
    let craft_w = cols as f32 * (cell + gap) - gap;
    let panel_w = (grid_w + craft_w + 90.0).min(r.w - 40.0);
    let panel_h = (cell * 4.0 + gap * 3.0 + 130.0).min(r.h - 40.0);
    let card = r.align(panel_w, panel_h);
    ui.panel_titled(card, if table { "Mesa de Crafteo" } else { "Inventario" });
    // Rejilla principal 3×9.
    let grid = Rect::new(card.x + 20.0, card.y + 60.0, grid_w, cell * 3.0 + gap * 2.0);
    let mut clicked_slot: Option<usize> = None;
    for row in 0..3 {
        for colx in 0..9 {
            let idx = 9 + row * 9 + colx;
            let rect = Rect::new(
                grid.x + colx as f32 * (cell + gap),
                grid.y + row as f32 * (cell + gap),
                cell,
                cell,
            );
            let view = inv.slots.get(idx).and_then(|x| x.as_ref()).map(slot_view);
            let resp = ui.item_slot(4100 + idx as u64, rect, view.as_ref(), false);
            if resp.clicked {
                clicked_slot = Some(idx);
            }
            if resp.hovered {
                if let Some(v) = &view {
                    ui.tooltip(rect, &v.label);
                }
            }
        }
    }
    // Crafteo.
    let craft = Rect::new(
        grid.x + grid.w + 24.0,
        card.y + 60.0,
        craft_w,
        cols as f32 * (cell + gap) - gap,
    );
    for cy in 0..cols {
        for cx in 0..cols {
            let i = cy * cols + cx;
            let rect = Rect::new(
                craft.x + cx as f32 * (cell + gap),
                craft.y + cy as f32 * (cell + gap),
                cell,
                cell,
            );
            let view = inv.craft.get(i).and_then(|x| x.as_ref()).map(slot_view);
            if ui.item_slot(4200 + i as u64, rect, view.as_ref(), false).clicked {
                clicked_slot = Some(1000 + i);
            }
        }
    }
    // Resultado.
    let result_rect = Rect::new(craft.x, craft.y + craft_w + 20.0, cell, cell);
    let grid_view: Vec<Option<rivaren_gameplay::ItemStack>> =
        inv.craft.iter().take(cols * cols).copied().collect();
    let result = try_craft(&grid_view, cols, cols, table, &rivaren_gameplay::recipe_book());
    let rview = result.as_ref().map(|(st, _)| slot_view(st));
    let rresp = ui.item_slot(4300, result_rect, rview.as_ref(), result.is_some());
    if let Some((stack, slots)) = result {
        if rresp.clicked {
        if let Some(sm) = app.session.as_mut() {
            sm.inventory.add(stack);
            for si in slots {
                sm.inventory.craft[si] = None;
            }
            if !sm.crafted.contains(&stack.id) {
                sm.crafted.push(stack.id);
            }
            sm.audio.play(rivaren_audio::Sfx::Craft, 0.8, 0.0);
        }
        }
    }
    // Hotbar.
    let hotbar = Rect::new(grid.x, grid.y + grid.h + 24.0, grid_w, cell);
    for i in 0..9 {
        let rect = Rect::new(hotbar.x + i as f32 * (cell + gap), hotbar.y, cell, cell);
        let view = inv.slots.get(i).and_then(|x| x.as_ref()).map(slot_view);
        if ui
            .item_slot(4400 + i as u64, rect, view.as_ref(), i == inv.selected)
            .clicked
        {
            clicked_slot = Some(i);
        }
    }
    // Cursor.
    if let Some(item) = app.cursor_item {
        let def = item_def(item.id);
        let m = 12.0;
        ui.draw.rounded(
            ui.input.mouse_x - m,
            ui.input.mouse_y - m,
            m * 2.0,
            m * 2.0,
            4.0,
            [def.color[0], def.color[1], def.color[2], 1.0],
        );
        if item.count > 1 {
            ui.draw.text(
                format!("{}", item.count),
                ui.input.mouse_x + m,
                ui.input.mouse_y + m,
                SMALL_SIZE,
                ui.palette.text,
            );
        }
    }
    // Gestión del clic.
    if let Some(slot) = clicked_slot {
        let sm = app.session.as_mut().unwrap();
        if slot >= 1000 {
            let ci = slot - 1000;
            let cur = sm.inventory.craft[ci];
            let taken = std::mem::replace(&mut sm.inventory.craft[ci], app.cursor_item.take());
            app.cursor_item = match (cur, taken) {
                (Some(a), Some(b)) if a.id == b.id => {
                    let max = item_def(a.id).stack_max.max(1);
                    let total = a.count as u32 + b.count as u32;
                    if total <= max as u32 {
                        Some(rivaren_gameplay::ItemStack {
                            id: a.id,
                            count: total as u8,
                            durability: a.durability,
                        })
                    } else {
                        sm.inventory.craft[ci] = Some(rivaren_gameplay::ItemStack {
                            id: a.id,
                            count: (total - max as u32) as u8,
                            durability: a.durability,
                        });
                        Some(rivaren_gameplay::ItemStack {
                            id: a.id,
                            count: max,
                            durability: a.durability,
                        })
                    }
                }
                (Some(a), Some(b)) => {
                    sm.inventory.craft[ci] = Some(b);
                    Some(a)
                }
                (Some(a), None) => Some(a),
                (None, Some(b)) => {
                    sm.inventory.craft[ci] = Some(b);
                    None
                }
                (None, None) => None,
            };
        } else {
            let cur = sm.inventory.slots.get(slot).copied().flatten();
            let taken = std::mem::replace(&mut sm.inventory.slots[slot], app.cursor_item.take());
            app.cursor_item = match (cur, taken) {
                (Some(a), Some(b)) if a.id == b.id => {
                    let max = item_def(a.id).stack_max.max(1);
                    let total = a.count as u32 + b.count as u32;
                    if total <= max as u32 {
                        Some(rivaren_gameplay::ItemStack {
                            id: a.id,
                            count: total as u8,
                            durability: a.durability,
                        })
                    } else {
                        sm.inventory.slots[slot] = Some(rivaren_gameplay::ItemStack {
                            id: a.id,
                            count: (total - max as u32) as u8,
                            durability: a.durability,
                        });
                        Some(rivaren_gameplay::ItemStack {
                            id: a.id,
                            count: max,
                            durability: a.durability,
                        })
                    }
                }
                (Some(a), Some(b)) => {
                    sm.inventory.slots[slot] = Some(b);
                    Some(a)
                }
                (Some(a), None) => Some(a),
                (None, Some(b)) => {
                    sm.inventory.slots[slot] = Some(b);
                    None
                }
                (None, None) => None,
            };
        }
    }
    // Botones.
    let close = Rect::new(
        card.x + card.w - 190.0,
        card.y + card.h - 56.0,
        80.0,
        38.0,
    );
    let mut back = false;
    if app.cursor_item.is_none()
        && ui.button(4500, close, "Cerrar").clicked
    {
        back = true;
    }
    if ui.input.key_pressed(rivaren_ui::input::Key::Escape) {
        back = true;
    }
    if back {
        if app.cursor_item.is_some() {
            if let Some(sm) = app.session.as_mut() {
                if let Some(item) = app.cursor_item.take() {
                    sm.inventory.add(item);
                }
            }
        }
        app.screen = Screen::Game;
        app.set_cursor_held(true);
    }
}

// ── Diálogo ──────────────────────────────────────────────────────

fn dialogue(app: &mut App, ui: &mut Ui) {
    let r = screen_rect(ui);
    ui.dim(r, 0.35);
    let Some(session) = app.session.as_mut() else {
        return;
    };
    let Some(dlg) = session.dialogue.as_ref() else {
        app.screen = Screen::Game;
        app.set_cursor_held(true);
        return;
    };
    let npc_index = dlg.npc_index;
    let node_id = dlg.node;
    let npc_name = session.npcs[npc_index].name.clone();
    let npc_kind = session.npcs[npc_index].kind;
    let node_text = session.npcs[npc_index]
        .tree
        .node(node_id)
        .map(|n| n.text.to_string())
        .unwrap_or_default();
    let options: Vec<String> = session.npcs[npc_index]
        .tree
        .node(node_id)
        .map(|n| n.options.iter().map(|o| o.label.to_string()).collect())
        .unwrap_or_default();
    let ai_text = dlg.ai_text.clone();
    let ai_pending = dlg.ai_pending;
    let history: Vec<(String, String)> = dlg.history.clone();
    let ai_mode = app.settings.game.ai_mode;
    let _ = session;

    let card = Stack::centered_card(r, 700.0, 520.0);
    ui.panel(card);
    // Cabecera.
    ui.draw
        .rect(card.x, card.y, card.w, 56.0, [0.16, 0.14, 0.12, 1.0]);
    ui.draw.text(
        &npc_name,
        card.x + SPACE,
        card.y + 16.0,
        HEADING_SIZE,
        ui.palette.text_title,
    );
    ui.draw.text(
        npc_kind,
        card.x + SPACE,
        card.y + 38.0,
        SMALL_SIZE,
        ui.palette.text_dim,
    );
    // Cuerpo.
    let body = Rect::new(
        card.x + SPACE,
        card.y + 70.0,
        card.w - SPACE * 2.0,
        card.h - 190.0,
    );
    ui.draw
        .rounded(body.x, body.y, body.w, body.h, RADIUS_SMALL, [0.06, 0.06, 0.08, 0.9]);
    let mut y = body.y + 10.0;
    // Historial (últimas 4 líneas).
    let start = history.len().saturating_sub(4);
    for (who, what) in history[start..].iter() {
        ui.draw.text(
            format!("{who}: {what}"),
            body.x + 10.0,
            y,
            SMALL_SIZE,
            ui.palette.text_dim,
        );
        y += 20.0;
    }
    // Texto actual del árbol.
    ui.draw.text(&node_text, body.x + 10.0, y, BODY_SIZE, ui.palette.text);
    y += 26.0;
    // Texto IA en streaming.
    if ai_pending || !ai_text.is_empty() {
        ui.draw.text(
            format!("{}: {}{}", npc_name, ai_text, if ai_pending { "▌" } else { "" }),
            body.x + 10.0,
            y,
            BODY_SIZE,
            [0.85, 0.9, 1.0, 1.0],
        );
    }
    // Opciones.
    let mut chosen: Option<usize> = None;
    let opts_rect = Rect::new(card.x + SPACE, card.y + card.h - 150.0, card.w - SPACE * 2.0, 90.0);
    let mut col = Stack::new(opts_rect, Dir::Column, 6.0);
    for (i, o) in options.iter().enumerate() {
        let rect = col.next(Size::Fixed(28.0));
        if ui.button(6000 + i as u64, rect, o).clicked {
            chosen = Some(i);
        }
    }
    // Entrada libre si hay IA (modo local/nube).
    let mut ask_text: Option<String> = None;
    if ai_mode > 0 {
        let input_rect = Rect::new(card.x + SPACE, card.y + card.h - 52.0, card.w - 220.0, 36.0);
        let buf = app.text_buffers.entry(7000).or_default();
        let mut text = buf.clone();
        let _field = ui.text_field(7000, input_rect, &mut text, "Pregunta libre al NPC...");
        *buf = text.clone();
        if let Some(session) = app.session.as_mut() {
            if let Some(d) = session.dialogue.as_mut() {
                d.input = text;
            }
        }
        let send = Rect::new(
            card.x + card.w - 160.0,
            card.y + card.h - 52.0,
            100.0,
            36.0,
        );
        if ui.button_primary(7100, send, "Preguntar").clicked {
            let q = app.text_buffers.get(&7000).cloned().unwrap_or_default();
            if !q.trim().is_empty() {
                ask_text = Some(q);
                app.text_buffers.insert(7000, String::new());
            }
        }
    }
    let close = Rect::new(card.x + card.w - 56.0, card.y + 12.0, 40.0, 32.0);
    let mut close_clicked = false;
    if ui.button(7200, close, "X").clicked {
        close_clicked = true;
    }
    if ui.input.key_pressed(rivaren_ui::input::Key::Escape) {
        close_clicked = true;
    }
    // Acciones (sin doble préstamo).
    let mut leave = close_clicked;
    if let Some(session) = app.session.as_mut() {
        if let Some(i) = chosen {
            session.dialogue_choose(i);
            leave = session.dialogue.is_none();
        }
        if let Some(q) = ask_text {
            session.dialogue_ask(q);
        }
        if close_clicked {
            session.close_dialogue();
        }
    }
    if leave {
        app.screen = Screen::Game;
        app.set_cursor_held(true);
    }
}

// ── Diario ───────────────────────────────────────────────────────

fn journal(app: &mut App, ui: &mut Ui) {
    let r = screen_rect(ui);
    ui.dim(r, 0.5);
    let Some(s) = app.session.as_ref() else {
        return;
    };
    let card = Stack::centered_card(r, 760.0, 560.0);
    ui.panel_titled(card, "Diario del Alma");
    let list = Rect::new(
        card.x + SPACE,
        card.y + 60.0,
        card.w - SPACE * 2.0,
        card.h - 130.0,
    );
    let mut y = list.y;
    for q in &s.quests.quests {
        let (color, state) = match q.state {
            QuestState::Done => (ui.palette.success, "COMPLETADA"),
            QuestState::Active => (ui.palette.accent, "ACTIVA"),
            QuestState::Available => (ui.palette.text, "DISPONIBLE"),
            QuestState::Locked => (ui.palette.text_dim, "BLOQUEADA"),
        };
        ui.draw.text(
            format!("{} — {}", q.title, state),
            list.x,
            y,
            18.0,
            color,
        );
        y += 22.0;
        ui.draw.text(q.description, list.x + 14.0, y, SMALL_SIZE, ui.palette.text_dim);
        y += 18.0;
        for o in &q.objectives {
            let txt = match o {
                Objective::Collect { item, count } => {
                    let have = s.inventory.count_of(*item);
                    format!("• {} x{} ({}/{})", item_def(*item).name, count, have, count)
                }
                Objective::Kill { mob, count } => format!("• Derrota {} x{}", mob, count),
                Objective::Reach { dim } => format!(
                    "• Llega a {}",
                    match dim {
                        1 => "el Infierno",
                        2 => "el Cielo",
                        _ => "la Tierra",
                    }
                ),
                Objective::Talk { npc } => format!("• Habla con {npc}"),
                Objective::Karma { axis, min } => format!(
                    "• Alcanza {:.0} de {}",
                    min,
                    match axis {
                        0 => "Compasión",
                        1 => "Justicia",
                        _ => "Sabiduría",
                    }
                ),
                Objective::Craft { item } => format!("• Fabrica {}", item_def(*item).name),
            };
            let done = match o {
                Objective::Collect { item, count } => s.inventory.count_of(*item) >= *count,
                Objective::Reach { dim } => match dim {
                    0 => s.dimension == rivaren_gameplay::Dimension::Tierra,
                    1 => s.dimension == rivaren_gameplay::Dimension::Infierno,
                    _ => s.dimension == rivaren_gameplay::Dimension::Cielo,
                },
                Objective::Talk { npc } => s.talked.iter().any(|t| t == npc),
                Objective::Karma { axis, min } => match axis {
                    0 => s.karma.compasion >= *min,
                    1 => s.karma.justicia >= *min,
                    _ => s.karma.sabiduria >= *min,
                },
                Objective::Craft { item } => s.crafted.contains(item),
                Objective::Kill { .. } => false,
            };
            ui.draw.text(
                txt,
                list.x + 20.0,
                y,
                SMALL_SIZE,
                if done { ui.palette.success } else { ui.palette.text },
            );
            y += 17.0;
        }
        y += 8.0;
        if y > list.y + list.h - 20.0 {
            break;
        }
    }
    let btn = Rect::new(card.x + SPACE, card.y + card.h - 62.0, 160.0, 42.0);
    if ui.button(8000, btn, "Volver").clicked
        || ui.input.key_pressed(rivaren_ui::input::Key::Escape)
    {
        app.screen = Screen::Game;
        app.set_cursor_held(true);
    }
}

// ── Muerte ───────────────────────────────────────────────────────

fn death(app: &mut App, ui: &mut Ui) {
    let r = screen_rect(ui);
    ui.draw.rect(0.0, 0.0, r.w, r.h, [0.25, 0.02, 0.03, 0.85]);
    let card = Stack::centered_card(r, 520.0, 300.0);
    ui.panel(card);
    ui.draw.text_centered(
        "Has caído",
        card.center_x(),
        card.y + 40.0,
        TITLE_SIZE * 0.7,
        ui.palette.danger,
    );
    let Some(s) = app.session.as_mut() else {
        return;
    };
    let (dest, dim) = s
        .respawn
        .on_death(s.dimension, &s.karma, s.seed);
    let text = match dim {
        rivaren_gameplay::Dimension::Tierra => "Despiertas en tu hogar.",
        rivaren_gameplay::Dimension::Cielo => "El Cielo te recibe: tu alma era ligera.",
        rivaren_gameplay::Dimension::Infierno => "El Infierno te reclama: aún hay deudas.",
    };
    ui.draw.text_centered(
        text,
        card.center_x(),
        card.y + 100.0,
        BODY_SIZE,
        ui.palette.text,
    );
    let btn = Rect::new(card.x + 60.0, card.y + card.h - 90.0, card.w - 120.0, 46.0);
    if ui.button_primary(9000, btn, "Reaparecer").clicked {
        s.health = 20.0;
        s.hunger = 20.0;
        s.player.pos = glam::Vec3::new(dest[0] as f32, dest[1] as f32 + 2.0, dest[2] as f32);
        s.player.vel = glam::Vec3::ZERO;
        if dim != s.dimension {
            s.switch_dimension(dim);
        }
        app.screen = Screen::Game;
        app.set_cursor_held(true);
    }
    let quit = Rect::new(card.x + 60.0, card.y + card.h - 36.0, card.w - 120.0, 28.0);
    if ui.button(9001, quit, "Salir al menú").clicked {
        app.session = None;
        app.screen = Screen::MainMenu;
        app.set_cursor_held(false);
    }
}
