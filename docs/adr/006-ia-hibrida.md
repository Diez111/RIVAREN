# ADR-006: IA de NPCs híbrida (reglas / local / nube)

- Fecha: 2026-09-18. Estado: aceptado.
- Decisión: `crates/ai` define un servicio con worker dedicado y canales:
  - `Rules`: determinista, offline, siempre disponible (fallback).
  - `Local`: Ollama (`/api/chat`) o llama.cpp-server (OpenAI-compatible).
  - `Cloud`: API OpenAI-compatible con clave en variable de entorno.
- El prompt se construye con persona + lore + karma + dimensión + clima + memoria
  persistente del NPC. El streaming se simula por chunks para la UI.
- Nunca bloquea el frame; timeout configurable; errores degradan a reglas.
- Sin voz (TTS/STT descartado por decisión del usuario).
