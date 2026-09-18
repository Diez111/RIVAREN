//! RIVAREN — aplicación multiplataforma (Linux/Windows/macOS/Android).
//! Ventana winit + renderer wgpu + UI propia + juego completo.

mod app;
mod budget;
mod game;
mod jobs;
mod save;
mod screens;

use anyhow::Result;

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();
    tracing::info!("RIVAREN {} — motor {} ", env!("CARGO_PKG_VERSION"), rivaren_render::GPU_BACKEND_NAME);

    let seed: u64 = std::env::var("RIVAREN_SEED")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(1337);
    let args: Vec<String> = std::env::args().collect();
    if let Some(i) = args.iter().position(|a| a == "--screenshot") {
        let path = args.get(i + 1).cloned().unwrap_or_else(|| "/tmp/rivaren.ppm".into());
        return run_screenshot(&path);
    }
    if let Some(i) = args.iter().position(|a| a == "--screenshot-ingame") {
        let path = args.get(i + 1).cloned().unwrap_or_else(|| "/tmp/rivaren_game.ppm".into());
        return run_screenshot_ingame(seed, &path);
    }
    if args.iter().any(|a| a == "--heights") {
        let mut above = 0;
        let mut below = 0;
        let mut min = f32::MAX;
        let mut max = f32::MIN;
        let mut sum = 0.0;
        let mut n = 0;
        for dz in -20..20 {
            for dx in -20..20 {
                let x = dx as f32 * 64.0;
                let z = dz as f32 * 64.0;
                let h = rivaren_world::sdf::surface_height(seed, x, z);
                min = min.min(h);
                max = max.max(h);
                sum += h;
                n += 1;
                if h > 62.0 {
                    above += 1;
                } else {
                    below += 1;
                }
            }
        }
        println!(
            "heights seed={seed}: min={min:.0} max={max:.0} media={:.0} tierra={above} agua={below}",
            sum / n as f32
        );
        return Ok(());
    }

    if args.iter().any(|a| a == "--headless") {
        return run_headless_demo(seed);
    }
    app::run()
}

/// Renderiza un frame de UI en offscreen y lo guarda como PPM (verificación).
fn run_screenshot(path: &str) -> Result<()> {
    use rivaren_core::DrawList;
    use rivaren_render::{QualityTier, Renderer, SceneParams};
    use rivaren_ui::theme::Breakpoint;
    use rivaren_ui::widgets::Ui;

    let mut renderer = Renderer::new(None, (960, 600), QualityTier::Balanced)?;
    let mut app = app::App {
        screen: app::Screen::MainMenu,
        settings: rivaren_gameplay::Settings::default(),
        world_cfg: rivaren_gameplay::WorldConfig::default(),
        session: None,
        renderer: None,
        window: None,
        input: Default::default(),
        draw: DrawList::default(),
        frame: 1,
        last: std::time::Instant::now(),
        fps: 60.0,
        fps_acc: (0.0, 0),
        debug: true,
        cursor_held: false,
        cursor_item: None,
        text_buffers: Default::default(),
        mods_loaded: vec![],
        error: None,
        save_name: "mundo".into(),
        settings_tab: 0,
        quickstart: false,
        budget: crate::budget::FrameBudget::new(60),
        loading_started: None,
    };
    app.input.screen = (960.0, 600.0);
    let mut draw = DrawList::default();
    let mut input = app.input.clone();
    input.begin_frame();
    {
        let mut ui = Ui::new(&mut draw, &input, Breakpoint::of(960.0), 1);
        screens::draw(&mut app, &mut ui);
    }
    let scene = SceneParams {
        camera: rivaren_render::Camera {
            pos: glam::Vec3::new(0.0, 110.0, 0.0),
            yaw: 0.6,
            pitch: -0.15,
            aspect: 960.0 / 600.0,
            fov: 70f32.to_radians(),
            ..Default::default()
        },
        time_of_day: 0.35,
        ..Default::default()
    };
    // Dos frames para estabilizar TAA/historial.
    for _ in 0..2 {
        renderer.render(&scene, &draw)?;
    }
    renderer.capture_ppm(path)?;
    println!(
        "screenshot: {path} | quads={} textos={} draws={}",
        draw.quads.len(),
        draw.texts.len(),
        renderer.stats.draw_calls
    );
    Ok(())
}

/// Renderiza el juego real (streaming + jugador) y guarda un PPM.
fn run_screenshot_ingame(seed: u64, path: &str) -> Result<()> {
    use rivaren_render::{QualityTier, Renderer, SceneParams};

    let mut renderer = Renderer::new(None, (1280, 720), QualityTier::Balanced)?;
    let settings = rivaren_gameplay::Settings::default();
    let cfg = rivaren_gameplay::WorldConfig {
        seed,
        render_distance: 4,
        gamemode: rivaren_gameplay::Gamemode::Creative,
        ..Default::default()
    };
    let mut session = game::GameSession::new(seed, cfg, &settings);
    // Asegura chunks alrededor del spawn antes de la demo.
    for _ in 0..900 {
        session.update_streaming(&mut renderer);
        if session.stats_chunks > 160 {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(8));
    }
    // Espera a que terminen los trabajos pendientes.
    for _ in 0..200 {
        session.update_streaming(&mut renderer);
        if session.stats_chunks > 200 {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    // El spawn ya viene de find_spawn (superficie clamps); ajusta vista.
    session.player.flying = true;
    session.player.yaw = 0.7;
    session.player.pitch = -0.15;
    let h = session.player.pos.y;
    session.spawn_npc("Iluminado", glam::Vec3::new(6.0, h + 1.0, 4.0));
    // Demo de Pulso: palanca + cables + lámpara dentro del chunk del jugador.
    let pc = glam::IVec3::new(
        (session.player.pos.x / 32.0).floor() as i32,
        (session.player.pos.y / 32.0).floor() as i32,
        (session.player.pos.z / 32.0).floor() as i32,
    );
    while !session.chunks.contains_key(&pc) {
        session.update_streaming(&mut renderer);
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let base = pc * 32;
    let lx = (session.player.pos.x as i32).rem_euclid(32);
    let lz = (session.player.pos.z as i32).rem_euclid(32);
    let py = base.y + (session.player.pos.y as i32).rem_euclid(32) - 2;
    let bx = base.x + lx;
    let bz = base.z + lz;
    session.set_block(bx, py, bz, 82); // palanca
    session
        .pulso
        .place([bx, py, bz], rivaren_physics::pulso::PulsoKind::Lever);
    for x in [bx + 1, bx + 2, bx + 3] {
        session.set_block(x, py, bz, 80); // cable
        session
            .pulso
            .place([x, py, bz], rivaren_physics::pulso::PulsoKind::Cable);
    }
    session.set_block(bx + 4, py, bz, 86); // lámpara apagada
    session
        .pulso
        .place([bx + 4, py, bz], rivaren_physics::pulso::PulsoKind::Lamp);
    session.pulso.toggle([bx, py, bz]);
    for _ in 0..10 {
        session.tick_pulso();
    }
    if session.chunks.contains_key(&pc) {
        session.remesh(pc, &mut renderer);
    }
    // Cámara mirando el circuito.
    let eye = glam::Vec3::new(
        (bx - 5) as f32 + 0.5,
        (py + 3) as f32,
        (bz - 5) as f32 + 0.5,
    );
    let target = glam::Vec3::new(
        (bx + 2) as f32 + 0.5,
        py as f32 + 0.5,
        bz as f32 + 0.5,
    );
    session.player.pos = eye;
    let dir = (target - eye).normalize();
    session.player.yaw = dir.x.atan2(-dir.z);
    session.player.pitch = dir.y.asin();
    for _ in 0..10 {
        session.update_streaming(&mut renderer);
        std::thread::sleep(std::time::Duration::from_millis(8));
    }
    println!(
        "  pulso: palanca=({bx},{py},{bz}) lámpara=({},{py},{bz}) → bloque {} (87 = encendida)",
        bx + 4,
        session.block_at(bx + 4, py, bz)
    );
    session.message = Some(("RIVAREN — captura en juego".into(), 5.0));
    // Simula unos ticks para estabilizar.
    for _ in 0..40 {
        session.tick(1.0 / 60.0);
        session.update_streaming(&mut renderer);
        std::thread::sleep(std::time::Duration::from_millis(6));
    }
    // Construye un frame de HUD con el toolkit.
    let mut app_draw = rivaren_core::DrawList::default();
    {
        let input = rivaren_ui::input::InputState {
            screen: (1280.0, 720.0),
            ..Default::default()
        };
        let mut ui = rivaren_ui::widgets::Ui::new(
            &mut app_draw,
            &input,
            rivaren_ui::theme::Breakpoint::of(1280.0),
            10,
        );
        screens::draw_hud_only(&session, &mut ui, 60.0);
    }
    let scene = SceneParams {
        camera: session.player.camera(1280.0 / 720.0),
        time_of_day: 0.35,
        underwater: false,
        weather: 0.0,
        exposure: 1.0,
        bloom: 0.06,
        sharpen: 0.25,
        taa: true,
        render_scale: 1.0,
        fog_density: 1.0,
    };
    // Spawnea algunos mobs para la captura.
    for _ in 0..14 {
        session.try_spawn_mob(match session.next_mob_id % 4 {
            0 => crate::game::MobSpecies::Uro,
            1 => crate::game::MobSpecies::Jabali,
            2 => crate::game::MobSpecies::ZorroBruma,
            _ => crate::game::MobSpecies::Uro,
        });
    }
    for _ in 0..30 {
        session.tick_mobs(1.0 / 20.0);
    }
    let mut entities: Vec<rivaren_render::EntityInstance> = Vec::new();
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
    renderer.set_entities(&entities);
    for _ in 0..6 {
        renderer.render(&scene, &app_draw)?;
    }
    println!("  mobs: {} entidades", entities.len());
    renderer.capture_ppm(path)?;
    println!(
        "screenshot ingame: {path} | chunks={} draws={} quads_ui={}",
        session.stats_chunks, renderer.stats.draw_calls, renderer.stats.quads_ui
    );
    Ok(())
}

fn run_headless_demo(seed: u64) -> Result<()> {
    use rivaren_compression::{BrickPool, SvdagBuilder, build_aadf};
    use rivaren_meshing::{MeshData, bake_lighting, greedy_mesh_lit};
    use rivaren_world::pipeline::generate_chunk;
    use std::sync::atomic::AtomicBool;
    use std::time::Instant;

    let cancel = AtomicBool::new(false);
    println!("RIVAREN headless — seed={seed}");
    for cy in [2, 1, 3, 0] {
        let chunk = glam::IVec3::new(0, cy, 0);
        let t0 = Instant::now();
        let b = generate_chunk(seed, chunk, &cancel);
        let gen_ms = t0.elapsed().as_secs_f64() * 1000.0;
        let nonempty = b.voxels.iter().filter(|&&v| v != 0).count();
        if nonempty < 1000 {
            continue;
        }
        let t1 = Instant::now();
        let light = bake_lighting(&b.voxels);
        let light_us = t1.elapsed().as_micros();
        let t2 = Instant::now();
        let mut mesh = MeshData::default();
        greedy_mesh_lit(&b.voxels, &light, &mut mesh);
        let mesh_us = t2.elapsed().as_micros();
        let t3 = Instant::now();
        let mut svdag = SvdagBuilder::new();
        let dag = svdag.build(&b.voxels);
        let dag_us = t3.elapsed().as_micros();
        let t4 = Instant::now();
        let _aadf = build_aadf(&dag);
        let aadf_us = t4.elapsed().as_micros();
        let mut pool = BrickPool::new();
        let _ = pool.intern_chunk(&b.voxels);
        println!("  chunk {chunk:?}: {gen_ms:.2} ms, {nonempty} sólidos");
        println!("    luz    {light_us} µs | meshing {mesh_us} µs, {} quads, {} verts", mesh.quad_count(), mesh.vertices.len());
        println!("    svdag  {dag_us} µs, {} bytes ({:.3} b/voxel) | aadf {aadf_us} µs | bricks {}", dag.compressed_bytes(), dag.bits_per_voxel(nonempty), pool.brick_count());
        break;
    }
    println!(
        "  karma Cielo: {:?}",
        rivaren_gameplay::Karma {
            compasion: 60.0,
            justicia: 40.0,
            sabiduria: 10.0
        }
        .death_destination()
    );
    Ok(())
}
