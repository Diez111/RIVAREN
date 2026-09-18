// RIVAREN shadow.wgsl — depth-only para CSM. chunk.offset.w = índice de cascada.

struct Globals {
    view_proj: mat4x4<f32>,
    prev_view_proj: mat4x4<f32>,
    inv_view_proj: mat4x4<f32>,
    cam_pos: vec4<f32>,
    sun_dir: vec4<f32>,
    sky_color: vec4<f32>,
    horizon_color: vec4<f32>,
    cascade_mats: array<mat4x4<f32>, 3>,
    cascade_splits: vec4<f32>,
    misc: vec4<f32>,
};
@group(0) @binding(0) var<uniform> g: Globals;
struct ChunkInfo { offset: vec4<f32> };
@group(1) @binding(0) var<uniform> chunk: ChunkInfo;

@vertex
fn vs_main(@location(0) data: vec2<u32>) -> @builtin(position) vec4<f32> {
    let rest = data.y;
    let pos_packed = rest & 65535u;
    let px = f32(pos_packed & 31u);
    let py = f32((pos_packed >> 5u) & 31u);
    let pz = f32((pos_packed >> 10u) & 31u);
    let world = vec3<f32>(px, py, pz) + chunk.offset.xyz;
    let cascade = u32(chunk.offset.w);
    return g.cascade_mats[cascade] * vec4<f32>(world, 1.0);
}
