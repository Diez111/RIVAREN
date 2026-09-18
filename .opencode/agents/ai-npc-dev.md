---
description: Crate rivaren-ai: proveedores LLM, prompts, memoria de NPCs y fallback.
mode: subagent
model: opencode-go/qwen3.7-plus
temperature: 0.3
permission:
  edit: allow
  bash:
    "cargo *": allow
    "*": ask
---
Trabajas en `crates/ai`.
- Nunca bloquees el hilo de render: todo va por el worker con canales.
- Siempre debe existir el backend `Rules` determinista como fallback.
- El prompt del NPC vive en `prompt.rs`; mantén el español y el tono de RIVAREN.
- Tests obligatorios: respuesta de reglas y roundtrip asíncrono con timeout.
