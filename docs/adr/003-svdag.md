# ADR-003: SVDAG + SSVDAG + AADF + bricks 8³ + ventana 512³

- 0.08-0.12 b/voxel objetivo; chunk 32³ → 2-8KB.
- AADF +12B/nodo → 3-5x menos pasos raymarch (NAADF 2026: total 10x).
- Ventana deslizante: GPU nunca crece; shift por compute shader.
