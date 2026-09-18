# ADR-001: wgpu como única abstracción GPU (Vulkan/Metal/DX12)

- Fecha: 2026-09-18. Estado: aceptado.
- Contexto: soportar Linux (Vulkan), Windows (DX12/Vulkan), Mac (Metal) con un solo código.
- Decisión: wgpu 24 + Limits::downlevel_defaults (Vulkan 1.1 / GTX600 / Mali-G52).
- vulkano/ash solo si wgpu bloquea mesh shaders; no en Fase 1-6.
- Consecuencia: un `cargo xtask build-all` genera los 3 binarios; Metal vía traducción, sin código separado.
