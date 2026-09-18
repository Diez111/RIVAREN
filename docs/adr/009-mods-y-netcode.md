# ADR-009: Mods JSON y netcode UDP con rollback

- Fecha: 2026-09-18. Estado: aceptado.
- Mods: manifests JSON en `~/.local/share/rivaren/mods` con bloques (id ≥ 200),
  biomas y ejes de karma. Los nombres se registran como items colocables y el
  shader deriva su color del hash del id (HSV determinista). WASM (wasmtime)
  queda tras la feature `wasm` para comportamientos sandbox.
- Netcode: UDP con `Frame { seq, ack, payload }`, reenvío selectivo de eventos,
  rollback con historial de inputs (256) y snapshots (64). Simulación fixed-point
  determinista. QUIC (quinn) queda tras la feature `quic` para cifrado.
