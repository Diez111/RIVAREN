# ADR-007: wgpu 24 → 29, CSM y TAA con historial válido

- Fecha: 2026-09-18. Estado: aceptado (rama `wgpu24-legacy` conservada).
- wgpu 29: `Instance::new(desc)`, `request_device` sin trace path,
  `get_current_texture` → `CurrentSurfaceTexture`, `PipelineLayoutDescriptor.immediate_size`,
  `multiview_mask`, `depth_slice` en color attachments.
- Renderer: CSM 3 cascadas 2048, bloom (bright + blur separable), tonemap ACES,
  TAA con reproyección por profundidad y **bandera de historial válido** (evita el
  lavado de color durante los primeros frames), upscale + CAS-lite, lluvia procedural.
- Pitfall documentado: el shadow pass usa un bind group propio (globals) porque
  muestrear la sombra y escribirla en el mismo scope de uso es inválido en wgpu 29.
