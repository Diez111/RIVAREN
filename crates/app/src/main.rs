//! RIVAREN — binario principal multiplataforma (Linux/Mac/Windows/Android).
//! Main loop con frame budget, jobs con deadlines, resolución dinámica.
//! `--window`: ventana winit + wgpu real con el chunk mesheado en 3D.

mod budget;
mod jobs;
mod window_app;

use anyhow::Result;
use glam::IVec3;
use std::sync::atomic::AtomicBool;
use std::time::Instant;

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();
    tracing::info!("RIVAREN {} iniciando", env!("CARGO_PKG_VERSION"));

    let seed: u64 = std::env::var("RIVAREN_SEED")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(1337);

    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--window") {
        window_app::run_window(seed)?;
    } else {
        run_headless_demo(seed)?;
    }
    Ok(())
}

fn run_headless_demo(seed: u64) -> Result<()> {
    use rivaren_compression::{BrickPool, SvdagBuilder, build_aadf};
    use rivaren_meshing::{MeshData, greedy_mesh};
    use rivaren_world::pipeline::generate_chunk;

    let cancel = AtomicBool::new(false);
    println!("RIVAREN headless demo — seed={seed}");
    for cy in [0, 1, -1, 2] {
        let chunk = IVec3::new(0, cy, 0);
        let t0 = Instant::now();
        let builder = generate_chunk(seed, chunk, &cancel);
        let gen_ms = t0.elapsed().as_secs_f64() * 1000.0;
        let nonempty = builder.voxels.iter().filter(|&&v| v != 0).count();
        println!("  chunk {chunk:?}: {gen_ms:.2} ms, sólidos {nonempty}/32768");
        if nonempty > 1000 {
            let voxels: &[u16; 32768] = unsafe { &*builder.voxels.as_ptr().cast() };
            let t1 = Instant::now();
            let mut mesh = MeshData::default();
            greedy_mesh(voxels, &mut mesh);
            let mesh_us = t1.elapsed().as_micros();
            let t2 = Instant::now();
            let mut svdag = SvdagBuilder::new();
            let dag = svdag.build(voxels);
            let comp_us = t2.elapsed().as_micros();
            let t3 = Instant::now();
            let aadf = build_aadf(&dag);
            let aadf_us = t3.elapsed().as_micros();
            let mut pool = BrickPool::new();
            let brick_ids = pool.intern_chunk(voxels);
            let uniq = {
                let mut s = brick_ids.to_vec();
                s.sort_unstable();
                s.dedup();
                s.len()
            };
            println!("    → meshing {mesh_us} µs, {} quads, {} verts", mesh.quad_count(), mesh.vertices.len());
            println!("    → svdag {comp_us} µs, {} nodos, {} hojas, {} bytes ({:.3} b/voxel)",
                dag.nodes.len(), dag.leaves.len(), dag.compressed_bytes(),
                dag.bits_per_voxel(nonempty.max(1)));
            println!("    → aadf {aadf_us} µs, {} nodos | bricks: {uniq}/64 únicos, pool {}", aadf.len(), pool.brick_count());
            break;
        }
    }
    println!("  karma demo : {:?}", rivaren_gameplay::Karma { compasion: 60.0, justicia: 40.0, sabiduria: 10.0 }.death_destination());
    println!("  aldea demo : {:?}", rivaren_gameplay::village_fragment(seed, 0, 0).title);
    Ok(())
}
