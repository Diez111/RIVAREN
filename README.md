# RIVAREN

Motor voxel infinito 3D en Rust + Vulkan (wgpu) + juego original de karma y 3 dimensiones.
Multiplataforma: **Linux + Windows + Mac con una sola compilación** (`cargo xtask build-all`).

## Compilación unificada (una vez, 3 OS)

```bash
# Todo en paralelo con todos los hilos CPU + sccache si existe:
cargo xtask build-all --release   # dev optimizado
cargo xtask build-all --dist      # LTO fat, máximo rendimiento
cargo xtask build-all --mobile    # opt-s para Mali / Android

# O el script:
./scripts/build-all.sh --dist

# Checks rápidos de los 4 targets:
cargo xtask check-all

# Demo headless (sin ventana, CI-friendly):
cargo run -p rivaren-app
RIVAREN_SEED=42 cargo run -p rivaren-app --release
```

Binarios: `target/<triple>/release/rivaren` (+ `.exe` en Windows).
Empaquetar: `cargo xtask package` → `dist/*.tar.gz`.

## Arquitectura

```
crates/{core,world,compression,meshing,render,lighting,entities,physics,
        gameplay,netcode,mods,audio,ui,app} + xtask (compilador unificado)
assets/{shaders/*.wgsl, data/*.json}
docs/adr/ — decisiones técnicas
```

## Optimización (resumen)

- Mundo calculado `f(seed,x,y,z)`, RAM = seed + deltas + ventana 512³ GPU.
- quick-noise manual + amortized lattice + SIMD `wide` (8-wide).
- SVDAG + SSVDAG (3 bits simetría) + AADF (+12B/nodo, 3-5x menos pasos) + bricks 8³.
- Binary greedy meshing (~65-90µs), LOD0-4, re-meshing incremental.
- Render GPU-driven: compute cull → MDI indirect, HZB 2-pass, VRS, FSR.
- Jobs <1ms con deadlines + cancelación cooperativa, fixed 20 TPS.
- Perfiles: `release` (thin LTO) / `dist` (fat LTO, abort, 1 codegen) / `mobile` (opt-s).

## Estado

Fase 1 fundación: workspace + SDF + biomas + SVDAG + greedy + demo headless.
Ver `docs/adr/` y el MEGA PLAN en el historial del proyecto.
