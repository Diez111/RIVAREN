# AGENTS.md — Reglas para agentes en RIVAREN

## Qué es
Motor de vóxeles infinito 3D en Rust + wgpu 29 (Vulkan/DX12/Metal) con juego
original: karma de 3 ejes, Tierra/Cielo/Infierno, Pulso (señales), mods JSON/WASM.

## Comandos
- `cargo test --workspace` — obligatorio antes de cada commit.
- `cargo clippy --workspace --all-targets` — sin errores.
- `cargo run -p rivaren-app --release` — juego completo (ventana).
- `cargo run -p rivaren-app --release -- --headless` — benchmark de generación.
- `cargo run -p rivaren-app --release -- --screenshot /tmp/ui.ppm` — captura de UI.
- `cargo run -p rivaren-app --release -- --screenshot-ingame /tmp/game.ppm` — captura 3D.
- `cargo xtask build-all --release|--dist|--mobile` — Linux + Windows + macOS.
- `cargo xtask check-all` — check de los 4 targets.

## Reglas de oro
1. **Determinismo**: `f(seed,x,y,z)` puro; sin HashMap con semilla aleatoria en generación.
2. **Cero allocs en hot path**: buffers reutilizados; `MeshData::clear()` antes de reusar.
3. **Presupuesto**: gen de chunk < 2 ms, meshing < 200 µs, SVDAG < 0.5 ms, frame 60 FPS.
4. **Sin comentarios** salvo doc de módulo (`//!`) y bloques `// ──` de sección.
5. **UI responsiva**: todo widget dentro de su rect; sin desbordes; soporte táctil ≥ 48 px.
6. **Nada de contenido de terceros**: nombres/assets/lore 100% originales (ADR-004).
7. **Mods**: JSON en `~/.local/share/rivaren/mods`; bloques con id ≥ 200.
8. **Grpc de error**: usar `anyhow`/`thiserror`; nunca `unwrap` en código de juego.
9. **Commits**: convención `area: mensaje` (world:, render:, ui:, gameplay:, fix:).

## Arquitectura
`crates/{core,world,compression,meshing,render,lighting,entities,physics,gameplay,netcode,mods,audio,ui,ai,app}`
- DAG: `app → {gameplay, render, ui, ai, audio, mods, netcode} → ... → core`.
- `render` consume `core::DrawList` producido por `ui`.
- `gameplay` no depende de render/UI.
