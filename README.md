# RIVAREN

Motor de vóxeles infinito 3D en **Rust + wgpu 29 (Vulkan/DX12/Metal)** con juego
original: karma de 3 ejes, dimensiones Cielo/Tierra/Infierno, Pulso (señales),
IA híbrida para NPCs, mods JSON y multijugador con rollback.

## Compilar y jugar

```bash
cargo test --workspace                 # 30 suites verdes
cargo run -p rivaren-app --release     # juego (ventana)
cargo run -p rivaren-app --release -- --quickstart   # entra directo al mundo
```

Controles: **WASD** mover, ratón mirar, **E** inventario, **C** mesa de crafteo,
**R/J** diario, **F** hablar/interactuar, **1-9** hotbar, **F3** debug, **Esc** pausa.

## Verificación sin GPU/ventana

```bash
cargo run -p rivaren-app --release -- --headless                 # benchmarks de gen
cargo run -p rivaren-app --release -- --screenshot /tmp/ui.ppm   # captura de menú
cargo run -p rivaren-app --release -- --screenshot-ingame /tmp/g.ppm  # captura 3D
```

## Multiplataforma (una sola compilación)

```bash
cargo xtask build-all --release   # Linux + Windows + macOS en paralelo
cargo xtask build-all --dist      # LTO fat
cargo xtask build-all --mobile    # opt-s (Android)
cargo xtask check-all             # check de los 4 targets
./scripts/build-all.sh --dist     # wrapper
```

## Rendimiento medido (Ryzen 16 hilos, RADV)

| Sistema | Medición | Objetivo |
|---|---|---|
| Generación de chunk | **0.31 ms** | < 2 ms |
| Meshing greedy + luz | **254–320 µs** | < 200–500 µs |
| SVDAG | **57 µs**, 444 B | < 0.5 ms, 0.08–0.12 b/vóxel |
| AADF | **1 µs** | 3–5× menos pasos |
| Streaming | **~360 chunks** en captura | 60 FPS |
| Netcode handshake | **loopback OK** | determinista |

## Arquitectura

```
crates/{core,world,compression,meshing,render,lighting,entities,physics,
        gameplay,netcode,mods,audio,ui,ai,app} + xtask
assets/{shaders/*.wgsl, fonts/, data/}
.opencode/{agents,commands,skills}  ← flujo de trabajo con opencode
docs/adr/001..009
```

- **Mundo calculado**: `f(seed,x,y,z)` puro; RAM = seed + log de ediciones + ventana.
- **Compresión**: SVDAG + SSVDAG (simetría) + AADF + bricks 8³ + ventana 512³.
- **Render**: CSM 3 cascadas, sky atmosférico, agua, bloom, ACES, TAA con historial
  validado, upscale CAS-lite, lluvia procedural, UI SDF instanciada + glyphon.
- **Juego**: karma (Compasión/Justicia/Sabiduría), muerte → dimensión según karma,
  Portales de Ascensión/Descenso, Mapa del Alma, aldeas con Fragmento de Historia.
- **Pulso**: cables, antorchas, palancas, botones, compuertas Y/O/NO/XOR, lámparas.
- **IA híbrida**: reglas offline + Ollama/llama.cpp local + nube OpenAI-compatible.
- **Audio**: música y SFX **sintetizados en memoria** (100% original, sin assets).
- **Mods**: manifests JSON (bloques ≥ 200, biomas, ejes de karma); WASM tras feature.
- **Netcode**: UDP con acks y rollback determinista en fixed-point; QUIC tras feature.

## opencode

`opencode.json` + `.opencode/` definen agentes especializados (`engine-dev`,
`render-dev`, `ui-dev`, `ai-npc-dev`, `bench-guard`, `reviewer`) y comandos
`/build-all`, `/bench`, `/screenshot`. Tras editar configuración, reiniciar opencode.

## Licencia

**GNU General Public License v3.0 o posterior** (GPL-3.0-or-later).
Ver [LICENSE](LICENSE). Todo el contenido (código, lore, nombres, sonidos
sintetizados) es original; las fuentes incluidas son libres (DejaVu, Bitstream
Vera) y los shaders de upscale derivan de FidelityFX (MIT).

## Estado

Implementado: G0–G8 (renderer completo), UI + pantallas, inventario/crafteo, quests,
IA, audio, guardado, streaming y físicas, Pulso, clima visual, netcode y mods JSON.
Pendiente: WASM runtime (feature `wasm`), QUIC (feature `quic`), APK Android (xtask).
