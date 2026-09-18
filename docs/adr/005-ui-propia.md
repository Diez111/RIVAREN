# ADR-005: Toolkit UI propio sobre wgpu (sin egui)

- Fecha: 2026-09-18. Estado: aceptado.
- Contexto: se evaluó egui (0.35) por rapidez; se descartó para tener control total
  del rendimiento, estética original y cero dependencias de UI de terceros.
- Decisión: `crates/ui` implementa layout (`Stack` con Fixed/Fill/Percent), widgets
  (botón, toggle, slider, campo de texto, tabs, slots, tooltip, progreso), tema y
  entrada; produce `core::DrawList` que `render` dibuja con quads SDF instanciados
  + texto glyphon.
- Consecuencias: control total y 100% Rust; a cambio hay que mantener widgets.
  Regla: el DrawList se limpia cada frame (bug de acumulación corregido).
