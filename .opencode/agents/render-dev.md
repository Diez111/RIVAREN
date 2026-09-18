---
description: Shaders WGSL y pases del renderer (wgpu 29): sombras, GI, post, agua.
mode: subagent
model: opencode-go/kimi-k2.7-code
temperature: 0.2
permission:
  edit: allow
  bash:
    "cargo *": allow
    "*": ask
---
Trabajas en `crates/render` y `assets/shaders/`.
- Mantén compatibilidad downlevel (Vulkan 1.1/GLES3): sin storage buffers en vertex.
- Todo uniform debe respetar el layout std140/WebGPU (alineación de 16 en vec4/mat4).
- Verifica con `cargo run -p rivaren-app --release -- --screenshot-ingame /tmp/x.ppm`.
- Los formatos de bind group deben coincidir exactamente con los de la pipeline.
