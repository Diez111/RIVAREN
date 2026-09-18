---
name: rivaren-dev
description: Convenciones, comandos y arquitectura del proyecto RIVAREN (motor de vóxeles Rust + wgpu). Úsala al tocar cualquier crate, shader, UI o sistema del juego.
---
# RIVAREN — guía de desarrollo

## Comandos esenciales
```bash
cargo test --workspace                       # tests (30 suites)
cargo clippy --workspace --all-targets       # lints
cargo run -p rivaren-app --release           # juego (ventana)
cargo run -p rivaren-app --release -- --headless            # benchmark
cargo run -p rivaren-app --release -- --screenshot ui.ppm   # captura UI
cargo run -p rivaren-app --release -- --screenshot-ingame g.ppm # captura 3D
cargo xtask build-all --dist                 # Linux+Windows+macOS
```

## Mapa de crates
- `core`: tipos (BlockId, PackedVertex 8B, DrawList, Fixed point, hash3).
- `world`: SDF (terrain/caves), biomas, estructuras, pipeline de chunks.
- `compression`: SVDAG+SSVDAG, AADF, bricks 8³, ventana deslizante.
- `meshing`: binary greedy + luz horneada (`bake_lighting`) + LOD + dirty set.
- `render`: wgpu 29; pases: sombras CSM, cielo, terreno, agua, bloom, tonemap+TAA, UI.
- `physics`: AABB, tick queue, fluidos, **Pulso** (cables/antorchas/palancas/compuertas/lámparas).
- `gameplay`: karma 3 ejes, dimensiones, respawn, items, inventario, crafteo, quests, diálogo, settings.
- `ai`: LLM híbrido (Rules/Ollama/OpenAI-compatible) con worker y streaming.
- `audio`: kira + síntesis procedural (sin assets de terceros).
- `ui`: toolkit propio (layout Stack, widgets, tema) → produce `DrawList`.
- `app`: ventana winit, pantallas, sesión de juego, guardado.
- `netcode`: UDP con acks + rollback determinista (QUIC tras feature).
- `mods`: manifests JSON con bloques/biomas/ejes de karma (WASM tras feature).

## Reglas que no se negocian
1. Determinismo total en generación (`seed` + coords).
2. `DrawList::clear()` cada frame en `App::build_ui`.
3. Uniforms con padding explícito (vec4/mat4 alineados a 16).
4. Bind groups con layout exacto (¡samplers comparison!).
5. Tests antes de commit; benchmarks antes de optimizar.
