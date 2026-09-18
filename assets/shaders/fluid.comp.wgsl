// RIVAREN fluid.comp.wgsl — autómata agua viva (solo chunks activos).
// Nivel 0..7 en R8Uint 3D. Reglas espejo de physics::fluid_step.

@group(0) @binding(0) var<storage, read> state_in: array<u32>;
@group(0) @binding(1) var<storage, read_write> state_out: array<u32>;
@group(0) @binding(2) var<uniform> dims: vec4<u32>; // 32,32,32,active_count

fn idx(x: u32, y: u32, z: u32) -> u32 {
    return (y * 32u + z) * 32u + x;
}

@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= 32u || id.y >= 32u || id.z >= 32u) {
        return;
    }
    let lv = state_in[idx(id.x, id.y, id.z)] & 7u;
    if (lv == 0u) {
        return;
    }
    // Caída: si aire debajo, mueve (el host marca el chunk activo vecino).
    if (id.y > 0u && (state_in[idx(id.x, id.y - 1u, id.z)] & 8u) == 0u) {
        state_out[idx(id.x, id.y - 1u, id.z)] = 7u;
    } else {
        // Expansión a 4 vecinos con nivel-1.
        let s = lv - 1u;
        if (s > 0u) {
            if (id.x > 0u) { state_out[idx(id.x - 1u, id.y, id.z)] = max(state_out[idx(id.x - 1u, id.y, id.z)], s); }
            if (id.x < 31u) { state_out[idx(id.x + 1u, id.y, id.z)] = max(state_out[idx(id.x + 1u, id.y, id.z)], s); }
            if (id.z > 0u) { state_out[idx(id.x, id.y, id.z - 1u)] = max(state_out[idx(id.x, id.y, id.z - 1u)], s); }
            if (id.z < 31u) { state_out[idx(id.x, id.y, id.z + 1u)] = max(state_out[idx(id.x, id.y, id.z + 1u)], s); }
        }
    }
}
