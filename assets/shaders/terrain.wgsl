// RIVAREN terrain.wgsl — raster de chunks con PackedVertex (8B = 2×u32).
// data.x = uv_tex (u:8 v:8 tex:12), data.y = pos_packed:16 | ao:4 light:4 pad:8

struct Camera {
    view_proj: mat4x4<f32>,
};

@group(0) @binding(0) var<uniform> cam: Camera;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) color: vec3<f32>,
    @location(1) shade: f32,
};

fn block_color(tex: u32) -> vec3<f32> {
    switch tex {
        case 1u: { return vec3<f32>(0.35, 0.62, 0.28); }
        case 2u: { return vec3<f32>(0.45, 0.32, 0.20); }
        case 4u: { return vec3<f32>(0.55, 0.57, 0.62); }
        case 6u: { return vec3<f32>(0.85, 0.75, 0.50); }
        case 20u: { return vec3<f32>(0.20, 0.45, 0.80); }
        case 21u: { return vec3<f32>(0.70, 0.68, 0.75); }
        default: { return vec3<f32>(0.80, 0.30, 0.80); }
    }
}

@vertex
fn vs_main(@location(0) data: vec2<u32>) -> VsOut {
    var out: VsOut;
    let uv_tex_packed = data.x;
    let rest = data.y;
    let pos_packed = rest & 65535u;
    let ao_light_packed = (rest >> 16u) & 255u;
    let px = f32(pos_packed & 31u);
    let py = f32((pos_packed >> 5u) & 31u);
    let pz = f32((pos_packed >> 10u) & 31u);
    let normal = (pos_packed >> 15u) & 7u;
    let tex = (uv_tex_packed >> 16u) & 4095u;
    let ao = f32(ao_light_packed & 15u) / 3.0;
    let world = vec4<f32>(px - 16.0, py - 16.0, pz - 16.0, 1.0);
    out.pos = cam.view_proj * world;
    out.color = block_color(tex);
    var shade = 1.0;
    switch normal {
        case 2u, 3u: { shade = 1.0; }
        case 0u, 1u: { shade = 0.75; }
        default: { shade = 0.6; }
    }
    out.shade = shade * (0.55 + 0.45 * ao);
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    return vec4<f32>(in.color * in.shade, 1.0);
}
