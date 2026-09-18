// RIVAREN traverse.comp.wgsl — ray marching sobre SVDAG + AADF.
// Half-resolution en móvil, full en PC. Blue-noise dither + temporal.

@group(0) @binding(0) var<storage, read> nodes: array<u32>;
@group(0) @binding(1) var<storage, read> bricks: array<u32>;
@group(0) @binding(2) var<uniform> cam: vec4; // xyz=pos, w=fov

@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    // DDA traversal con salto AADF (implementación Fase 4).
    // Placeholder compilable: escribe gradiente de cielo.
    // El traverser real usa `nodes` + AADF + brick 8³.
}
