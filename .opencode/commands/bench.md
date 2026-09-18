---
description: Corre tests y benchmarks clave de RIVAREN y compara con el presupuesto.
agent: bench-guard
---
Corre:
1. `cargo test --workspace`
2. `cargo run -p rivaren-app --release -- --headless`

Reporta en una tabla: gen ms, meshing µs, SVDAG µs y bytes/vóxel, y si cumplen
los presupuestos (gen < 2 ms, meshing < 200 µs, bits/vóxel < 0.12 en mundo real).
