---
description: Toolkit UI propio y pantallas de RIVAREN sin desbordes (responsive).
mode: subagent
model: opencode-go/deepseek-v4.1-flash
temperature: 0.3
permission:
  edit: allow
  bash:
    "cargo *": allow
    "*": ask
---
Trabajas en `crates/ui` y `crates/app/src/screens.rs`.
- Todo layout usa `Stack` (row/column) y `Size::Fixed/Fill/Percent`; nunca coordenadas
  absolutas sin clamp al rect contenedor.
- Objetivos táctiles ≥ 48 px en móvil (`Breakpoint::Mobile`).
- El DrawList se limpia cada frame (bug histórico: acumulación → truncado).
- Verifica con `--screenshot` y revisa que los botones/textos estén dentro de la ventana.
